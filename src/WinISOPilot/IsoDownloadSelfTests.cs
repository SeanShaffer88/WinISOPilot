using System.Net;
using System.Net.Http.Headers;

namespace ImageGuide;

public static partial class Engine
{
    sealed class DownloadFixture(Func<HttpRequestMessage, CancellationToken, Task<HttpResponseMessage>> response) : HttpMessageHandler
    {
        public int Requests;
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken token) { Requests++; return response(request, token); }
    }
    sealed class DownloadProgress(Action<JobProgress> action) : IProgress<JobProgress> { public void Report(JobProgress value) => action(value); }
    public static void DownloadSelfTests()
    {
        int passed = 0;
        void Test(string name, Action body) { body(); passed++; Console.WriteLine("PASS " + name); }
        void Check(bool value, string message = "Download assertion failed") { if (!value) throw new Exception(message); }
        void Reject(Action body) { try { body(); } catch (Exception ex) when (ex is IOException or InvalidDataException or OperationCanceledException) { return; } throw new Exception("Expected download refusal."); }
        const string url = "https://software.download.prss.microsoft.com/dbazure/Windows11.iso?token=fixture";
        byte[] data = new byte[70 * 1024]; System.Text.Encoding.ASCII.GetBytes("CD001").CopyTo(data, 32769); data[32774] = 1;
        HttpResponseMessage Binary(byte[]? bytes = null, long? length = null, string type = "application/octet-stream")
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK) { Content = new ByteArrayContent(bytes ?? data) };
            response.Content.Headers.ContentType = new MediaTypeHeaderValue(type);
            if (length != null) response.Content.Headers.ContentLength = length;
            return response;
        }
        using var success = new DownloadFixture((_, _) => Task.FromResult(Binary()));
        string root = Path.Combine(Path.GetTempPath(), "WinISOPilot-download-tests-" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(root);
        try
        {
            string NewPath() => Path.Combine(root, Guid.NewGuid().ToString("N") + ".iso");
            IsoDownloadResult Run(string path, HttpMessageHandler handler, JobControl? control = null, IProgress<JobProgress>? progress = null, string? sha = null)
            {
                using var owned = control == null ? new JobControl() : null;
                return DownloadIsoCore(ValidateMicrosoftIsoLink(url), path, sha, control ?? owned!, progress, handler, 64 * 1024, 128 * 1024).GetAwaiter().GetResult();
            }
            Test("only exact Microsoft HTTPS ISO hosts accepted", () =>
            {
                foreach (string bad in new[] { "http://software.download.prss.microsoft.com/a.iso", "https://software.download.prss.microsoft.com.evil.example/a.iso", "https://user@software.download.prss.microsoft.com/a.iso", "https://software.download.prss.microsoft.com:444/a.iso", "https://software.download.prss.microsoft.com/a.iso#fragment", "https://127.0.0.1/a.iso", "https://software.download.prss.microsoft.com/a.exe" }) Reject(() => ValidateMicrosoftIsoLink(bad));
                Check(ValidateMicrosoftIsoLink(url).Query.Contains("token"));
            });
            Test("native transfer verifies and publishes without overwrite", () =>
            {
                string path = NewPath(); var result = Run(path, success);
                Check(result.Bytes == data.Length && File.ReadAllBytes(path).SequenceEqual(data) && !result.TrustedChecksumMatched);
                Check(result.Sha256 == Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(data)));
                int calls = success.Requests; Reject(() => Run(path, success)); Check(success.Requests == calls, "Existing target reached network.");
            });
            Test("explicit checksum match is distinguished from computed hash", () =>
            {
                string digest = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(data)); Check(Run(NewPath(), success, sha: digest).TrustedChecksumMatched);
                string target = NewPath(); Reject(() => Run(target, success, sha: new string('0', 64))); Check(!File.Exists(target));
            });
            Test("unsafe destinations rejected before network", () =>
            {
                int calls = success.Requests;
                foreach (string path in new[] { "relative.iso", @"\\?\C:\temp\a.iso", Path.Combine(root, "existing:stream.iso"), Path.Combine(root, "NUL.iso"), Path.Combine(root, "trailing. ", "a.iso") }) Reject(() => Run(path, success));
                Check(calls == success.Requests);
            });
            Test("foreign redirect refused before contacting another host", () =>
            {
                using var handler = new DownloadFixture((_, _) => { var r = new HttpResponseMessage(HttpStatusCode.Found); r.Headers.Location = new Uri("https://example.com/a.iso"); return Task.FromResult(r); });
                Reject(() => Run(NewPath(), handler)); Check(handler.Requests == 1);
            });
            Test("redirect loop is bounded", () =>
            {
                using var handler = new DownloadFixture((_, _) => { var r = new HttpResponseMessage(HttpStatusCode.Found); r.Headers.Location = new Uri(url); return Task.FromResult(r); });
                Reject(() => Run(NewPath(), handler)); Check(handler.Requests == 4);
            });
            Test("expired Microsoft link creates no partial file", () =>
            {
                using var handler = new DownloadFixture((_, _) => Task.FromResult(new HttpResponseMessage(HttpStatusCode.Forbidden)));
                int before = Directory.GetFiles(root).Length; Reject(() => Run(NewPath(), handler)); Check(Directory.GetFiles(root).Length == before);
            });
            Test("HTML and out of bound sizes rejected before file creation", () =>
            {
                foreach (var response in new[] { Binary(type: "text/html"), Binary(length: 1), Binary(length: 128 * 1024 + 1) })
                {
                    using var handler = new DownloadFixture((_, _) => Task.FromResult(response));
                    int before = Directory.GetFiles(root).Length; Reject(() => Run(NewPath(), handler)); Check(Directory.GetFiles(root).Length == before);
                }
            });
            Test("short or oversized response cannot publish", () =>
            {
                foreach (long length in new[] { data.Length - 1L, data.Length + 1L })
                {
                    using var handler = new DownloadFixture((_, _) => Task.FromResult(Binary(length: length)));
                    string path = NewPath(); Reject(() => Run(path, handler)); Check(!File.Exists(path));
                }
            });
            Test("binary without ISO descriptor cannot publish", () =>
            {
                using var handler = new DownloadFixture((_, _) => Task.FromResult(Binary(new byte[data.Length])));
                string path = NewPath(); Reject(() => Run(path, handler)); Check(!File.Exists(path));
            });
            Test("UDF descriptor in later volume sector recognized", () =>
            {
                byte[] udf = new byte[70 * 1024]; System.Text.Encoding.ASCII.GetBytes("NSR03").CopyTo(udf, 19 * 2048 + 1); udf[19 * 2048 + 6] = 1;
                Check(HasIsoDescriptor(new MemoryStream(udf)));
            });
            Test("cancellation keeps source and offers only owned unchanged partial", () =>
            {
                using var control = new JobControl(); string path = NewPath();
                var progress = new DownloadProgress(p => { if (p.Phase == "Downloading ISO") control.RequestCancel(); });
                Reject(() => Run(path, success, control, progress)); Check(!File.Exists(path));
                var preview = PreviewDownloadCleanup(control.RunId); Check(preview.CanDelete && preview.Files.Count == 1);
                var cleanup = CleanupDownload(control.RunId); Check(cleanup.Removed.Count == 1 && cleanup.Errors.Count == 0);
            });
            Test("changed partial is retained after cleanup preview", () =>
            {
                using var control = new JobControl(); string path = NewPath();
                Reject(() => Run(path, success, control, new DownloadProgress(p => { if (p.Phase == "Downloading ISO") control.RequestCancel(); })));
                var preview = PreviewDownloadCleanup(control.RunId); Check(preview.CanDelete);
                string partial = Path.Combine(root, preview.Files.Single().RelativePath); File.AppendAllText(partial, "foreign bytes");
                Check(CleanupDownload(control.RunId).Removed.Count == 0 && File.Exists(partial));
            });
            Test("network cancellation drains request without publishing", () =>
            {
                using var control = new JobControl(); using var handler = new DownloadFixture(async (_, token) => { control.RequestCancel(); await Task.Delay(Timeout.Infinite, token); return Binary(); });
                string path = NewPath(); Reject(() => Run(path, handler, control)); Check(!File.Exists(path));
            });
            Test("download pause waits and resume publishes unchanged bytes", () =>
            {
                using var control = new JobControl(); int once = 0; string path = NewPath();
                var progress = new DownloadProgress(p => { if (p.Phase == "Downloading ISO" && Interlocked.Exchange(ref once, 1) == 0) control.RequestPause(); });
                var task = Task.Run(() => Run(path, success, control, progress));
                Check(SpinWait.SpinUntil(() => control.IsPaused, 5000), "Download did not pause."); Check(!File.Exists(path), "Paused download published.");
                control.Resume(); Check(task.GetAwaiter().GetResult().Bytes == data.Length);
            });
            Test("late cancellation cannot publish a completed transfer", () =>
            {
                using var control = new JobControl(); string path = NewPath();
                Reject(() => Run(path, success, control, new DownloadProgress(p => { if (p.Phase == "Verifying downloaded file") control.RequestCancel(); })));
                Check(!File.Exists(path)); Check(PreviewDownloadCleanup(control.RunId).CanDelete);
            });
            Test("destination appearing during download is never replaced", () =>
            {
                string path = NewPath(); int once = 0;
                var progress = new DownloadProgress(p => { if (p.Phase == "Downloading ISO" && Interlocked.Exchange(ref once, 1) == 0) File.WriteAllText(path, "keep foreign destination"); });
                Reject(() => Run(path, success, progress: progress)); Check(File.ReadAllText(path) == "keep foreign destination");
            });
            Test("Fido arguments are fixed link-only unelevated script parameters",()=>
            {
                var args=FidoArguments("pinned.ps1","English");
                Check(args.Contains("-GetUrl")&&args.Contains("-File")&&!args.Contains("-Command")&&!args.Contains("-ExecutionPolicy")&&!args.Contains("-PipeName"));
                Check(args[Array.IndexOf(args,"-Lang")+1]=="^English$");
                Reject(()=>FidoArguments("pinned.ps1","English.*"));
            });
            Test("Fido accepts only one successful Microsoft ISO URL",()=>
            {
                Check(ParseFidoLink(url+"\r\n","",0,false,false).Host=="software.download.prss.microsoft.com");
                Reject(()=>ParseFidoLink(url+"\n"+url,"",0,false,false));
                Reject(()=>ParseFidoLink("https://example.com/file.iso","",0,false,false));
                Reject(()=>ParseFidoLink(url,"",1,false,false));
                Reject(()=>ParseFidoLink(url,"",0,true,false));
                Reject(()=>ParseFidoLink(url,"",0,false,true));
            });
            Test("Fido policy errors are sanitized without token disclosure",()=>
            {
                try {ParseFidoLink("", "UnauthorizedAccess "+url,1,false,false);throw new Exception("Policy refusal expected.");}
                catch(IOException ex){Check(ex.Message.Contains("policy",StringComparison.OrdinalIgnoreCase)&&!ex.Message.Contains("token")&&!ex.Message.Contains(url));}
            });
            Test("bundled provider bytes must match pinned upstream source",()=>
            {
                string script=Path.Combine(AppContext.BaseDirectory,"third_party","Fido","Fido.ps1");
                using var file=File.OpenRead(script); ValidateFidoSource(file);
                Reject(()=>ValidateFidoSource(new MemoryStream([1,2,3])));
            });
            Test("Fido languages cannot be mutated by a caller",()=>
            {
                Check(DownloadLanguages.Count==2&&DownloadLanguages[0]=="English");
                if(DownloadLanguages is IList<string> list){try{list[0]="arbitrary";throw new Exception("Language list mutated.");}catch(NotSupportedException){}}
            });
        }
        finally { Directory.Delete(root, true); }
        Console.WriteLine($"{passed} download fixture tests passed; no real ISO transfer is implied.");
    }
}
