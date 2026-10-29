using System.Diagnostics;
using System.Security.Cryptography;
using System.Security.Principal;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Win32.SafeHandles;

namespace ImageGuide;

public sealed record ProvisionedApp(string PackageName, string DisplayName, bool Removable, string Explanation);
public sealed record ServicingInventory(Guid Id, string SourceSha256, int EditionIndex, string EditionVersion,
    string ToolPath, string ToolVersion, string ToolSha256, string WimSha256, ProvisionedApp[] Apps);
public sealed record ServicingChoices(Guid InventoryId, string[] RemovePackages, bool ComponentCleanup)
{
    public bool HasChanges => RemovePackages.Length != 0 || ComponentCleanup;
}
public sealed record ServicingReadiness(bool Available, string[] Reasons);
public sealed record ServicingEvidence(string[] RemovedPackages, bool ComponentCleanup, string BeforeWimSha256, string AfterWimSha256);

public static partial class Engine
{
    internal static Inspection FreezeInspection(Inspection inspection)
    {
        if(inspection.Editions==null)throw new InvalidDataException("Missing observed editions.");
        return inspection with { Editions=inspection.Editions.Select(x=>x with{Languages=x.Languages?.ToList()}).ToList(),Tools=inspection.Tools?.ToList()??[] };
    }
    static readonly Dictionary<Guid, ServicingInventory> servicingInventories = new();
    static readonly object inventoryGate = new();
    static readonly Regex PackageIdentifier = new(@"^[A-Za-z0-9][A-Za-z0-9.\-]{0,150}_[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+_(?:x64|x86|arm64|neutral)_[A-Za-z0-9.~\-]*_[a-z0-9]{13}$", RegexOptions.CultureInvariant);
    static readonly string[] RemovableFamilies = ["Clipchamp.Clipchamp", "Microsoft.MicrosoftSolitaireCollection", "Microsoft.BingNews", "Microsoft.BingWeather", "Microsoft.GetHelp", "Microsoft.Getstarted"];
    static string RecoveryRoot => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "WinISOPilot", "ServicingRecovery");
    public static bool IsAdministrator { get { using var identity=WindowsIdentity.GetCurrent(); return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator); } }
    public static bool HasUnresolvedServicing
    {
        get
        {
            try { CheckPath(RecoveryRoot); return Directory.Exists(RecoveryRoot) && Directory.EnumerateFiles(RecoveryRoot, "*.pending.json").Any(); }
            catch { return true; }
        }
    }
    public static ServicingReadiness CheckServicing(Edition? edition, IEnumerable<Tool>? tools = null)
    {
        var reasons = new List<string>();
        if (!IsAdministrator) reasons.Add("Reopen WinISOPilot as administrator to read or customize offline apps.");
        if (RestartRequired || HasUnresolvedServicing) reasons.Add("An unfinished servicing journal needs manual recovery. Keep all files; no automatic mount cleanup is allowed. Journal folder: " + RecoveryRoot);
        var dism = (tools ?? Discover()).FirstOrDefault(x => x.Kind == "ADK DISM");
        if (edition == null) reasons.Add("Inspect the ISO and confirm an edition first.");
        else if (edition.Architecture != "x64" || edition.Installation != "Client" || edition.Build != 26100)
            reasons.Add("Unsupported image/tool combination. This first servicing profile accepts x64 Windows 11 build 26100 only; build " + edition.Build + " is not approved.");
        if (dism == null || !Regex.IsMatch(dism.Version, @"^10\.0\.26100\.")) reasons.Add("Microsoft Windows ADK 26100 Deployment Tools are required for this servicing profile.");
        return new(reasons.Count == 0, reasons.ToArray());
    }
    static Tool RequireServicing(Edition edition)
    {
        var tools = Discover(); var ready = CheckServicing(edition, tools);
        if (!ready.Available) throw new InvalidOperationException(string.Join("\n", ready.Reasons));
        return tools.First(x => x.Kind == "ADK DISM");
    }
    public static ProvisionedApp DescribeApp(string packageName)
    {
        if (!PackageIdentifier.IsMatch(packageName)) throw new InvalidDataException("Invalid provisioned app identity.");
        string family = packageName[..packageName.IndexOf('_')];
        string resource = packageName.Split('_')[3];
        string publisher = packageName.Split('_')[4];
        bool approvedPublisher = publisher == (family == "Clipchamp.Clipchamp" ? "yxz26nhyzhsrt" : "8wekyb3d8bbwe");
        bool allowed = RemovableFamilies.Contains(family, StringComparer.Ordinal) && approvedPublisher && (resource.Length == 0 || resource == "~");
        return new(packageName, family, allowed, allowed
            ? "Removing this app's provisioning means it will not be installed automatically for new users of this edition. Its function will be unavailable. Reinstallation may require the Store or installation files; dependency and boot tests have not run."
            : "Protected or outside the first supported removal list. No general dependency resolver is available; keep this app.");
    }
    public static ProvisionedApp[] ParseProvisionedApps(string text)
    {
        if (text.Length > 4 * 1024 * 1024 || !text.Contains("The operation completed successfully.", StringComparison.Ordinal)) throw new InvalidDataException("Incomplete English app inventory.");
        var blocks = Regex.Split(text, @"(?m)(?=^DisplayName\b)");
        if (Regex.IsMatch(blocks[0], @"(?m)^(?:PackageName|DisplayName)\b")) throw new InvalidDataException("Unpaired app inventory field.");
        var apps = new List<ProvisionedApp>();
        foreach (string block in blocks.Skip(1))
        {
            _ = One(block, "DisplayName");
            apps.Add(DescribeApp(One(block, "PackageName")));
        }
        if (apps.Count > 4096 || Regex.Matches(text, @"(?m)^PackageName\b").Count != apps.Count || Regex.Matches(text, @"(?m)^DisplayName\b").Count != apps.Count || apps.Select(x => x.PackageName).Distinct(StringComparer.Ordinal).Count() != apps.Count)
            throw new InvalidDataException("Incomplete, malformed or duplicate app records.");
        return apps.OrderBy(x => x.PackageName, StringComparer.Ordinal).ToArray();
    }

    internal static string[] ValidateServicingChoices(ServicingInventory inventory, ServicingChoices choices)
    {
        if (choices.InventoryId != inventory.Id || choices.RemovePackages == null || choices.RemovePackages.Length > 64 || choices.RemovePackages.Distinct(StringComparer.Ordinal).Count() != choices.RemovePackages.Length)
            throw new InvalidDataException("Stale or duplicate servicing choices. Read apps again.");
        foreach (string name in choices.RemovePackages)
            if (!inventory.Apps.Any(x => x.PackageName == name && x.Removable) || !DescribeApp(name).Removable)
                throw new InvalidDataException("App was not observed as removable: " + name);
        return choices.RemovePackages.Order(StringComparer.Ordinal).ToArray();
    }
    sealed class ServicingLease : IDisposable
    {
        readonly Mutex mutex = new(false, @"Global\WinISOPilot-Servicing-v1");
        bool held;
        public ServicingLease()
        {
            try { held = mutex.WaitOne(0); } catch (AbandonedMutexException) { held = true; }
            if (!held) { mutex.Dispose(); throw new IOException("Another servicing operation is active."); }
        }
        public void Dispose() { if (held) { held = false; mutex.ReleaseMutex(); } mutex.Dispose(); }
    }
    sealed class OfflineSession
    {
        readonly string root, wim, mount, scratch, toolHash, wimIdentity, mountIdentity, scratchIdentity, rootIdentity, recovery;
        readonly Tool tool;
        readonly int index;
        readonly JobControl control;
        readonly IProgress<JobProgress>? progress;
        readonly Action<string, string> write;
        readonly FileStream toolLease, wimLease;
        readonly SafeFileHandle rootLease, mountLease, scratchLease;
        readonly Func<string[], Task<ProcessOutput>>? testRunner;
        readonly Action? spaceCheck;
        int serial;
        bool mountAttempted, detached;
        public OfflineSession(string root, string wim, int index, Tool tool, JobControl control, IProgress<JobProgress>? progress, Action<string, string> write, string? expectedToolHash = null, Func<string[], Task<ProcessOutput>>? testRunner = null, string? fixtureRecoveryRoot = null, Action? spaceCheck = null)
        {
            this.root = root; this.wim = wim; this.index = index; this.tool = tool; this.control = control; this.progress = progress; this.write = write;
            this.testRunner = testRunner;this.spaceCheck=spaceCheck;
            CheckPath(root); CheckPath(wim); rootIdentity = CleanupNative.DirectoryIdentity(root);
            mount = Path.Combine(root, "servicing-mount"); scratch = Path.Combine(root, "servicing-scratch");
            if (!CreateDirectoryW(mount, IntPtr.Zero) || !CreateDirectoryW(scratch, IntPtr.Zero)) throw new IOException("Servicing destinations already exist or cannot be exclusively created.");
            mountIdentity = CleanupNative.DirectoryIdentity(mount); scratchIdentity = CleanupNative.DirectoryIdentity(scratch);
            rootLease = CleanupNative.LeaseDirectory(root);
            try
            {
                mountLease = CleanupNative.LeaseDirectory(mount);
                try
                {
                    scratchLease = CleanupNative.LeaseDirectory(scratch);
                    try
                    {
                        wimLease = new FileStream(wim, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
                        try
                        {
                            wimIdentity = CleanupNative.CreatedIdentity(wimLease.SafeFileHandle, wim);
                            toolLease = new(tool.Path, FileMode.Open, FileAccess.Read, FileShare.Read);
                            try
                            {
                                toolHash = Hash(tool.Path);
                                if (expectedToolHash != null && toolHash != expectedToolHash) throw new IOException("Servicing executable changed before its lease was acquired.");
                                if (FileVersionInfo.GetVersionInfo(tool.Path).FileVersion is string actualVersion && actualVersion != tool.Version && testRunner == null) throw new IOException("Servicing executable version changed.");
                                string recoveryDirectory = testRunner != null && fixtureRecoveryRoot != null ? fixtureRecoveryRoot : RecoveryRoot;
                                CheckPath(recoveryDirectory); Directory.CreateDirectory(recoveryDirectory); CheckPath(recoveryDirectory);
                                recovery = Path.Combine(recoveryDirectory, control.RunId.ToString("N") + ".pending.json");
                                using var journal = new FileStream(recovery, FileMode.CreateNew, FileAccess.Write, FileShare.None);
                                JsonSerializer.Serialize(journal, new { control.RunId, Root = root, Wim = wim, WimIdentity = wimIdentity, Index = index, Mount = mount, MountIdentity = mountIdentity, Tool = tool, ToolSha256 = toolHash, Status = "Mount intent; retain files until verified detached" }, Json);
                                journal.Flush(true);
                            }
                            catch { toolLease.Dispose(); throw; }
                        }
                        catch { wimLease.Dispose(); throw; }
                    }
                    catch { scratchLease.Dispose(); throw; }
                }
                catch { mountLease.Dispose(); throw; }
            }
            catch { rootLease.Dispose(); throw; }
        }

        void VerifyPaths()
        {
            CheckPath(root); CheckPath(wim); CheckPath(mount); CheckPath(scratch); CheckPath(tool.Path);
            if (CleanupNative.DirectoryIdentity(root) != rootIdentity || CleanupNative.DirectoryIdentity(mount) != mountIdentity || CleanupNative.DirectoryIdentity(scratch) != scratchIdentity || Hash(tool.Path) != toolHash) throw new IOException("Servicing resource identity changed.");
            using var file = File.OpenRead(wim);
            if (CleanupNative.CreatedIdentity(file.SafeFileHandle, wim) != wimIdentity) throw new IOException("Servicing WIM identity changed.");
        }
        async Task<string> Command(string phase, string[] arguments, bool checkpoint = true)
        {
            if (checkpoint) control.Checkpoint(phase, progress);
            VerifyPaths();
            if(arguments.Contains("/Mount-Image")||arguments.Contains("/Remove-ProvisionedAppxPackage")||arguments.Contains("/StartComponentCleanup")||arguments.Contains("/Commit"))spaceCheck?.Invoke();
            var args = new List<string> { "/English", "/NoRestart", "/ScratchDir:" + scratch, "/LogPath:" + Path.Combine(root, "dism-" + serial + ".log") };
            args.AddRange(arguments);
            ValidateDismArguments(args);
            JobControl.Publish(progress, new(phase, "DISM is running. Pause/cancel requests wait for its actual exit; it is never forcibly killed.", CancellationPending: control.CancellationRequested));
            var start = new ProcessStartInfo(tool.Path) { UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
            foreach (string arg in args) start.ArgumentList.Add(arg);
            ProcessOutput observed;
            if (testRunner != null) observed = await testRunner(args.ToArray());
            else using (var process = Process.Start(start) ?? throw new IOException("Could not start Microsoft DISM.")) observed = await ObserveProcessAsync(process, TimeSpan.FromHours(3));
            int id = serial++;
            write("dism-" + id + ".stdout.txt", observed.Stdout.Text); write("dism-" + id + ".stderr.txt", observed.Stderr.Text);
            write("dism-" + id + ".result.json", JsonSerializer.Serialize(new { Executable = tool.Path, ToolSha256 = toolHash, Arguments = args, observed.ExitCode, observed.ExceededBudget, StdoutTruncated = observed.Stdout.Truncated, StderrTruncated = observed.Stderr.Truncated }, Json));
            VerifyObservedQuery(observed); // Native failures always outrank pending cancellation.
            if (Regex.IsMatch(observed.Stdout.Text, @"(?im)^(?:Restart Needed|Restart Required|Reboot Required)\s*:\s*(?:Yes|True)\s*$")) throw new IOException("DISM reported a pending restart; this output is not approved.");
            VerifyPaths();
            if (checkpoint) control.Checkpoint(phase, progress);
            return observed.Stdout.Text;
        }
        async Task VerifyAssociation(bool present)
        {
            string text = await Command("Checking owned image mount", ["/Get-MountedWimInfo"], false);
            var mounts = ParseMountedImages(text);
            var matching = mounts.Where(x => x.Mount.Equals(mount, StringComparison.OrdinalIgnoreCase)).ToArray();
            if (present ? matching.Length != 1 || !matching[0].Wim.Equals(wim, StringComparison.OrdinalIgnoreCase) || matching[0].Index != index || matching[0].Status != "Ok" : matching.Length != 0)
                throw new IOException("Owned mount association could not be proven. Keep files for manual recovery.");
        }
        public async Task Mount(bool readOnly)
        {
            await VerifyAssociation(false);
            control.Checkpoint("Mounting private WIM copy", progress);
            mountAttempted = true;
            var args = new List<string> { "/Mount-Image", "/ImageFile:" + wim, "/Index:" + index, "/MountDir:" + mount, "/CheckIntegrity" };
            if (readOnly) args.Add("/ReadOnly");
            await Command("Mounting private WIM copy", args.ToArray(), false);
            await VerifyAssociation(true);
            control.Checkpoint("Private WIM mounted", progress);
        }
        public async Task<ProvisionedApp[]> Apps() => ParseProvisionedApps(await Command("Reading provisioned apps", ["/Image:" + mount, "/Get-ProvisionedAppxPackages"]));
        public Task Remove(string name) => Command("Removing app provisioning", ["/Image:" + mount, "/Remove-ProvisionedAppxPackage", "/PackageName:" + name]);
        public Task Cleanup() => Command("Cleaning replaced components", ["/Image:" + mount, "/Cleanup-Image", "/StartComponentCleanup"]);
        public void CheckPendingActions()
        {
            string pending = Path.Combine(mount,"Windows","WinSxS","pending.xml");
            CheckPath(pending);
            if (File.Exists(pending)) throw new IOException("Pending component servicing actions detected. No commit is approved.");
        }
        public async Task Health()
        {
            CheckPendingActions();
            string text = await Command("Checking component-store health", ["/Image:" + mount, "/Cleanup-Image", "/CheckHealth"]);
            if (!text.Contains("No component store corruption detected.", StringComparison.Ordinal)) throw new IOException("Healthy component-store status was not proven.");
        }
        public async Task Finish(bool commit)
        {
            if (!mountAttempted) { detached = true; return; }
            await VerifyAssociation(true);
            if (commit) { CheckPendingActions(); control.Checkpoint("Committing customized edition", progress); }
            var arguments = new List<string> { "/Unmount-Image", "/MountDir:" + mount, commit ? "/Commit" : "/Discard" };
            if (commit) arguments.Add("/CheckIntegrity");
            await Command(commit ? "Committing and unmounting edition" : "Final cleanup: discarding owned mount", arguments.ToArray(), false);
            await VerifyAssociation(false); detached = true;
        }
        public async Task Release()
        {
            try
            {
                if (!detached) await Finish(false);
                if (!detached) throw new IOException("Mount release was not proven.");
                // Keep an immutable terminal record. A marker alone never authorizes mount cleanup.
                File.Move(recovery, recovery.Replace(".pending.json", ".detached.json", StringComparison.Ordinal), false);
            }
            catch (Exception ex) { RequireRestart("Servicing cleanup uncertain: " + ex.Message); throw; }
            finally { toolLease.Dispose(); wimLease.Dispose(); scratchLease.Dispose(); mountLease.Dispose(); rootLease.Dispose(); }
        }
    }
    public sealed record MountedImage(string Mount, string Wim, int Index, string Status);
    public static MountedImage[] ParseMountedImages(string text)
    {
        if (text.Length > 4 * 1024 * 1024 || !text.Contains("The operation completed successfully.", StringComparison.Ordinal)) throw new InvalidDataException("Incomplete mounted-image observation.");
        if (!text.Contains("Mounted images:", StringComparison.Ordinal)) throw new InvalidDataException("Missing mounted-image inventory header.");
        var records = new List<MountedImage>();
        var blocks = Regex.Split(text, @"(?m)(?=^Mount Dir\b)" ).Skip(1);
        foreach (string block in blocks)
        {
            string mount = One(block, "Mount Dir"), wim = One(block, "Image File"), status = One(block, "Status");
            if (!int.TryParse(One(block, "Image Index"), out int index) || index <= 0 || !Path.IsPathFullyQualified(mount) || !Path.IsPathFullyQualified(wim)) throw new InvalidDataException("Invalid mounted-image identity.");
            records.Add(new(Path.GetFullPath(mount), Path.GetFullPath(wim), index, status));
        }
        if (records.Select(x => x.Mount).Distinct(StringComparer.OrdinalIgnoreCase).Count() != records.Count) throw new InvalidDataException("Duplicate mounted-image records.");
        if (new[]{"Mount Dir","Image File","Image Index","Status"}.Any(label => Regex.Matches(text, @"(?m)^" + Regex.Escape(label) + @"\b").Count != records.Count)) throw new InvalidDataException("Mounted-image records were not fully parsed.");
        return records.ToArray();
    }
    internal static void ValidateDismArguments(IEnumerable<string> arguments)
    {
        string[] flags = ["/English", "/NoRestart", "/Get-MountedWimInfo", "/Mount-Image", "/CheckIntegrity", "/ReadOnly", "/Get-ProvisionedAppxPackages", "/Remove-ProvisionedAppxPackage", "/Cleanup-Image", "/StartComponentCleanup", "/CheckHealth", "/Unmount-Image", "/Commit", "/Discard"];
        string[] values = ["/ScratchDir:", "/LogPath:", "/ImageFile:", "/Index:", "/MountDir:", "/Image:", "/PackageName:"];
        foreach (string arg in arguments)
        {
            if (arg.Contains('\n') || arg.Contains('\r') || arg.Contains('*') || arg.Contains('?') ||
                !(flags.Contains(arg, StringComparer.Ordinal) || values.Any(prefix => arg.StartsWith(prefix, StringComparison.Ordinal) && arg.Length > prefix.Length)))
                throw new InvalidDataException("DISM argument is outside the fixed servicing allowlist.");
            if (arg.StartsWith("/PackageName:", StringComparison.Ordinal) && !DescribeApp(arg[13..]).Removable) throw new InvalidDataException("Package identity is not approved for removal.");
        }
    }
}
