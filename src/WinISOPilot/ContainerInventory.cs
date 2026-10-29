using System.Security.Principal;
using System.Text.Json;
using System.Xml;
using System.Xml.Linq;
namespace ImageGuide;

public sealed record ContainerImage(int Index,string Name,string? Architecture,string? InstallationType,string? Version,string? EditionId);
public sealed record ContainerObservation(string Scope,List<ContainerImage> Images);
public sealed record ContainerReport(string Source,string Sha256,List<ContainerObservation> Containers,string Integrity,string Mounting,string WinReCoverage);
public static class ContainerInventory
{
    public static bool IsElevated {get{using var identity=WindowsIdentity.GetCurrent();return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);}}
    public static string Validate(string path)
    {
        if(!Path.IsPathFullyQualified(path)||path.StartsWith(@"\\",StringComparison.Ordinal)||path.Contains("://",StringComparison.Ordinal))throw new InvalidDataException("Choose an absolute local image file.");path=Path.GetFullPath(path);if(path[2..].Contains(':')||!File.Exists(path))throw new InvalidDataException("Image file unavailable or unsupported path.");Engine.CheckPath(path);
        if(!new[]{".iso",".wim",".esd",".swm"}.Contains(Path.GetExtension(path),StringComparer.OrdinalIgnoreCase))throw new InvalidDataException("Choose ISO, WIM or ESD media.");
        if(path.EndsWith(".swm",StringComparison.OrdinalIgnoreCase))throw new InvalidDataException("Split SWM recognized. Set inventory and servicing are not implemented; no part was opened or changed.");return path;
    }
    public static List<ContainerImage> Parse(string xml,int expected)
    {
        if(expected<1||expected>1024)throw new InvalidDataException("Image count outside bound.");using var input=new StringReader(xml);using var reader=XmlReader.Create(input,new XmlReaderSettings{DtdProcessing=DtdProcessing.Prohibit,XmlResolver=null,MaxCharactersInDocument=4*1024*1024});var doc=XDocument.Load(reader);if(doc.Root?.Name!=XName.Get("WIM"))throw new InvalidDataException("Unexpected metadata root.");var images=doc.Root.Elements("IMAGE").ToArray();if(images.Length!=expected)throw new InvalidDataException("Container count mismatch.");
        string? Field(XElement? node,string name){if(node==null)return null;var found=node.Elements(name).ToArray();if(found.Length>1)throw new InvalidDataException("Duplicate metadata field: "+name);return found.Length==0?null:found[0].Value.Trim();}
        XElement? Node(XElement node,string name){var found=node.Elements(name).ToArray();if(found.Length>1)throw new InvalidDataException("Duplicate metadata node: "+name);return found.SingleOrDefault();}
        var result=new List<ContainerImage>();foreach(var image in images){if(!int.TryParse(image.Attribute("INDEX")?.Value,out int index)||index<1)throw new InvalidDataException("Invalid index.");var windows=Node(image,"WINDOWS");var version=windows==null?null:Node(windows,"VERSION");string? part(string key){string? text=Field(version,key);if(text!=null&&(!int.TryParse(text,out int number)||number<0))throw new InvalidDataException("Invalid version number.");return text;}var numbers=new[]{part("MAJOR"),part("MINOR"),part("BUILD"),part("SPBUILD")};string? arch=Field(windows,"ARCH");result.Add(new(index,Field(image,"NAME")??"Unnamed image",arch switch{"9"=>"x64","12"=>"arm64","0"=>"x86",null=>null,_=>"Unknown code "+arch},Field(windows,"INSTALLATIONTYPE"),numbers.All(x=>x!=null)?string.Join('.',numbers):null,Field(windows,"EDITIONID")));}
        if(!result.Select(x=>x.Index).Order().SequenceEqual(Enumerable.Range(1,expected)))throw new InvalidDataException("Noncontiguous or duplicate image indexes.");return result;
    }
    static void WriteNew(string path,string text){using var stream=new FileStream(path,FileMode.CreateNew,FileAccess.Write,FileShare.None);using var writer=new StreamWriter(stream);writer.Write(text);}
    public static Task<ContainerReport> Inspect(string source,string output,JobControl? control=null,IProgress<JobProgress>? progress=null)=>Task.Run(()=>InspectCore(source,output,control,progress));
    static ContainerReport InspectCore(string source,string output,JobControl? control,IProgress<JobProgress>? progress)
    {
        if(Engine.RestartRequired)throw new IOException("Restart required before more native work.");control?.Checkpoint("Starting container inventory",progress);source=Validate(source);using var lease=new FileStream(source,FileMode.Open,FileAccess.Read,FileShare.Read);output=Engine.NewFolder(output);using var outputLease=Engine.InventoryDirectoryLease(output);string? before=null;NativeIso? iso=null;Exception? failure=null;var containers=new List<ContainerObservation>();
        try{before=Engine.HashControlled(source,control,progress,"Hashing container source");control?.Checkpoint("Reading container inventory",progress);
            if(source.EndsWith(".iso",StringComparison.OrdinalIgnoreCase)){iso=new NativeIso(source);iso.AttachOrReuse();iso.VerifyAssociation();string root=iso.DriveRoot??throw new IOException("ISO root unavailable.");var installs=new[]{"install.wim","install.esd"}.Select(x=>Path.Combine(root,"sources",x)).Where(File.Exists).ToArray();if(installs.Length!=1)throw new InvalidDataException("Exactly one installation WIM or ESD is required; split media is unsupported.");Read(installs[0],"install");Read(Path.Combine(root,"sources","boot.wim"),"boot");iso.VerifyAssociation();}
            else Read(source,"standalone");
        }catch(Exception ex){failure=ex;}
        finally{try{iso?.Dispose();if(iso!=null&&iso.Disposition!="HandleReleased")throw new IOException("ISO handle release unconfirmed.");}catch(Exception ex){Engine.LatchInventoryFailure("Container ISO cleanup uncertain: "+ex.Message);failure=ex;}try{if(before!=null&&Engine.HashControlled(source,null,progress,"Final source verification",false)!=before){Engine.LatchInventoryFailure("Container source identity changed.");failure=new IOException("Original image identity changed; keep reports.");}}catch(Exception ex){Engine.LatchInventoryFailure("Container integrity verification failed: "+ex.Message);failure=ex;}}
        if(failure!=null)System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(failure).Throw();control?.FinalCheckpoint("Final container inventory",progress);var report=new ContainerReport(source,before!,containers,"SHA-256 matched before/after; not authenticity or install proof","Not implemented; no Windows image mounted","WinRE inside installation images not searched. Standalone WinRE WIM may be queried explicitly.");WriteNew(Path.Combine(output,"container-inventory.json"),JsonSerializer.Serialize(report,Engine.Json));return report;
        void Read(string payload,string scope){control?.Checkpoint("Reading "+scope+" metadata",progress);Engine.CheckPath(payload);string folder=Engine.NewFolder(Path.Combine(output,scope));using var folderLease=Engine.InventoryDirectoryLease(folder);var observations=new List<WimObservation>();try{var raw=WimMetadata.ReadRaw(payload,folder,observations);containers.Add(new(scope,Parse(raw.Xml,raw.Count)));}finally{if(observations.Any(x=>x.Operation=="ModuleRetained"||((x.Operation=="WIMCloseHandle"||x.Operation=="FreeLibrary")&&x.ReturnValue=="False")||(x.Operation=="LocalFree(XML buffer)"&&x.ReturnValue.StartsWith("non-NULL",StringComparison.Ordinal))))Engine.LatchInventoryFailure("WIMGAPI cleanup uncertain during container inventory.");WriteNew(Path.Combine(folder,"native-observations.json"),JsonSerializer.Serialize(observations,Engine.Json));}}
    }
}
public static partial class Engine
{
    internal static Microsoft.Win32.SafeHandles.SafeFileHandle InventoryDirectoryLease(string path)=>CleanupNative.LeaseDirectory(path);
    internal static void LatchInventoryFailure(string reason)=>RequireRestart(reason);
}
