using System.Net;
using System.Security.Cryptography;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

namespace ImageGuide;

public sealed record IsoDownloadResult(Guid RunId, string Path, long Bytes, string Sha256, bool TrustedChecksumMatched);

public static partial class Engine
{
    public const string MicrosoftIsoPage = "https://www.microsoft.com/software-download/windows11";
    const long IsoMinimum = 4L * 1024 * 1024 * 1024, IsoMaximum = 32L * 1024 * 1024 * 1024;
    sealed class DownloadRun(string parent, string partial, string identity, string directory)
    {
        public readonly string Parent = parent, Partial = partial, Identity = identity, Directory = directory;
        public bool Canceled;
        public long ExpectedBytes, ExpectedWrite;
        public string ExpectedHash = Convert.ToHexString(SHA256.HashData([]));
        public FileSnapshot? Frozen, Preview;
    }
    static readonly Dictionary<Guid, DownloadRun> downloads = new();
    static readonly object downloadGate = new();

    public static Uri ValidateMicrosoftIsoLink(string text)
    {
        if (text.Length > 16384 || !Uri.TryCreate(text.Trim(), UriKind.Absolute, out var uri) || uri.Scheme != "https" || !uri.IsDefaultPort ||
            uri.UserInfo.Length != 0 || uri.Fragment.Length != 0 || (uri.IdnHost != "software.download.prss.microsoft.com" && uri.IdnHost != "software-download.microsoft.com") ||
            !uri.AbsolutePath.EndsWith(".iso", StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Paste a generated HTTPS ISO link from Microsoft's Windows 11 download page. Other hosts, ports and non-ISO links are not accepted.");
        return uri;
    }

    public static Task<IsoDownloadResult> DownloadIso(string link, string destination, string? expectedSha256, JobControl control, IProgress<JobProgress>? progress = null)
    {
        var uri = ValidateMicrosoftIsoLink(link);
        var handler = new HttpClientHandler { AllowAutoRedirect = false, UseCookies = false, Credentials = null, AutomaticDecompression = DecompressionMethods.None };
        return Task.Run(async () => { using (handler) return await DownloadIsoCore(uri, destination, expectedSha256, control, progress, handler, IsoMinimum, IsoMaximum); });
    }

    internal static async Task<IsoDownloadResult> DownloadIsoCore(Uri initial, string destination, string? expectedSha256, JobControl control,
        IProgress<JobProgress>? progress, HttpMessageHandler handler, long minimum, long maximum)
    {
        ValidateMicrosoftIsoLink(initial.AbsoluteUri);
        expectedSha256 = string.IsNullOrWhiteSpace(expectedSha256) ? null : expectedSha256.Trim().ToUpperInvariant();
        if (expectedSha256 != null && !System.Text.RegularExpressions.Regex.IsMatch(expectedSha256, "^[A-F0-9]{64}$")) throw new InvalidDataException("The optional SHA-256 checksum must contain exactly 64 hexadecimal characters.");
        if (!Path.IsPathFullyQualified(destination) || destination.StartsWith(@"\\", StringComparison.Ordinal) || destination.Length < 4 ||
            destination[1] != ':' || destination.IndexOf(':', 2) >= 0 || destination.Split('\\', '/').Skip(1).Any(x => x.Length == 0 || x.EndsWith(' ') || x.EndsWith('.') ||
                System.Text.RegularExpressions.Regex.IsMatch(x, @"^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)", System.Text.RegularExpressions.RegexOptions.IgnoreCase)))
            throw new InvalidDataException("Choose a normal absolute local .iso filename; device paths and alternate data streams are not accepted.");
        destination = Path.GetFullPath(destination);
        string parent = Path.GetDirectoryName(destination) ?? throw new InvalidDataException("Choose a destination folder.");
        CheckPath(parent); CheckPath(destination);
        var drive = new DriveInfo(Path.GetPathRoot(parent)!);
        if (!Directory.Exists(parent) || parent.StartsWith(@"\\", StringComparison.Ordinal) || drive.DriveType != DriveType.Fixed || drive.DriveFormat != "NTFS" ||
            !destination.EndsWith(".iso", StringComparison.OrdinalIgnoreCase) || File.Exists(destination) || Directory.Exists(destination))
            throw new IOException("Choose a new .iso filename in an existing local NTFS folder. Existing files will not be replaced.");
        using var directoryLease = CleanupNative.LeaseDirectory(parent);
        string directoryIdentity = CleanupNative.DirectoryIdentity(parent);
        void Recheck() { CheckPath(parent); if (CleanupNative.DirectoryIdentity(parent) != directoryIdentity) throw new IOException("Download folder changed. Partial data is retained."); }
        using var client = new HttpClient(handler, false) { Timeout = Timeout.InfiniteTimeSpan };
        client.DefaultRequestHeaders.UserAgent.ParseAdd("WinISOPilot/1.0");
        HttpResponseMessage? response = null;
        try
        {
            Uri current = initial;
            for (int redirects = 0; ; redirects++)
            {
                control.Checkpoint("Connecting to Microsoft", progress);
                using var request = new HttpRequestMessage(HttpMethod.Get, current);
                response = await NetworkStep(token => client.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, token), control);
                int status = (int)response.StatusCode;
                if (status is 301 or 302 or 303 or 307 or 308)
                {
                    if (redirects >= 3 || response.Headers.Location == null) throw new IOException("Microsoft link redirected too many times or omitted its destination.");
                    current = ValidateMicrosoftIsoLink(new Uri(current, response.Headers.Location).AbsoluteUri);
                    response.Dispose(); response = null; continue;
                }
                if (response.StatusCode != HttpStatusCode.OK) throw new IOException("Microsoft download request returned HTTP " + status + ". Generate a fresh link on Microsoft's page.");
                break;
            }
            long length = response.Content.Headers.ContentLength ?? throw new IOException("Download size is unknown. No file was created.");
            string? type = response.Content.Headers.ContentType?.MediaType;
            if (length < minimum || length > maximum || response.Content.Headers.ContentEncoding.Count != 0 || response.Content.Headers.ContentRange != null ||
                (type != null && type is not ("application/octet-stream" or "application/x-iso9660-image" or "application/x-iso-image")))
                throw new IOException("Response is not an ISO-sized binary download. No file was created.");
            if (drive.AvailableFreeSpace < checked(length + 1024L * 1024 * 1024)) throw new IOException("Not enough free space for the ISO plus a 1 GB margin.");
            control.Checkpoint("Starting ISO download", progress); Recheck();
            string partial = Path.Combine(parent, Path.GetFileName(destination) + "." + control.RunId.ToString("N") + ".partial");
            DownloadRun? run = null;
            try
            {
                using (var handle = DownloadNative.Create(partial))
                using (var file = new FileStream(handle, FileAccess.ReadWrite, 1024 * 1024, false))
                {
                    string identity = CleanupNative.CreatedIdentity(handle, partial);
                    run = new(parent, partial, identity, directoryIdentity);
                    lock (downloadGate) downloads.Add(control.RunId, run);
                    try
                    {
                    using var input = await response.Content.ReadAsStreamAsync();
                    using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
                    byte[] buffer = new byte[1024 * 1024]; long received = 0;
                    while (true)
                    {
                        control.Checkpoint("Downloading ISO", progress, received, length);
                        int read = await NetworkStep(token => input.ReadAsync(buffer.AsMemory(), token).AsTask(), control);
                        if (read == 0) break;
                        received = checked(received + read);
                        if (received > length) throw new IOException("Download exceeded its declared length.");
                        Recheck(); file.Write(buffer, 0, read); hash.AppendData(buffer, 0, read);
                        run.ExpectedBytes = received; run.ExpectedHash = Convert.ToHexString(hash.GetCurrentHash());
                    }
                    if (received != length) throw new IOException("Download ended before its declared length. Partial data is retained.");
                    file.Flush(true);
                    control.Checkpoint("Checking downloaded ISO", progress);
                    if (!HasIsoDescriptor(file)) throw new IOException("Downloaded data has no recognized ISO/UDF descriptor. It will not be offered as an ISO.");
                    string digest = Convert.ToHexString(hash.GetHashAndReset());
                    if (expectedSha256 != null && digest != expectedSha256) throw new IOException("SHA-256 did not match the supplied checksum. The file is not approved for use.");
                    file.Position = 0; using var readback = IncrementalHash.CreateHash(HashAlgorithmName.SHA256); long checkedBytes = 0; int n;
                    while ((n = file.Read(buffer)) != 0) { control.Checkpoint("Verifying downloaded file", progress, checkedBytes, length); readback.AppendData(buffer, 0, n); checkedBytes += n; }
                    if (checkedBytes != length || Convert.ToHexString(readback.GetHashAndReset()) != digest) throw new IOException("Downloaded-file readback did not match transfer bytes.");
                    control.FinalCheckpoint("Finalizing download", progress); Recheck();
                    DownloadNative.Rename(handle, destination);
                    lock (downloadGate) downloads.Remove(control.RunId);
                    JobControl.Publish(progress, new("Complete", "ISO downloaded. Select Use downloaded ISO, then inspect it separately. No installer ran."));
                    return new(control.RunId, destination, length, digest, expectedSha256 != null);
                    }
                    catch (OperationCanceledException)
                    {
                        file.Flush(true);
                        run.ExpectedWrite = File.GetLastWriteTimeUtc(partial).ToFileTimeUtc();
                        throw;
                    }
                }
            }
            catch (OperationCanceledException)
            {
                if (run != null)
                {
                    Recheck(); var frozen = CleanupNative.Snapshot(partial, false, out _, true);
                    if (frozen.Identity == run.Identity && frozen.Bytes == run.ExpectedBytes && frozen.Sha256 == run.ExpectedHash && frozen.LastWrite == run.ExpectedWrite)
                    { run.Frozen = frozen; run.Canceled = true; }
                }
                throw;
            }
            catch (Exception) { lock (downloadGate) downloads.Remove(control.RunId); throw; }
        }
        finally { response?.Dispose(); }
    }

    static async Task<T> NetworkStep<T>(Func<CancellationToken, Task<T>> operation, JobControl control)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
        Task<T> task = operation(timeout.Token);
        while (!task.IsCompleted)
        {
            if (control.CancellationRequested) { timeout.Cancel(); try { await task; } catch { } throw new OperationCanceledException("Download canceled. Partial data is retained unless you choose guarded cleanup."); }
            await Task.WhenAny(task, Task.Delay(100));
        }
        try { return await task; } catch (OperationCanceledException) when (!control.CancellationRequested) { throw new IOException("Microsoft download timed out. Partial data is retained; generate a new link and restart."); }
    }

    internal static bool HasIsoDescriptor(Stream file)
    {
        byte[] descriptor = new byte[7];
        for (int sector = 16; sector <= 32 && (long)sector * 2048 + descriptor.Length <= file.Length; sector++)
        {
            file.Position = (long)sector * 2048;
            file.ReadExactly(descriptor);
            string id = Encoding.ASCII.GetString(descriptor, 1, 5);
            if (descriptor[6] == 1 && id is "CD001" or "NSR02" or "NSR03") return true;
        }
        return false;
    }

    public static CleanupPreview PreviewDownloadCleanup(Guid id)
    {
        lock (downloadGate)
        {
            if (!downloads.TryGetValue(id, out var run) || !run.Canceled || run.Frozen == null) return new(false, [], 0, ["No safely canceled partial download is eligible."]);
            try
            {
                if (CleanupNative.DirectoryIdentity(run.Parent) != run.Directory) throw new IOException("Folder identity changed.");
                var snapshot = CleanupNative.Snapshot(run.Partial, false, out _, true);
                if (snapshot != run.Frozen) throw new IOException("Partial file changed; retain it.");
                run.Preview = snapshot;
                return new(true, [new(Path.GetFileName(run.Partial), snapshot.Bytes)], snapshot.Bytes, ["Only this unchanged partial download is eligible. The destination, source ISO and other files are never deleted."]);
            }
            catch (Exception) { return new(false, [], 0, ["Partial-file identity or content could not be proven. Keep it."]); }
        }
    }

    public static CleanupResult CleanupDownload(Guid id)
    {
        lock (downloadGate)
        {
            try
            {
                if (!downloads.TryGetValue(id, out var run) || !run.Canceled || run.Preview == null) throw new IOException("A current guarded partial-file preview is required.");
                var preview = run.Preview; run.Preview = null;
                using var directory = CleanupNative.LeaseDirectory(run.Parent);
                if (CleanupNative.DirectoryIdentity(run.Parent) != run.Directory) throw new IOException("Folder changed; partial file retained.");
                var now = CleanupNative.Snapshot(run.Partial, true, out var held, true);
                using (held) { if (now != preview) throw new IOException("Partial file changed; retained."); CleanupNative.DeleteHeld(held!); }
                downloads.Remove(id); return new([Path.GetFileName(run.Partial)], [], [], false);
            }
            catch (Exception ex) { return new([], [], [ex.Message], false); }
        }
    }

    static class DownloadNative
    {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern SafeFileHandle CreateFileW(string path, uint access, uint share, nint security, uint creation, uint flags, nint template);
        [DllImport("kernel32.dll", SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)] static extern bool SetFileInformationByHandle(SafeFileHandle handle, int kind, nint info, uint size);
        [StructLayout(LayoutKind.Sequential)] struct RenameInfo { public uint Flags; public nint Root; public uint Length; public ushort Name; }
        internal static SafeFileHandle Create(string path)
        {
            CheckPath(path); var handle = CreateFileW(path, 0xC0010000, 1, 0, 1, 0x00200000, 0);
            if (handle.IsInvalid) { handle.Dispose(); throw new IOException("Cannot exclusively create the partial download."); }
            return handle;
        }
        internal static void Rename(SafeFileHandle handle, string destination)
        {
            CheckPath(destination); byte[] name = Encoding.Unicode.GetBytes(destination);
            int offset = (int)Marshal.OffsetOf<RenameInfo>(nameof(RenameInfo.Name)); int size = checked(offset + name.Length + 2);
            nint buffer = Marshal.AllocHGlobal(size);
            try
            {
                for (int i = 0; i < size; i++) Marshal.WriteByte(buffer, i, 0);
                Marshal.WriteInt32(buffer, (int)Marshal.OffsetOf<RenameInfo>(nameof(RenameInfo.Length)), name.Length);
                Marshal.Copy(name, 0, buffer + offset, name.Length);
                if (!SetFileInformationByHandle(handle, 3, buffer, (uint)size)) throw new IOException("Cannot publish the downloaded ISO without overwriting. Partial file retained; Win32 " + Marshal.GetLastWin32Error());
            }
            finally { Marshal.FreeHGlobal(buffer); }
        }
    }
}
