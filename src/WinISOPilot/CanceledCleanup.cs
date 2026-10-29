using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.Win32.SafeHandles;

namespace ImageGuide;
public sealed record CleanupFile(string RelativePath,long Bytes);
public sealed record CleanupPreview(bool CanDelete,List<CleanupFile> Files,long TotalBytes,List<string> Warnings);
public sealed record CleanupResult(List<string> Removed,List<string> Retained,List<string> Errors,bool DirectoryRemoved);
public static partial class Engine
{
    static readonly Guid sessionId=Guid.NewGuid();
    static readonly ConcurrentDictionary<Guid,InspectionRun> inspectionRuns=new();
    static readonly ConcurrentDictionary<Guid,CleanupSnapshot> cleanupPreviews=new();
    sealed record Marker(int SchemaVersion,Guid RunId,string Purpose,string CanonicalOutput,Guid SessionId);
    sealed class InspectionRun(string output,string source,Guid id,string directoryIdentity)
    {
        public string Output{get;}=output;public string Source{get;}=source;public Guid Id{get;}=id;public string DirectoryIdentity{get;}=directoryIdentity;
        public volatile bool BeforeHashStarted,BeforeHashComplete,AfterHashComplete,HashMatches,AttachAttempted,NativeFinished,CleanupRequired,Finished;
        public string Terminal="Active";public string MarkerHash="";readonly HashSet<string> generated=new(StringComparer.OrdinalIgnoreCase);
        public Dictionary<string,FileSnapshot> FinalFiles{get;}=new(StringComparer.OrdinalIgnoreCase);
        public void Freeze(){foreach(string name in Files()){try{FinalFiles[name]=CleanupNative.Snapshot(Path.Combine(Output,name),false,out _);}catch(Exception){/* Unconfirmed final identity is retained, never cleanup-authorized. */}}}
        public void Track(string name){if(name!=Path.GetFileName(name)||name==".imageguide-owned")throw new InvalidDataException("Invalid generated filename tracking.");lock(generated)generated.Add(name);}
        public void TrackIfExists(string name){if(File.Exists(Path.Combine(Output,name)))Track(name);}
        public string[] Files(){lock(generated)return generated.ToArray();}
    }
    sealed record FileSnapshot(string Identity,long Bytes,long LastWrite,string Sha256);
    sealed record CleanupSnapshot(string Output,string DirectoryIdentity,Dictionary<string,FileSnapshot> Files);
    static InspectionRun BeginInspectionRun(string output,string source,Guid runId)
    {
        string identity=CleanupNative.DirectoryIdentity(output);
        var run=new InspectionRun(output,source,runId,identity);
        if(!inspectionRuns.TryAdd(runId,run))throw new IOException("Run identifier already used in this session.");
        string marker=Path.Combine(output,".imageguide-owned");File.WriteAllText(marker,JsonSerializer.Serialize(new Marker(1,runId,"inspection",output,sessionId),Json));run.MarkerHash=Hash(marker);return run;
    }
    static InspectionRun ValidateCleanupRun(string output,Guid runId)
    {
        if(RestartRequired)throw new IOException("Restart required; cleanup disabled due to unresolved integrity/resource state.");
        output=Path.GetFullPath(output);CheckPath(output);
        if(!inspectionRuns.TryGetValue(runId,out var run)||!run.Output.Equals(output,StringComparison.OrdinalIgnoreCase)||run.Source.Equals(output,StringComparison.OrdinalIgnoreCase))throw new IOException("Not an inspection run owned by this application session.");
        if(!run.Finished||run.Terminal!="Canceled"||run.CleanupRequired||!run.NativeFinished)throw new IOException("Only safely completed canceled inspections can be cleaned.");
        if(run.BeforeHashComplete?!(run.AfterHashComplete&&run.HashMatches):run.BeforeHashStarted||run.AttachAttempted)throw new IOException("Source integrity/attachment completion is unresolved; diagnostics must be kept.");
        if(!CleanupNative.DirectoryIdentity(output).Equals(run.DirectoryIdentity,StringComparison.Ordinal))throw new IOException("Output directory identity changed.");
        string markerPath=Path.Combine(output,".imageguide-owned");CheckPath(markerPath);
        using var markerStream=new FileStream(markerPath,FileMode.Open,FileAccess.Read,FileShare.Read);if(markerStream.Length>16384)throw new IOException("Ownership marker exceeds bound.");var markerBytes=new byte[checked((int)markerStream.Length)];markerStream.ReadExactly(markerBytes);
        if(Convert.ToHexString(SHA256.HashData(markerBytes))!=run.MarkerHash)throw new IOException("Ownership marker bytes changed; cleanup refused.");
        var marker=JsonSerializer.Deserialize<Marker>(markerBytes,Json);
        if(marker==null||marker.SchemaVersion!=1||marker.RunId!=runId||marker.Purpose!="inspection"||marker.SessionId!=sessionId||!marker.CanonicalOutput.Equals(output,StringComparison.OrdinalIgnoreCase))throw new IOException("Ownership marker does not match this canceled inspection.");
        return run;
    }
    public static CleanupPreview PreviewCanceledCleanup(string output,Guid runId)
    {
        var files=new List<CleanupFile>();var warnings=new List<string>();
        try{
            var run=ValidateCleanupRun(output,runId);var captured=new Dictionary<string,FileSnapshot>(StringComparer.OrdinalIgnoreCase);var known=run.Files().ToHashSet(StringComparer.OrdinalIgnoreCase);
            foreach(string entry in Directory.EnumerateFileSystemEntries(run.Output).Take(4097)){
                if(captured.Count+warnings.Count>4096)throw new IOException("Too many output entries to preview safely.");string name=Path.GetFileName(entry);if(name==".imageguide-owned")continue;
                if(!known.Contains(name)||Directory.Exists(entry)){warnings.Add("Retain unexpected entry: "+name);continue;}
                try{var snapshot=CleanupNative.Snapshot(entry,false,out _);if(!run.FinalFiles.TryGetValue(name,out var final)||snapshot!=final){warnings.Add("Retain changed/unconfirmed generated file: "+name);continue;}captured.Add(name,snapshot);files.Add(new(name,snapshot.Bytes));}catch(Exception ex){warnings.Add("Retain "+name+": "+ex.Message);}
            }
            cleanupPreviews[runId]=new(run.Output,run.DirectoryIdentity,captured);
            return new(files.Count>0,files,files.Sum(f=>f.Bytes),warnings);
        }catch(Exception ex){cleanupPreviews.TryRemove(runId,out _);warnings.Add(ex.Message);return new(false,files,0,warnings);}
    }
    public static CleanupResult CleanupCanceledJob(string output,Guid runId)
    {
        var removed=new List<string>();var retained=new List<string>();var errors=new List<string>();bool directoryRemoved=false;
        try{
            var run=ValidateCleanupRun(output,runId);
            if(!cleanupPreviews.TryRemove(runId,out var preview)||!preview.Output.Equals(run.Output,StringComparison.OrdinalIgnoreCase)||preview.DirectoryIdentity!=run.DirectoryIdentity)throw new IOException("A matching cleanup preview is required before deletion.");
            foreach(var pair in preview.Files){
                string path=Path.Combine(run.Output,pair.Key);SafeFileHandle? held=null;
                try{var current=CleanupNative.Snapshot(path,true,out held);if(current!=pair.Value){retained.Add(pair.Key+" (changed since preview)");continue;}CleanupNative.DeleteHeld(held!);removed.Add(pair.Key);}
                catch(Exception ex){retained.Add(pair.Key);errors.Add(pair.Key+": "+ex.Message);}finally{held?.Dispose();}
            }
            foreach(string entry in Directory.EnumerateFileSystemEntries(run.Output))if(Path.GetFileName(entry)!=".imageguide-owned")retained.Add(Path.GetFileName(entry));
            if(retained.Count==0){
                // Revalidate session/marker/directory after all file operations. No recursive deletion.
                ValidateCleanupRun(run.Output,runId);string marker=Path.Combine(run.Output,".imageguide-owned");
                SafeFileHandle? markerHandle=null;try{var markerSnapshot=CleanupNative.Snapshot(marker,true,out markerHandle);if(markerSnapshot.Sha256!=run.MarkerHash)throw new IOException("Marker changed; retained.");CleanupNative.DeleteHeld(markerHandle!);}finally{markerHandle?.Dispose();}
                try{Directory.Delete(run.Output,false);directoryRemoved=true;inspectionRuns.TryRemove(runId,out _);}catch(Exception ex){errors.Add("Directory retained: "+ex.Message);}
            }
        }catch(Exception ex){errors.Add(ex.Message);}
        return new(removed,retained.Distinct().ToList(),errors,directoryRemoved);
    }
    static class CleanupNative
    {
        [StructLayout(LayoutKind.Sequential)]struct FileInformation{public uint Attributes,CreationLow,CreationHigh,AccessLow,AccessHigh,WriteLow,WriteHigh,Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;}
        [StructLayout(LayoutKind.Sequential)]struct Disposition{public int DeleteFile;}
        [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern SafeFileHandle CreateFile(string path,uint access,uint share,IntPtr security,uint creation,uint flags,IntPtr template);
        [DllImport("kernel32.dll",SetLastError=true)][return:MarshalAs(UnmanagedType.Bool)]static extern bool GetFileInformationByHandle(SafeFileHandle handle,out FileInformation info);
        [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern uint GetFinalPathNameByHandle(SafeFileHandle handle,StringBuilder path,uint capacity,uint flags);
        [DllImport("kernel32.dll",SetLastError=true)][return:MarshalAs(UnmanagedType.Bool)]static extern bool SetFileInformationByHandle(SafeFileHandle handle,int kind,ref Disposition value,uint size);
        [DllImport("kernel32.dll",SetLastError=true)][return:MarshalAs(UnmanagedType.Bool)]static extern bool ReadFile(SafeFileHandle handle,[Out] byte[] buffer,uint count,out uint read,IntPtr overlapped);
        static FileInformation Info(SafeFileHandle handle){if(!GetFileInformationByHandle(handle,out var info))throw new IOException("Cannot establish file identity; Win32 "+Marshal.GetLastWin32Error());return info;}
        static string Identity(FileInformation i)=>$"{i.Volume:X8}:{i.IndexHigh:X8}{i.IndexLow:X8}";
        static void ExactPath(SafeFileHandle handle,string path){var buffer=new StringBuilder(32768);uint length=GetFinalPathNameByHandle(handle,buffer,(uint)buffer.Capacity,0);if(length==0||length>=buffer.Capacity)throw new IOException("Cannot establish canonical file path.");string actual=buffer.ToString();if(actual.StartsWith(@"\\?\",StringComparison.Ordinal))actual=actual[4..];if(!Path.GetFullPath(path).Equals(actual,StringComparison.OrdinalIgnoreCase))throw new IOException("Canonical file path changed.");}
        public static string DirectoryIdentity(string path){CheckPath(path);using var handle=CreateFile(path,0,7,IntPtr.Zero,3,0x02200000,IntPtr.Zero);if(handle.IsInvalid)throw new IOException("Cannot open output directory identity.");ExactPath(handle,path);var info=Info(handle);if((info.Attributes&0x400)!=0)throw new IOException("Reparse directory rejected.");return Identity(info);}
        public static SafeFileHandle OpenForDelete(string path){CheckPath(path);var handle=CreateFile(path,0x80010000,1,IntPtr.Zero,3,0x00200000,IntPtr.Zero);if(handle.IsInvalid){handle.Dispose();throw new IOException("File is locked or cannot be opened for guarded deletion.");}try{ExactPath(handle,path);var info=Info(handle);if((info.Attributes&(0x400|0x10))!=0||info.Links!=1)throw new IOException("Reparse, directory or hard-linked file rejected.");return handle;}catch{handle.Dispose();throw;}}
        public static FileSnapshot Snapshot(string path,bool keepForDelete,out SafeFileHandle? held){held=null;CheckPath(path);SafeFileHandle handle=keepForDelete?OpenForDelete(path):CreateFile(path,0x80000000,1,IntPtr.Zero,3,0x00200000,IntPtr.Zero);if(handle.IsInvalid){handle.Dispose();throw new IOException("File unavailable or locked.");}bool handed=false;try{ExactPath(handle,path);var info=Info(handle);long bytes=((long)info.SizeHigh<<32)|info.SizeLow;if((info.Attributes&(0x400|0x10))!=0||info.Links!=1||bytes>32*1024*1024)throw new IOException("Unsupported file type/link count/size; retained.");using var hash=IncrementalHash.CreateHash(HashAlgorithmName.SHA256);var buffer=new byte[65536];while(true){if(!ReadFile(handle,buffer,(uint)buffer.Length,out uint read,IntPtr.Zero))throw new IOException("Cannot hash guarded file.");if(read==0)break;hash.AppendData(buffer,0,(int)read);}var snapshot=new FileSnapshot(Identity(info),bytes,((long)info.WriteHigh<<32)|info.WriteLow,Convert.ToHexString(hash.GetHashAndReset()));if(keepForDelete){held=handle;handed=true;}return snapshot;}finally{if(!handed)handle.Dispose();}}
        public static void DeleteHeld(SafeFileHandle handle){var value=new Disposition{DeleteFile=1};if(!SetFileInformationByHandle(handle,4,ref value,4))throw new IOException("Guarded file deletion failed; Win32 "+Marshal.GetLastWin32Error());}
    }
}
