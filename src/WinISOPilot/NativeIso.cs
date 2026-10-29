using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

namespace ImageGuide;
public sealed record NativeStatus(string Operation,uint? Code,string Detail);
public sealed class NativeIso:IDisposable
{
    [StructLayout(LayoutKind.Sequential)] struct StorageType {public uint DeviceId;public Guid VendorId;}
    [StructLayout(LayoutKind.Explicit,Size=44)] struct OpenParameters
    {
        [FieldOffset(0)] public uint Version;
        [FieldOffset(4)] public uint RWDepth;
    }
    [DllImport("virtdisk.dll",CharSet=CharSet.Unicode)]
    static extern uint OpenVirtualDisk(ref StorageType type,string path,uint access,uint flags,ref OpenParameters parameters,out SafeFileHandle handle);
    [DllImport("virtdisk.dll")]
    static extern uint AttachVirtualDisk(SafeFileHandle handle,IntPtr descriptor,uint flags,uint providerFlags,IntPtr parameters,IntPtr overlapped);
    [DllImport("virtdisk.dll",CharSet=CharSet.Unicode)]
    static extern uint GetVirtualDiskPhysicalPath(SafeFileHandle handle,ref uint size,StringBuilder path);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern uint QueryDosDevice(string name,[Out] char[] target,uint maxChars);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern uint GetDriveType(string root);
    SafeFileHandle? handle;
    readonly string source;
    string? physical;
    string? ntDevice;
    string? driveRoot;
    public List<NativeStatus> Statuses {get;}=new();
    public string Ownership {get;private set;}="NotOpened";
    public string Disposition {get;private set;}="NotOpened";
    public string? PostReleasePhysicalPath {get;private set;}
    public string? DriveRoot=>driveRoot;
    public static int OpenParameterSize=>Marshal.SizeOf<OpenParameters>();
    public static int VersionOffset=>(int)Marshal.OffsetOf<OpenParameters>(nameof(OpenParameters.Version));
    public static int RWDepthOffset=>(int)Marshal.OffsetOf<OpenParameters>(nameof(OpenParameters.RWDepth));
    public NativeIso(string path){source=path;}
    SafeFileHandle Open()
    {
        var type=new StorageType{DeviceId=1,VendorId=new Guid("EC984AEC-A0F9-47e9-901F-71415A66345B")};
        var parameters=new OpenParameters{Version=1,RWDepth=0};
        uint code=OpenVirtualDisk(ref type,source,0xD0000,0,ref parameters,out var opened);
        Statuses.Add(new("OpenVirtualDisk",code,"ISO; access0xD0000; flags0; OPEN version1/RWDepth0/size44"));
        if(code!=0){opened?.Dispose();throw new Win32Exception((int)code,"OpenVirtualDisk: "+new Win32Exception((int)code).Message);}
        return opened;
    }
    (uint Code,string? Path) Physical(SafeFileHandle opened,string operation)
    {
        uint bytes=1024;var buffer=new StringBuilder(512);
        uint code=GetVirtualDiskPhysicalPath(opened,ref bytes,buffer);
        if(code==122&&bytes>1024&&bytes<=65536){buffer=new StringBuilder(checked((int)(bytes/2)));code=GetVirtualDiskPhysicalPath(opened,ref bytes,buffer);}
        string? path=code==0?buffer.ToString():null;
        Statuses.Add(new(operation,code,path??new Win32Exception((int)code).Message));
        return(code,path);
    }
    string Mapping(string name)
    {
        var buffer=new char[32768];uint count=QueryDosDevice(name,buffer,(uint)buffer.Length);
        if(count==0){int error=Marshal.GetLastWin32Error();Statuses.Add(new("QueryDosDevice("+name+")",(uint)error,new Win32Exception(error).Message));throw new Win32Exception(error);}
        string current=new string(buffer,0,(int)count).Split('\0')[0];
        Statuses.Add(new("QueryDosDevice("+name+")",0,current));return current;
    }
    public void AttachOrReuse()
    {
        handle=Open();Disposition="HandleOpen";
        var existing=Physical(handle,"GetPhysicalPath(beforeAttach)");
        if(existing.Code==0){physical=existing.Path;Ownership="PreExisting";}
        else {
            // ERROR_DEV_NOT_EXIST is the expected unattached-state response.
            if(existing.Code!=55)throw new Win32Exception((int)existing.Code,"Cannot establish unattached ISO state; refusing attach.");
            uint code=AttachVirtualDisk(handle,IntPtr.Zero,1,0,IntPtr.Zero,IntPtr.Zero);
            Statuses.Add(new("AttachVirtualDisk",code,"READ_ONLY only; no permanent lifetime; null optional parameters"));
            var after=Physical(handle,"GetPhysicalPath(afterAttach)");
            if(code==0){Ownership="HandleScopedAttach";if(after.Code!=0)throw new Win32Exception((int)after.Code,"Attached but physical association unavailable.");physical=after.Path;}
            else if(after.Code==0){Ownership="ObservedAfterAttachFailure";physical=after.Path;}
            else throw new Win32Exception((int)code,"Read-only ISO attach failed: "+new Win32Exception((int)code).Message);
        }
        if(string.IsNullOrEmpty(physical)||!physical.StartsWith(@"\\.\",StringComparison.Ordinal))throw new InvalidDataException("Unexpected virtual disk physical path.");
        string name=physical[4..];
        if(!name.StartsWith("CDROM",StringComparison.OrdinalIgnoreCase)||name.Length==5||name[5..].Any(c=>!char.IsAsciiDigit(c)))throw new InvalidDataException("Expected optical CDROM physical device.");
        ntDevice=Mapping(name);
        // Bound drive-letter discovery; never assign a letter or choose an arbitrary drive.
        for(int attempt=0;attempt<20;attempt++){
            var matches=new List<string>();
            for(char letter='A';letter<='Z';letter++){
                string root=letter+@":\";if(GetDriveType(root)!=5)continue;
                string mapping;try{mapping=Mapping(letter+":");}catch(Win32Exception){continue;}
                if(mapping.Equals(ntDevice,StringComparison.OrdinalIgnoreCase))matches.Add(root);
            }
            if(matches.Count==1){driveRoot=matches[0];VerifyAssociation();return;}
            if(matches.Count>1)throw new InvalidDataException("Ambiguous optical drive mapping.");
            Thread.Sleep(250);
        }
        throw new IOException("No unique optical drive letter appeared within five seconds.");
    }
    public void VerifyAssociation()
    {
        if(handle==null||handle.IsClosed||physical==null||ntDevice==null||driveRoot==null)throw new InvalidOperationException("ISO association unavailable.");
        var current=Physical(handle,"GetPhysicalPath(preQuery)");
        if(current.Code!=0||!string.Equals(current.Path,physical,StringComparison.OrdinalIgnoreCase)||!Mapping(physical[4..]).Equals(ntDevice,StringComparison.OrdinalIgnoreCase)||GetDriveType(driveRoot)!=5||!Mapping(driveRoot[..2]).Equals(ntDevice,StringComparison.OrdinalIgnoreCase))throw new IOException("ISO/optical drive association changed; query rejected.");
    }
    public void Dispose()
    {
        if(handle==null)return;
        handle.Dispose();handle=null;Disposition="HandleReleased";
        Statuses.Add(new("SafeHandle.Dispose",null,"Application handle released; no explicit detach operation was called. Dispose provides no Win32 status code."));
        // A separate information handle observes disposition; it never attaches or detaches.
        try{using var probe=Open();var observed=Physical(probe,"GetPhysicalPath(afterRelease)");PostReleasePhysicalPath=observed.Path;Statuses.Add(new("PostReleaseObservation",observed.Code,observed.Code==0?"Device still observed; another/permanent lifetime may retain it":"No device observed by this query; not a global mount-state guarantee"));}
        catch(Exception ex){Statuses.Add(new("PostReleaseObservation",null,ex.Message));}
    }
}
