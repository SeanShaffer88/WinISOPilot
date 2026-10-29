using System.Collections.Concurrent;
using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.Win32.SafeHandles;
namespace ImageGuide;

public sealed record MediaBuildChoices(int SelectedIndex,bool PreferEdition=false);
public sealed record MediaBuildResult(Guid RunId,string IsoPath,string SourceSha256,string OutputSha256,string Status,string[] Checks,string[] NotRun);
public static partial class Engine
{
    sealed class BuildRun(string root,string source,Guid id)
    {
        public string Root=root,Source=source;public Guid Id=id;
        public string Terminal="Active";public bool Safe;
        public Dictionary<string,string> Directories=new(StringComparer.OrdinalIgnoreCase);public List<SafeFileHandle> DirectoryLeases=new();
        public HashSet<string> Tracked=new(StringComparer.OrdinalIgnoreCase);
        public Dictionary<string,string> Created=new(StringComparer.OrdinalIgnoreCase),ExpectedHash=new(StringComparer.OrdinalIgnoreCase);
        public Dictionary<string,FileSnapshot> Frozen=new(StringComparer.OrdinalIgnoreCase),Preview=new(StringComparer.OrdinalIgnoreCase);
    }
    static readonly ConcurrentDictionary<Guid,BuildRun> buildRuns=new();
    public static string EditionConfiguration(Edition edition)
    {
        if(edition.Installation!="Client"||edition.EditionId is not("Core" or "Professional"))throw new InvalidDataException("Edition preference supports only observed Windows Home (Core) or Pro (Professional). All editions remain in the ISO.");
        return "[EditionID]\r\n"+edition.EditionId+"\r\n[Channel]\r\nRetail\r\n[VL]\r\n0\r\n";
    }
    internal static string BuildRelative(string root,string path)
    {
        string relative=Path.GetRelativePath(root,path);
        if(Path.IsPathRooted(relative)||relative==".."||relative.StartsWith(".."+Path.DirectorySeparatorChar,StringComparison.Ordinal)||relative.Contains(':')||relative.Split(Path.DirectorySeparatorChar).Any(x=>x.Length==0||x is "." or ".."||x.EndsWith(' ')||x.EndsWith('.')))throw new InvalidDataException("Unsafe media path.");
        return relative;
    }
    internal static void CheckSetupConflicts(string root)
    {
        foreach(string relative in new[]{"sources/ei.cfg","sources/pid.txt","autounattend.xml","unattend.xml","sources/unattend.xml","sources/autounattend.xml"})
            if(File.Exists(Path.Combine(root,relative))||Directory.Exists(Path.Combine(root,relative)))throw new InvalidDataException("Edition preference is blocked by existing setup configuration: "+relative+". No file was replaced; choose no edition preference.");
    }
    static string ValidateMediaLayout(string root)
    {
        foreach(string name in new[]{"bootmgr","efi/boot/bootx64.efi","sources/boot.wim","boot/etfsboot.com","efi/microsoft/boot/efisys.bin"})
            if(!File.Exists(Path.Combine(root,name))||new FileInfo(Path.Combine(root,name)).Length==0)throw new InvalidDataException("Missing required x64 Windows media file: "+name);
        string sources=Path.Combine(root,"sources");if(Directory.EnumerateFiles(sources,"install*.swm").Any())throw new InvalidDataException("Split SWM media is unsupported.");
        var payloads=new[]{"install.wim","install.esd"}.Select(x=>Path.Combine(sources,x)).Where(File.Exists).ToArray();
        if(payloads.Length!=1)throw new InvalidDataException("Exactly one installation WIM or ESD is required.");return payloads[0];
    }
    static void CheckBuildDirectories(BuildRun run)
    {
        foreach(var pair in run.Directories){CheckPath(pair.Key);if(CleanupNative.DirectoryIdentity(pair.Key)!=pair.Value)throw new IOException("Build directory identity changed; retain files and inspect the workspace.");}
    }
    static void BuildDirectory(BuildRun run,string path){CheckPath(Path.GetDirectoryName(path)!);if(Directory.Exists(path)||File.Exists(path))throw new IOException("Build destination already exists.");if(!CreateDirectoryW(path,IntPtr.Zero))throw new IOException("Cannot exclusively create build directory.");run.Directories.Add(path,CleanupNative.DirectoryIdentity(path));run.DirectoryLeases.Add(CleanupNative.LeaseDirectory(path));}
    static void BuildWrite(BuildRun run,string relative,string text)
    {
        CheckBuildDirectories(run);string path=Path.Combine(run.Root,relative);BuildRelative(run.Root,path);run.Tracked.Add(path);
        using var file=new FileStream(path,FileMode.CreateNew,FileAccess.Write,FileShare.None);run.Created[path]=CleanupNative.CreatedIdentity(file.SafeFileHandle,path);byte[] data=Encoding.UTF8.GetBytes(text);file.Write(data);file.Flush(true);run.ExpectedHash[path]=Convert.ToHexString(SHA256.HashData(data));
    }
    static List<Edition> BuildReadMetadata(string payload,string output)
    {
        var observations=new List<WimObservation>();try{return WimMetadata.Read(payload,output,observations);}finally{
            bool failed=observations.Any(o=>o.Operation=="ModuleRetained"||((o.Operation=="WIMCloseHandle"||o.Operation=="FreeLibrary")&&o.ReturnValue=="False")||(o.Operation=="LocalFree(XML buffer)"&&o.ReturnValue.StartsWith("non-NULL",StringComparison.Ordinal)));
            if(failed)RequireRestart("Build WIMGAPI resource cleanup unconfirmed.");
            File.WriteAllText(Path.Combine(output,"native-wim-observations.json"),JsonSerializer.Serialize(observations,Json));
        }
    }
    static void SameEditions(List<Edition> expected,List<Edition> actual)
    {
        if(expected.Count!=actual.Count||expected.Zip(actual).Any(p=>p.First.Index!=p.Second.Index||p.First.EditionId!=p.Second.EditionId||p.First.Version!=p.Second.Version||p.First.Architecture!=p.Second.Architecture||p.First.Installation!=p.Second.Installation))throw new InvalidDataException("Installation edition metadata does not match the inspection.");
    }
    public static Task<MediaBuildResult> BuildIso(Inspection inspection,MediaBuildChoices choices,string output,JobControl? control=null,IProgress<JobProgress>? progress=null)=>Task.Run(()=>
    {
        using var owned=control==null?new JobControl():null;control??=owned!;
        using var mutex=new Mutex(false,@"Local\WinISOPilot-MediaBuild-v1");bool acquired=false;
        try{try{acquired=mutex.WaitOne(0);}catch(AbandonedMutexException){acquired=true;}if(!acquired)throw new IOException("Another ISO build is active.");try{return BuildIsoCore(inspection,choices,output,control,progress).GetAwaiter().GetResult();}finally{if(buildRuns.TryGetValue(control.RunId,out var finished)){foreach(var lease in finished.DirectoryLeases)lease.Dispose();finished.DirectoryLeases.Clear();}}}
        finally{if(acquired)mutex.ReleaseMutex();}
    });
    static async Task<MediaBuildResult> BuildIsoCore(Inspection inspection,MediaBuildChoices choices,string output,JobControl control,IProgress<JobProgress>? progress)
    {
        if(RestartRequired)throw new IOException("Restart required after unresolved resource or integrity state.");
        if(inspection.SchemaVersion!=1||inspection.Cleanup!="HandleReleased"||inspection.Editions==null||inspection.Editions.Count==0||!System.Text.RegularExpressions.Regex.IsMatch(inspection.SourceSha256??"", "^[A-Fa-f0-9]{64}$"))throw new InvalidDataException("A completed inspection is required.");
        var selected=inspection.Editions.SingleOrDefault(x=>x.Index==choices.SelectedIndex)??throw new InvalidDataException("Select an observed edition.");
        if(selected.Architecture!="x64"||selected.Installation!="Client"||selected.Build<22000)throw new InvalidDataException("Media creation currently supports x64 Windows 11 client media only.");
        string? config=choices.PreferEdition?EditionConfiguration(selected):null;
        string source=ValidateSource(inspection.Source);output=Path.GetFullPath(output);if(output.Contains('#'))throw new InvalidDataException("Choose an output folder without #; Oscdimg boot paths use that separator.");
        if(output.Equals(source,StringComparison.OrdinalIgnoreCase))throw new InvalidDataException("Output cannot be the source.");
        var tool=Discover().SingleOrDefault(x=>x.Kind=="Oscdimg")??throw new FileNotFoundException("Install Microsoft's Windows ADK Deployment Tools; Oscdimg was not found.");
        CheckPath(tool.Path);
        using var toolLease=new FileStream(tool.Path,FileMode.Open,FileAccess.Read,FileShare.Read);
        using var lease=new FileStream(source,FileMode.Open,FileAccess.Read,FileShare.Read);
        string hash=HashControlled(source,control,progress,"Hashing source");if(!hash.Equals(inspection.SourceSha256,StringComparison.OrdinalIgnoreCase))throw new InvalidDataException("Original ISO changed since inspection. Inspect it again.");
        string root=NewFolder(output);var run=new BuildRun(root,source,control.RunId);run.Directories.Add(root,CleanupNative.DirectoryIdentity(root));run.DirectoryLeases.Add(CleanupNative.LeaseDirectory(root));buildRuns[run.Id]=run;
        string marker=Path.Combine(root,".imageguide-owned");using(var markerFile=new FileStream(marker,FileMode.Open,FileAccess.Write,FileShare.None)){run.Created[marker]=CleanupNative.CreatedIdentity(markerFile.SafeFileHandle,marker);byte[] markerBytes=Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new{SchemaVersion=1,Purpose="media-build",run.Id,Session=sessionId,Root=root},Json));markerFile.Write(markerBytes);markerFile.SetLength(markerBytes.Length);markerFile.Flush(true);run.ExpectedHash[marker]=Convert.ToHexString(SHA256.HashData(markerBytes));}run.Tracked.Add(marker);
        Exception? failure=null;MediaBuildResult? result=null;string partial=Path.Combine(root,"customized.partial.iso"),final=Path.Combine(root,"customized.iso");bool nativeSafe=true;NativeIso? original=null,created=null;FileStream? outputLease=null;
        void Journal(string phase){BuildWrite(run,"journal-"+run.Tracked.Count+".json",JsonSerializer.Serialize(new{run.Id,Phase=phase,OriginalSha256=hash,FinalOriginalVerification="Pending",Servicing="Not performed",BootAndInstall="NotRun"},Json));}
        try{
            control.Checkpoint("Opening source read-only",progress);original=new NativeIso(source);original.AttachOrReuse();original.VerifyAssociation();string mediaRoot=original.DriveRoot!;
            string payload=ValidateMediaLayout(mediaRoot);if(config!=null)CheckSetupConflicts(mediaRoot);
            string sourceMeta=Path.Combine(root,"source-metadata");BuildDirectory(run,sourceMeta);run.Tracked.Add(Path.Combine(sourceMeta,"wim-container.xml"));SameEditions(inspection.Editions,BuildReadMetadata(payload,sourceMeta));
            var manifest=new List<(string Relative,long Bytes)>();var dirs=new List<string>();var seen=new HashSet<string>(StringComparer.OrdinalIgnoreCase);long total=0;
            void Scan(string directory,int depth){if(depth>32)throw new InvalidDataException("Media nesting exceeds bound.");foreach(string path in Directory.EnumerateFileSystemEntries(directory).OrderBy(x=>x,StringComparer.OrdinalIgnoreCase)){if(manifest.Count+dirs.Count>=100000)throw new InvalidDataException("Media entry count exceeds bound.");CheckPath(path);string relative=BuildRelative(mediaRoot,path);if(!seen.Add(relative))throw new InvalidDataException("Case-colliding media paths.");var attrs=File.GetAttributes(path);if((attrs&FileAttributes.ReparsePoint)!=0)throw new InvalidDataException("Media reparse entry rejected.");if((attrs&FileAttributes.Directory)!=0){dirs.Add(relative);Scan(path,depth+1);}else{long length=new FileInfo(path).Length;total=checked(total+length);manifest.Add((relative,length));}}}
            Scan(mediaRoot,0);if(total<=0)throw new InvalidDataException("Empty media.");
            var drive=new DriveInfo(Path.GetPathRoot(root)!);if(drive.AvailableFreeSpace<checked(total*3+1024L*1024*1024))throw new IOException("Not enough free space. Allow three times the media size plus 1 GB for a working copy, new ISO and safety margin.");
            Journal("Manifest validated");string media=Path.Combine(root,"media");BuildDirectory(run,media);foreach(string dir in dirs)BuildDirectory(run,Path.Combine(media,dir));
            long copied=0;var buffer=new byte[1024*1024];var hashes=new Dictionary<string,string>(StringComparer.OrdinalIgnoreCase);
            foreach(var entry in manifest){control.Checkpoint("Copying media",progress,copied,total);original.VerifyAssociation();CheckBuildDirectories(run);string from=Path.Combine(mediaRoot,entry.Relative),to=Path.Combine(media,entry.Relative);run.Tracked.Add(to);using var input=new FileStream(from,FileMode.Open,FileAccess.Read,FileShare.Read);if(input.Length!=entry.Bytes)throw new IOException("Media source length changed.");using var dest=new FileStream(to,FileMode.CreateNew,FileAccess.Write,FileShare.None);run.Created[to]=CleanupNative.CreatedIdentity(dest.SafeFileHandle,to);using var digest=IncrementalHash.CreateHash(HashAlgorithmName.SHA256);int count;while((count=input.Read(buffer))>0){control.Checkpoint("Copying media",progress,copied,total,true);dest.Write(buffer,0,count);digest.AppendData(buffer,0,count);copied+=count;}dest.Flush(true);string copiedHash=Convert.ToHexString(digest.GetHashAndReset());hashes.Add(entry.Relative,copiedHash);run.ExpectedHash[to]=copiedHash;}
            original.Dispose();original=null;
            control.Checkpoint("Preparing setup preference",progress);if(config!=null){string relative=Path.Combine("media","sources","ei.cfg");BuildWrite(run,relative,config);hashes.Add(Path.Combine("sources","ei.cfg"),Hash(Path.Combine(root,relative)));}Journal(config==null?"Unchanged media copy prepared":"Retail edition preference written only to copy");
            CheckBuildDirectories(run);foreach(var entry in hashes){control.Checkpoint("Checking working copy",progress);if(HashControlled(Path.Combine(media,entry.Key),control,progress,"Checking working copy")!=entry.Value)throw new IOException("Working copy changed before ISO creation.");}
            control.Checkpoint("Creating new ISO",progress);CheckBuildDirectories(run);string boot="-bootdata:2#p0,e,b"+Path.Combine(media,"boot","etfsboot.com")+"#pEF,e,b"+Path.Combine(media,"efi","microsoft","boot","efisys.bin");
            var arguments=new[]{"-m","-o","-u2","-udfver102",boot,media,partial};var start=new ProcessStartInfo(tool.Path){UseShellExecute=false,RedirectStandardOutput=true,RedirectStandardError=true,CreateNoWindow=true};foreach(string arg in arguments)start.ArgumentList.Add(arg);run.Tracked.Add(partial);
            JobControl.Publish(progress,new("Creating new ISO","Oscdimg is running. Pause and cancel wait for its actual exit; it is never forcibly killed.",CancellationPending:control.CancellationRequested));using(var process=Process.Start(start)??throw new IOException("Cannot start Oscdimg.")){var observed=await ObserveProcessAsync(process,TimeSpan.FromHours(2));BuildWrite(run,"oscdimg.stdout.txt",observed.Stdout.Text);BuildWrite(run,"oscdimg.stderr.txt",observed.Stderr.Text);BuildWrite(run,"oscdimg.result.json",JsonSerializer.Serialize(new{Executable=tool.Path,Arguments=arguments,observed.ExitCode,observed.ExceededBudget,StdoutTruncated=observed.Stdout.Truncated,StderrTruncated=observed.Stderr.Truncated},Json));if(observed.ExitCode!=0||observed.ExceededBudget||observed.Stdout.Truncated||observed.Stderr.Truncated)throw new IOException("Oscdimg failed, exceeded its budget, or produced truncated evidence. See the run's logs.");}
            control.Checkpoint("Verifying created ISO",progress);outputLease=new FileStream(partial,FileMode.Open,FileAccess.Read,FileShare.Read);created=new NativeIso(partial);created.AttachOrReuse();created.VerifyAssociation();string builtRoot=created.DriveRoot!;string builtPayload=ValidateMediaLayout(builtRoot);string verify=Path.Combine(root,"output-metadata");BuildDirectory(run,verify);run.Tracked.Add(Path.Combine(verify,"wim-container.xml"));SameEditions(inspection.Editions,BuildReadMetadata(builtPayload,verify));
            var actual=new HashSet<string>(StringComparer.OrdinalIgnoreCase);int verifiedEntries=0;void Verify(string directory,int depth){if(depth>32)throw new InvalidDataException("Output media nesting exceeds bound.");foreach(string path in Directory.EnumerateFileSystemEntries(directory)){if(++verifiedEntries>100000)throw new InvalidDataException("Output media entry count exceeds bound.");CheckPath(path);if(Directory.Exists(path))Verify(path,depth+1);else{string relative=BuildRelative(builtRoot,path);if(!actual.Add(relative)||!hashes.TryGetValue(relative,out var expected)||HashControlled(path,control,progress,"Verifying created ISO")!=expected)throw new IOException("Created ISO file set or content mismatch: "+relative);}}}Verify(builtRoot,0);if(actual.Count!=hashes.Count)throw new IOException("Created ISO is missing copied media files.");
            created.Dispose();created=null;Journal("Created ISO file content and edition inventory verified");
            string outputHash=HashControlled(partial,control,progress,"Hashing created ISO");result=new(run.Id,final,hash,outputHash,"Created; boot and installation not tested",new[]{"Original identity rechecked","All copied file hashes match output","Edition inventory preserved","Standard boot files present",config==null?"No edition preference requested":"Retail edition preference bytes verified"},new[]{"Firmware boot","Windows installation","Setup honoring edition preference","Activation","Updates and recovery"});
        }catch(Exception ex){failure=ex;}
        finally{
            JobControl.Publish(progress,new("Final verification and cleanup","Releasing read-only ISO handles and checking the original; pause/cancel waits.",CancellationPending:control.CancellationRequested));
            foreach(var native in new[]{created,original})if(native!=null){try{native.Dispose();if(native.Disposition!="HandleReleased")throw new IOException("ISO handle release unconfirmed.");}catch(Exception ex){nativeSafe=false;failure=new IOException("ISO cleanup unconfirmed.",ex);RequireRestart(failure.Message);}}
            try{string after=HashControlled(source,null,progress,"Verifying source identity",false);if(after!=hash){failure=new IOException("Original ISO identity changed. Do not use this output.");RequireRestart(failure.Message);}else run.Safe=nativeSafe&&!RestartRequired;}catch(Exception ex){failure=new IOException("Original ISO final verification failed.",ex);RequireRestart(failure.Message);}
        }
        if(failure==null){try{control.FinalCheckpoint("Finalizing build",progress);CheckBuildDirectories(run);BuildWrite(run,"build-result.json",JsonSerializer.Serialize(result,Json));outputLease?.Dispose();outputLease=null;File.Move(partial,final,false);run.Tracked.Remove(partial);run.Tracked.Add(final);run.Terminal="Complete";JobControl.Publish(progress,new("Complete","A separate ISO was created and read back. Boot and installation have not been tested."));return result!;}catch(Exception ex){failure=ex;}}
        outputLease?.Dispose();outputLease=null;run.Terminal=failure is OperationCanceledException?"Canceled":"Failed";
        try{BuildWrite(run,"build-failure.json",JsonSerializer.Serialize(new{run.Id,Status=run.Terminal,Error=failure!.Message,OriginalVerified=run.Safe,RestartRequired},Json));}catch(Exception){run.Safe=false;}
        if(run.Safe&&run.Terminal=="Canceled")foreach(string path in run.Tracked){try{if(File.Exists(path)){var snapshot=CleanupNative.Snapshot(path,false,out _,true);if(run.Created.TryGetValue(path,out var identity)&&identity==snapshot.Identity&&run.ExpectedHash.TryGetValue(path,out var expectedHash)&&snapshot.Sha256==expectedHash)run.Frozen[path]=snapshot;}}catch(Exception){/* Unconfirmed entries retained. */}}
        if(failure is OperationCanceledException)throw new OperationCanceledException("Build canceled safely. Original ISO unchanged. Run folder: "+root,failure);
        throw new IOException("Build did not complete. Run folder: "+root+". "+failure!.Message,failure);
    }
    static BuildRun BuildCleanupRun(string output,Guid id)
    {
        if(RestartRequired||!buildRuns.TryGetValue(id,out var run)||run.Root!=Path.GetFullPath(output)||run.Terminal!="Canceled"||!run.Safe)throw new IOException("Only this session's safely canceled build can be cleaned.");CheckBuildDirectories(run);return run;
    }
    public static CleanupPreview PreviewBuildCleanup(string output,Guid id)
    {
        var files=new List<CleanupFile>();var warnings=new List<string>();try{var run=BuildCleanupRun(output,id);run.Preview.Clear();foreach(string path in run.Tracked){if(!File.Exists(path))continue;try{var now=CleanupNative.Snapshot(path,false,out _,true);if(!run.Frozen.TryGetValue(path,out var expected)||now!=expected){warnings.Add("Retain changed file: "+BuildRelative(run.Root,path));continue;}run.Preview[path]=now;files.Add(new(BuildRelative(run.Root,path),now.Bytes));}catch(Exception ex){warnings.Add("Retain "+BuildRelative(run.Root,path)+": "+ex.Message);}}warnings.Add("Folders and any unexpected files are retained. Counts describe tracked files only. External-tool partial ISO and native diagnostic files are retained when creation identity is not proven. No recursive deletion is used.");return new(files.Count>0,files,files.Sum(x=>x.Bytes),warnings);}catch(Exception ex){warnings.Add(ex.Message);return new(false,files,0,warnings);}
    }
    public static CleanupResult CleanupBuild(string output,Guid id)
    {
        var removed=new List<string>();var retained=new List<string>();var errors=new List<string>();try{var run=BuildCleanupRun(output,id);var preview=run.Preview.ToArray();run.Preview.Clear();if(preview.Length==0)throw new IOException("A matching cleanup preview is required.");foreach(var pair in preview){SafeFileHandle? held=null;try{CheckBuildDirectories(run);var now=CleanupNative.Snapshot(pair.Key,true,out held,true);if(now!=pair.Value){retained.Add(BuildRelative(run.Root,pair.Key));continue;}CleanupNative.DeleteHeld(held!);removed.Add(BuildRelative(run.Root,pair.Key));}catch(Exception ex){retained.Add(BuildRelative(run.Root,pair.Key));errors.Add(ex.Message);}finally{held?.Dispose();}}}catch(Exception ex){errors.Add(ex.Message);}return new(removed,retained,errors,false);
    }
}
