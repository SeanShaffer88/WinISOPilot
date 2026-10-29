using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;

namespace ImageGuide;

public static partial class Engine
{
    public const string FidoVersion = "1.71";
    public const string FidoSha256 = "6481E40D3CF100C79B281942D0D1C7F6D111397BE296FEC867A56CBE368EBDF9";
    static readonly string[] fidoLanguages = ["English", "English International"];
    public static IReadOnlyList<string> DownloadLanguages => Array.AsReadOnly(fidoLanguages);
    internal static void ValidateFidoSource(Stream source)
    {
        if(Convert.ToHexString(SHA256.HashData(source))!=FidoSha256)throw new IOException("Bundled Fido source changed. Automatic link generation is disabled; use Microsoft's page.");
    }
    internal static string[] FidoArguments(string script, string language)
    {
        if (!fidoLanguages.Contains(language, StringComparer.Ordinal)) throw new InvalidDataException("Choose a listed ISO language.");
        // Fido escapes parentheses. The fixed list excludes other regex metacharacters.
        return ["-NoLogo", "-NoProfile", "-NonInteractive", "-File", script, "-Win", "Windows 11", "-Rel", "Latest", "-Ed", "^Windows 11 Home/Pro/Edu$", "-Lang", "^" + language + "$", "-Arch", "^x64$", "-PlatformArch", "x64", "-GetUrl", "-Locale", "en-US"];
    }
    internal static Uri ParseFidoLink(string stdout, string stderr, int exitCode, bool truncated, bool exceededBudget)
    {
        if (exitCode != 0 && (stderr.Contains("running scripts is disabled", StringComparison.OrdinalIgnoreCase) || stderr.Contains("UnauthorizedAccess", StringComparison.Ordinal)))
            throw new IOException("Windows PowerShell script policy blocked Fido. No policy change was attempted. Use Microsoft's download page and paste its ISO link instead.");
        if (exitCode != 0 || truncated || exceededBudget || stderr.Length != 0 || stdout.Length > 16384 || stdout.Trim().Contains('\n') || stdout.Trim().Contains('\r'))
            throw new IOException("Fido could not generate a usable Microsoft ISO link (exit " + exitCode + "). Use Microsoft's page instead. Provider output is not saved because it may contain expiring tokens.");
        return ValidateMicrosoftIsoLink(stdout.Trim());
    }
    public static Task<Uri> GenerateMicrosoftIsoLink(string language, JobControl control, IProgress<JobProgress>? progress = null) => Task.Run(async () =>
    {
        if (IsAdministrator) throw new InvalidOperationException("Reopen WinISOPilot without administrator privileges for automatic link generation. Offline servicing requires administrator privileges. A Microsoft-page direct link remains available.");
        string script = Path.Combine(AppContext.BaseDirectory, "third_party", "Fido", "Fido.ps1");
        string powershell = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows), "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
        string[] arguments = FidoArguments(script, language); CheckPath(script); CheckPath(powershell);
        using var directoryLease = CleanupNative.LeaseDirectory(Path.GetDirectoryName(script)!);
        using var scriptLease = new FileStream(script, FileMode.Open, FileAccess.Read, FileShare.Read);
        ValidateFidoSource(scriptLease);
        using var toolLease = new FileStream(powershell, FileMode.Open, FileAccess.Read, FileShare.Read);
        control.Checkpoint("Getting Microsoft ISO link", progress);
        var start = new ProcessStartInfo(powershell) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true, StandardOutputEncoding = Encoding.UTF8, StandardErrorEncoding = Encoding.UTF8, WorkingDirectory = Path.GetDirectoryName(script)! };
        foreach (string argument in arguments) start.ArgumentList.Add(argument);
        JobControl.Publish(progress, new("Getting Microsoft ISO link", "Fido 1.71 is contacting Microsoft. Pause/cancel waits for this link-only provider; no ISO or installer runs."));
        using var process = Process.Start(start) ?? throw new IOException("Windows PowerShell could not start the bundled link provider.");
        var observed = await ObserveProcessAsync(process, TimeSpan.FromMinutes(3));
        var result = ParseFidoLink(observed.Stdout.Text, observed.Stderr.Text, observed.ExitCode, observed.Stdout.Truncated || observed.Stderr.Truncated, observed.ExceededBudget);
        control.Checkpoint("Microsoft ISO link ready", progress); return result;
    });
}
