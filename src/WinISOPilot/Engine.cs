using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace ImageGuide;
public sealed record Edition(int Index,string Name,string Architecture,string Version,int Build,string Installation,string? EditionId=null,List<string>? Languages=null);
public sealed record Tool(string Kind,string Path,string Version);
public sealed record Inspection(int SchemaVersion,string Source,string SourceSha256,string CompletedUtc,string Cleanup,string Ownership,List<Edition> Editions,List<Tool> Tools,string Compatibility,string Coverage,string MetadataProvider="DISM Get-WimInfo");
public sealed class DismQueryException(int exitCode,string message):IOException(message){public int ExitCode{get;}=exitCode;}
public sealed record Choices(int SelectedIndex=1,bool Updates=false,bool Drivers=false,bool Languages=false,bool Apps=false,bool Setup=false,bool WinRE=false);
public static partial class Engine
{
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    [return:MarshalAs(UnmanagedType.Bool)]
    static extern bool CreateDirectoryW(string path,IntPtr attributes);
    public static readonly JsonSerializerOptions Json=new(){WriteIndented=true,PropertyNameCaseInsensitive=true};
    public static string ValidateSource(string path)
    {
        if(string.IsNullOrWhiteSpace(path)||!System.IO.Path.IsPathFullyQualified(path)||path.StartsWith(@"\\",StringComparison.Ordinal)||path.Contains("://",StringComparison.Ordinal))throw new InvalidDataException("Select an absolute local ISO path; UNC, URI and device paths are unsupported.");
        path=System.IO.Path.GetFullPath(path);
        if(path[2..].Contains(':'))throw new InvalidDataException("Alternate data streams are unsupported.");
        if(!path.EndsWith(".iso",StringComparison.OrdinalIgnoreCase)||!File.Exists(path))throw new InvalidDataException("Source must be an existing .iso file.");
        CheckPath(path);
        return path;
    }
    public static void CheckPath(string path)
    {
        for(string? p=path;p!=null;p=System.IO.Path.GetDirectoryName(p))
            if((File.Exists(p)||Directory.Exists(p))&&(File.GetAttributes(p)&FileAttributes.ReparsePoint)!=0)throw new InvalidDataException("Reparse points are unsupported: "+p);
    }
    public static string NewFolder(string path)
    {
        if(!System.IO.Path.IsPathFullyQualified(path)||path.StartsWith(@"\\",StringComparison.Ordinal))throw new InvalidDataException("Output must be an absolute local directory.");
        path=System.IO.Path.GetFullPath(path);CheckPath(path);
        if(File.Exists(path)||Directory.Exists(path))throw new IOException("Output already exists; choose a NEW directory.");
        string? parent=System.IO.Path.GetDirectoryName(path);
        if(parent==null||!Directory.Exists(parent))throw new IOException("Output parent directory must already exist.");
        // Native exclusive directory creation fails if another operation already created it.
        if(!CreateDirectoryW(path,IntPtr.Zero))throw new IOException("Cannot atomically create output directory; Win32 error "+Marshal.GetLastWin32Error());
        using var marker=new FileStream(System.IO.Path.Combine(path,".imageguide-owned"),FileMode.CreateNew,FileAccess.Write,FileShare.None);
        return path;
    }
    public static string Hash(string path)=>HashControlled(path,null,null,"Hashing source");
    internal static string HashControlled(string path,JobControl? control,IProgress<JobProgress>? progress,string phase,bool interruptible=true)
    {
        using var stream=File.OpenRead(path);using var hash=IncrementalHash.CreateHash(HashAlgorithmName.SHA256);var buffer=new byte[1024*1024];long completed=0,total=stream.Length;
        while(true){if(interruptible)control?.Checkpoint(phase,progress,completed,total,true);JobControl.Publish(progress,new(phase,!interruptible?"Integrity verification must finish before cancellation can complete.":"Reading source identity",completed,total,false,control?.CancellationRequested??false),true);int count=stream.Read(buffer);if(count==0)break;hash.AppendData(buffer,0,count);completed+=count;}
        JobControl.Publish(progress,new(phase,"Source hash complete",completed,total,false,control?.CancellationRequested??false));return Convert.ToHexString(hash.GetHashAndReset());
    }
    public sealed record BoundedOutput(string Text,bool Truncated,long TotalCharacters);
    public sealed record ProcessOutput(BoundedOutput Stdout,BoundedOutput Stderr,int ExitCode,bool ExceededBudget);
    internal static async Task<BoundedOutput> DrainBoundedAsync(TextReader reader,int maxCharacters=4*1024*1024)
    {
        var text=new System.Text.StringBuilder();var buffer=new char[8192];long total=0;int count;
        while((count=await reader.ReadAsync(buffer.AsMemory()))!=0){total+=count;int retain=Math.Min(count,Math.Max(0,maxCharacters-text.Length));if(retain>0)text.Append(buffer,0,retain);}
        return new(text.ToString(),total>maxCharacters,total);
    }
    internal static async Task<ProcessOutput> ObserveProcessAsync(Process process,TimeSpan budget)
    {
        var stdout=DrainBoundedAsync(process.StandardOutput);var stderr=DrainBoundedAsync(process.StandardError);bool late=false;
        try{try{await process.WaitForExitAsync().WaitAsync(budget);}catch(TimeoutException){late=true;}}
        finally{await process.WaitForExitAsync();await Task.WhenAll(stdout,stderr);}
        return new(await stdout,await stderr,process.ExitCode,late);
    }
    internal static void VerifyObservedQuery(ProcessOutput observed)
    {
        if(observed.Stdout.Truncated||observed.Stderr.Truncated)throw new IOException("DISM output exceeded retained 8MiB per stream; drained fully, truncated output rejected.");
        if(observed.ExceededBudget)throw new IOException("DISM query exceeded budget; actual exit and drains were observed.");
        if(observed.ExitCode!=0)throw new DismQueryException(observed.ExitCode,"DISM query failed with exit code "+observed.ExitCode+"; see raw output.");
    }
    public static List<Tool> Discover()
    {
        var result=new List<Tool>();
        Add("Inbox DISM",System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows),"System32","dism.exe"));
        var roots=new HashSet<string>(StringComparer.OrdinalIgnoreCase){System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86),"Windows Kits","10")};
        foreach(var view in new[]{RegistryView.Registry64,RegistryView.Registry32})
        {
            using var registry=RegistryKey.OpenBaseKey(RegistryHive.LocalMachine,view);
            using var key=registry.OpenSubKey(@"SOFTWARE\Microsoft\Windows Kits\Installed Roots");
            if(key?.GetValue("KitsRoot10") is string root)roots.Add(root);
        }
        foreach(var root in roots)foreach(var pair in new[]{("ADK DISM",@"DISM\dism.exe"),("Oscdimg",@"Oscdimg\oscdimg.exe")})
            Add(pair.Item1,System.IO.Path.Combine(root,"Assessment and Deployment Kit","Deployment Tools","amd64",pair.Item2));
        return result;
        void Add(string kind,string path){if(File.Exists(path)){CheckPath(path);if(!result.Any(t=>t.Path.Equals(path,StringComparison.OrdinalIgnoreCase)))result.Add(new(kind,path,FileVersionInfo.GetVersionInfo(path).FileVersion??"Unknown"));}}
    }
    public static string Compatibility(int build,IEnumerable<Tool> tools,string architecture="x64",string installation="Client")
    {
        if(installation!="Client"||architecture is not("x64" or "arm64")||RuntimeInformation.ProcessArchitecture!=Architecture.X64)return "Unknown: unsupported image architecture, installation type or host architecture";
        var tool=tools.FirstOrDefault(t=>t.Kind=="ADK DISM");
        if(tool==null)return "Unknown: ADK DISM not found; future servicing not approved";
        var match=Regex.Match(tool.Version,@"^10\.0\.(\d+)\.");
        if(!match.Success)return "Unknown: tool version unrecognized";
        int branch=int.Parse(match.Groups[1].Value);
        if(branch==26100&&(build==26100||build==26200))return "Documented profile: ADK 26100 / Windows 11 24H2 or 25H2; servicing remains unimplemented";
        if(branch==28000&&build==28000)return "Documented profile: ADK 28000 / Windows 11 26H1; servicing remains unimplemented";
        return "Unknown: no validated servicing profile for image build "+build+" and ADK branch "+branch;
    }
    static string One(string text,string label)
    {
        var matches=Regex.Matches(text,"(?m)^"+Regex.Escape(label)+@"\s*:\s*(.*?)\s*$");
        if(matches.Count!=1||string.IsNullOrWhiteSpace(matches[0].Groups[1].Value))throw new InvalidDataException("Missing or duplicate English DISM field: "+label);
        return matches[0].Groups[1].Value.Trim();
    }
    public static Edition ParseEdition(string text,int expected)
    {
        if(!int.TryParse(One(text,"Index"),out int index)||index!=expected||index<1)throw new InvalidDataException("Unexpected image index.");
        string architecture=One(text,"Architecture").ToLowerInvariant();
        if(architecture is not("x64" or "arm64"))throw new InvalidDataException("Unsupported Windows 11 architecture: "+architecture);
        string installation=One(text,"Installation");
        if(installation!="Client")throw new InvalidDataException("Only Windows client image metadata is supported; server/unknown installation rejected.");
        string version=One(text,"Version");
        if(!System.Version.TryParse(version,out var v)||v.Major!=10||v.Minor!=0||v.Build<22000)throw new InvalidDataException("Image metadata is not a recognized Windows 11 build.");
        return new(index,One(text,"Name"),architecture,version,v.Build,installation);
    }
    public static bool RestartRequired=>Volatile.Read(ref restartRequired)!=0;
    static int restartRequired;static string? restartReason;
    public static string? RestartReason=>restartReason;
    static void RequireRestart(string reason){restartReason=reason;Interlocked.Exchange(ref restartRequired,1);}
    public static Task<Inspection> Inspect(string source,string output,JobControl? control=null,IProgress<JobProgress>? progress=null)=>Task.Run(()=>
    {
        using var ownedControl=control==null?new JobControl():null;control??=ownedControl!;
        if(RestartRequired)throw new IOException("Application restart required after unresolved cleanup/integrity failure: "+restartReason);
        using var mutex=new Mutex(false,@"Local\ImageGuide-ReadOnlyInspection-v1");bool acquired=false;
        try{try{acquired=mutex.WaitOne(0);}catch(AbandonedMutexException){acquired=true;}
            if(!acquired)throw new IOException("Another WinISOPilot inspection is active. Wait for its completion.");
            return InspectCore(source,output,control,progress).GetAwaiter().GetResult();
        }finally{if(acquired)mutex.ReleaseMutex();}
    });
    static async Task<Inspection> InspectCore(string source,string output,JobControl control,IProgress<JobProgress>? progress)
    {
        control.Checkpoint("Starting",progress);source=ValidateSource(source);string folder=NewFolder(output);
        var state=BeginInspectionRun(folder,source,control.RunId);string? hash=null;var editions=new List<Edition>();var tools=new List<Tool>();
        Exception? failure=null;string metadataProvider="DISM Get-WimInfo";var wimObservations=new List<WimObservation>();var native=new NativeIso(source);
        async Task Save(string name,object value){await File.WriteAllTextAsync(System.IO.Path.Combine(folder,name),JsonSerializer.Serialize(value,Json));state.Track(name);}
        try {
            control.Checkpoint("Hashing source",progress);state.BeforeHashStarted=true;
            hash=HashControlled(source,control,progress,"Hashing source");state.BeforeHashComplete=true;
            await Save("source-identity-before.json",new{Source=source,Sha256=hash,Bytes=new FileInfo(source).Length});
            tools=Discover();var dism=tools.FirstOrDefault(t=>t.Kind=="ADK DISM")??tools.FirstOrDefault(t=>t.Kind=="Inbox DISM")??throw new FileNotFoundException("No DISM available for read-only query.");
            control.Checkpoint("Attaching ISO",progress);state.AttachAttempted=true;
            JobControl.Publish(progress,new("Attaching ISO","Native read-only call cannot be interrupted; pause/cancel will wait for its return.",CancellationPending:control.CancellationRequested));
            native.AttachOrReuse();
            string sources=System.IO.Path.Combine(native.DriveRoot!,"sources");var payloads=new[]{"install.wim","install.esd"}.Select(name=>System.IO.Path.Combine(sources,name)).Where(File.Exists).ToArray();
            if(Directory.EnumerateFiles(sources,"install*.swm").Any())throw new InvalidDataException("Split SWM media is unsupported.");
            if(payloads.Length!=1)throw new InvalidDataException("Expected exactly one install.wim or install.esd.");string payload=payloads[0];
            async Task<string> Query(int? index) {
                control.Checkpoint("Querying editions",progress);native.VerifyAssociation();
                string stem=index.HasValue?"index-"+index.Value:"editions";var args=new List<string>{"/English","/Get-WimInfo","/WimFile:"+payload};
                if(index.HasValue){if(index.Value<1)throw new InvalidDataException("Index must be positive.");args.Add("/Index:"+index.Value);}
                string log=stem+".dism.log";args.Add("/LogPath:"+System.IO.Path.Combine(folder,log));
                var info=new ProcessStartInfo(dism.Path){UseShellExecute=false,RedirectStandardOutput=true,RedirectStandardError=true,CreateNoWindow=true};foreach(string arg in args)info.ArgumentList.Add(arg);
                JobControl.Publish(progress,new("Querying editions","DISM query running; pause/cancel is deferred until child exit and stream drain.",CancellationPending:control.CancellationRequested));
                using var process=Process.Start(info)??throw new IOException("Cannot start DISM query.");
                ProcessOutput observed;try{observed=await ObserveProcessAsync(process,TimeSpan.FromMinutes(10));}finally{state.TrackIfExists(log);}
                await File.WriteAllTextAsync(System.IO.Path.Combine(folder,stem+".stdout.txt"),observed.Stdout.Text);state.Track(stem+".stdout.txt");
                await File.WriteAllTextAsync(System.IO.Path.Combine(folder,stem+".stderr.txt"),observed.Stderr.Text);state.Track(stem+".stderr.txt");
                await Save(stem+".result.json",new{Executable=dism.Path,Arguments=args,observed.ExitCode,observed.ExceededBudget,StdoutTruncated=observed.Stdout.Truncated,StderrTruncated=observed.Stderr.Truncated,StdoutCharacters=observed.Stdout.TotalCharacters,StderrCharacters=observed.Stderr.TotalCharacters,RetainedLimitBytesPerStream=8*1024*1024});
                VerifyObservedQuery(observed);control.Checkpoint("Between query results",progress);
                native.VerifyAssociation();return observed.Stdout.Text;
            }
            try {
                string list=await Query(null);var indices=Regex.Matches(list,@"(?m)^Index\s*:\s*([1-9][0-9]*)\s*$").Select(m=>int.Parse(m.Groups[1].Value)).ToArray();
                if(indices.Length==0||indices.Distinct().Count()!=indices.Length)throw new InvalidDataException("Missing or duplicate edition indices.");
                foreach(int index in indices){control.Checkpoint("Between editions",progress);editions.Add(ParseEdition(await Query(index),index));}
            }catch(DismQueryException ex) when(ex.ExitCode==740) {
                control.Checkpoint("Reading WIM metadata",progress);native.VerifyAssociation();
                JobControl.Publish(progress,new("Reading WIM metadata","Inbox WIMGAPI query-only call; pause/cancel waits for its return.",CancellationPending:control.CancellationRequested));
                try{editions=WimMetadata.Read(payload,folder,wimObservations);}finally{state.TrackIfExists("wim-container.xml");}
                native.VerifyAssociation();control.Checkpoint("Between metadata results",progress);metadataProvider="Inbox WIMGAPI QueryOnly; DISM returned740 (not elevated)";
            }
            if(editions.Count==0||editions.Select(e=>e.Build).Distinct().Count()!=1||editions.Select(e=>e.Architecture).Distinct().Count()!=1)throw new InvalidDataException("Inconsistent edition metadata.");
        }catch(Exception ex){failure=ex;if(state.BeforeHashStarted&&!state.BeforeHashComplete&&ex is not OperationCanceledException)RequireRestart("Initial source hash failed.");}
        finally {
            JobControl.Publish(progress,new("Releasing ISO handle","Native lifetime cleanup must finish before pause/cancel completes.",CancellationPending:control.CancellationRequested));
            try{native.Dispose();state.NativeFinished=!state.AttachAttempted||native.Disposition=="HandleReleased"||native.Ownership=="NotOpened";}
            catch(Exception ex){failure=new IOException("Native lifetime release failed.",ex);state.CleanupRequired=true;RequireRestart(failure.Message);}
            bool cleanupRequired=wimObservations.Any(o=>o.Operation=="ModuleRetained"||((o.Operation=="WIMCloseHandle"||o.Operation=="FreeLibrary")&&o.ReturnValue=="False")||(o.Operation=="LocalFree(XML buffer)"&&o.ReturnValue.StartsWith("non-NULL",StringComparison.Ordinal)));
            state.CleanupRequired|=cleanupRequired||!state.NativeFinished;if(state.CleanupRequired)RequireRestart("Native resource cleanup unconfirmed.");
            try{await Save("native-result.json",new{SchemaVersion=1,Source=source,MountOwnership=native.Ownership,Disposition=native.Disposition,native.PostReleasePhysicalPath,Statuses=native.Statuses,WimObservations=wimObservations,CleanupRequired=state.CleanupRequired,MetadataProvider=metadataProvider,Error=failure?.Message,CancellationPending=control.CancellationRequested,AttachmentNotice="Application handle released after all query completion; no explicit detach API."});}catch(Exception ex){failure=new IOException("Native evidence could not be saved.",failure??ex);}
            if(state.BeforeHashComplete){
                JobControl.Publish(progress,new("Verifying source identity","Uncancelable final source hash; cancellation remains pending until integrity is known.",CancellationPending:control.CancellationRequested));
                try{string after=HashControlled(source,control,progress,"Verifying source identity",false);state.AfterHashComplete=true;state.HashMatches=after==hash;await Save("source-identity-after.json",new{Source=source,Sha256=after,MatchesBefore=after==hash,Bytes=new FileInfo(source).Length});if(after!=hash){failure=new InvalidDataException("Source identity changed; inspection rejected.",failure);RequireRestart(failure.Message);}}
                catch(Exception ex){failure=new IOException("Final source identity verification failed.",failure??ex);RequireRestart(failure.Message);}
            }
        }
        if(failure==null){try{control.FinalCheckpoint("Finalizing",progress);}catch(OperationCanceledException ex){failure=ex;}}
        if(failure!=null){
            state.Terminal=failure is OperationCanceledException?"Canceled":"Failed";
            try{await Save("job-result.json",new{RunId=control.RunId,Status=state.Terminal,Error=failure.Message,RestartRequired,BeforeHashComplete=state.BeforeHashComplete,AfterHashComplete=state.AfterHashComplete,HashMatches=state.HashMatches});}catch(Exception ex){state.Terminal="Failed";state.Finished=true;throw new IOException("Terminal job evidence could not be saved.",ex);}state.Freeze();state.Finished=true;
            if(failure is OperationCanceledException){JobControl.Publish(progress,new("Canceled","Canceled after safe native completion and required integrity verification."));throw new OperationCanceledException("Canceled safely. Run folder: "+folder,failure);}
            throw new IOException(failure.Message+" Run folder: "+folder,failure);
        }
        if(native.Disposition!="HandleReleased")throw new InvalidDataException("Incomplete native lifetime record.");
        var inspection=new Inspection(1,source,hash!,DateTimeOffset.UtcNow.ToString("O"),native.Disposition,native.Ownership,editions,tools,Compatibility(editions[0].Build,tools,editions[0].Architecture,editions[0].Installation),"Edition metadata only; no package/driver/app inventories, WIM mount or servicing.",metadataProvider);
        await Save("inspection.json",inspection);state.Terminal="Complete";
        await Save("job-result.json",new{RunId=control.RunId,Status="Complete",RestartRequired});state.Freeze();state.Finished=true;
        JobControl.Publish(progress,new("Complete","Inspection complete; no servicing performed."));return inspection;
    }

    public static async Task<string> Plan(string inspectionPath,Choices choices,string output)
    {
        CheckPath(inspectionPath);
        var inspection=JsonSerializer.Deserialize<Inspection>(await File.ReadAllTextAsync(inspectionPath),Json)??throw new InvalidDataException("Invalid inspection.");
        if(inspection.SchemaVersion!=1||inspection.Cleanup!="HandleReleased"||inspection.Ownership is not("HandleScopedAttach" or "PreExisting" or "ObservedAfterAttachFailure")||inspection.Editions==null||inspection.Tools==null||!inspection.Editions.Any(e=>e.Index==choices.SelectedIndex)||!Regex.IsMatch(inspection.SourceSha256??"",@"^[A-Fa-f0-9]{64}$"))throw new InvalidDataException("Incomplete or unsupported inspection, attachment state, identity or selected edition.");
        var selected=inspection.Editions.Single(e=>e.Index==choices.SelectedIndex);
        if(selected.Index<1||selected.Build<22000||selected.Architecture is not("x64" or "arm64")||selected.Installation!="Client"||!System.Version.TryParse(selected.Version,out var version)||version.Major!=10||version.Minor!=0||version.Build!=selected.Build)throw new InvalidDataException("Invalid selected edition metadata.");
        string source=ValidateSource(inspection.Source);
        if(!Hash(source).Equals(inspection.SourceSha256,StringComparison.OrdinalIgnoreCase))throw new InvalidDataException("Source hash no longer matches inspection.");
        // JSON is evidence supplied by the caller, not an authenticated native result. Recompute availability.
        var tools=Discover();string compatibility=Compatibility(selected.Build,tools,selected.Architecture,selected.Installation);
        var operations=new List<object>();
        void Add(bool enabled,string name,string scope,string dependencies){if(enabled)operations.Add(new{Name=name,Scope=scope,Dependencies=dependencies,Status="NotImplemented",Executable=false,Verification="NotRun"});}
        Add(choices.Languages,"Language/FOD integration","install image","Matching build/architecture language payloads; release-specific order");
        Add(choices.Updates,"Update integration","install image","Applicable signed packages and prerequisite order");
        Add(choices.Drivers,"Driver integration","install image; boot image if explicitly selected later","Applicable signed INF packages");
        Add(choices.Apps,"Provisioned app choices","install image","App identities and dependency inventory required");
        Add(choices.Setup,"Answer file","Windows Setup configuration passes","Schema validation; secrets excluded from shared presets");
        Add(choices.WinRE,"Recovery image updates","nested WinRE image","Separate recovery servicing profile and packages");
        var plan=new{SchemaVersion=1,CreatedUtc=DateTimeOffset.UtcNow.ToString("O"),Executable=false,Status="PlanningOnly",Source=source,inspection.SourceSha256,Edition=selected,Choices=choices,Compatibility=compatibility,AttachmentDisposition=inspection.Cleanup,AttachmentNotice="Application handle released after query completion; existing/permanent/other-holder attachments may remain. No explicit detach API is called.",Trust="Caller-supplied inspection; source identity rehashed, metadata not cryptographically authenticated",Operations=operations,Verification=new[]{"Source hash checked","Future image inventories not implemented","Future boot/install/OOBE/update/recovery tests not run"},Limitations="No commands can execute this plan. No input packages validated. No build or servicing exists."};
        string folder=NewFolder(output);string file=System.IO.Path.Combine(folder,"plan.json");await File.WriteAllTextAsync(file,JsonSerializer.Serialize(plan,Json));return file;
    }
}
