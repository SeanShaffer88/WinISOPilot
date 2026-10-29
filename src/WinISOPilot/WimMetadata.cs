using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Xml;
using System.Xml.Linq;

namespace ImageGuide;
public sealed record WimObservation(string Operation,string ReturnValue,int? Win32Error);
public static class WimMetadata
{
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern IntPtr LoadLibraryEx(string path,IntPtr reserved,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Ansi,ExactSpelling=true,SetLastError=true)]static extern IntPtr GetProcAddress(IntPtr module,string name);
    [DllImport("kernel32.dll",SetLastError=true)][return:MarshalAs(UnmanagedType.Bool)]static extern bool FreeLibrary(IntPtr module);
    [DllImport("kernel32.dll",SetLastError=true)]static extern IntPtr LocalFree(IntPtr memory);
    [UnmanagedFunctionPointer(CallingConvention.Winapi,CharSet=CharSet.Unicode,SetLastError=true)]delegate IntPtr CreateFile(string path,uint access,uint disposition,uint flags,uint compression,out uint creationResult);
    [UnmanagedFunctionPointer(CallingConvention.Winapi,SetLastError=true)]delegate uint ImageCount(IntPtr handle);
    [UnmanagedFunctionPointer(CallingConvention.Winapi,SetLastError=true)][return:MarshalAs(UnmanagedType.Bool)]delegate bool ImageInformation(IntPtr handle,out IntPtr data,out uint bytes);
    [UnmanagedFunctionPointer(CallingConvention.Winapi,SetLastError=true)][return:MarshalAs(UnmanagedType.Bool)]delegate bool CloseHandle(IntPtr handle);
    public static List<Edition> Read(string payload,string folder,List<WimObservation> observations)
    {var raw=ReadRaw(payload,folder,observations);return Parse(raw.Xml,raw.Count);}
    public static (string Xml,int Count) ReadRaw(string payload,string folder,List<WimObservation> observations)
    {
        string dll=Path.Combine(Environment.SystemDirectory,"wimgapi.dll");Engine.CheckPath(dll);
        IntPtr library=LoadLibraryEx(dll,IntPtr.Zero,0x800);
        int? error=library==IntPtr.Zero?Marshal.GetLastWin32Error():null;
        observations.Add(new("LoadLibraryEx(System32 wimgapi.dll, SEARCH_SYSTEM32)",library==IntPtr.Zero?"NULL":"module",error));
        if(library==IntPtr.Zero)throw new Win32Exception(error!.Value,"Trusted inbox WIMGAPI could not load.");
        IntPtr handle=IntPtr.Zero;IntPtr data=IntPtr.Zero;CloseHandle? close=null;
        try {
            T Bind<T>(string name) where T:Delegate {var address=GetProcAddress(library,name);if(address==IntPtr.Zero)throw new Win32Exception(Marshal.GetLastWin32Error(),"Missing WIMGAPI query export: "+name);return Marshal.GetDelegateForFunctionPointer<T>(address);}
            var create=Bind<CreateFile>("WIMCreateFile");var count=Bind<ImageCount>("WIMGetImageCount");var information=Bind<ImageInformation>("WIMGetImageInformation");close=Bind<CloseHandle>("WIMCloseHandle");
            handle=create(payload,0,3,0,0,out uint creationResult);
            error=handle==IntPtr.Zero?Marshal.GetLastWin32Error():null;observations.Add(new("WIMCreateFile(access0, OPEN_EXISTING3, flags0, compression0)",handle==IntPtr.Zero?"NULL":"handle; creationResult="+creationResult,error));
            if(handle==IntPtr.Zero)throw new Win32Exception(error!.Value,"WIM container query-only open failed.");
            uint imageCount=count(handle);error=imageCount==0?Marshal.GetLastWin32Error():null;observations.Add(new("WIMGetImageCount",imageCount.ToString(),error));
            if(imageCount==0||imageCount>1024)throw new InvalidDataException("WIM image count unavailable or outside bound.");
            bool ok=information(handle,out data,out uint bytes);error=ok?null:Marshal.GetLastWin32Error();observations.Add(new("WIMGetImageInformation(container handle)",ok+"; bytes="+bytes,error));
            if(!ok)throw new Win32Exception(error!.Value,"WIM root metadata query failed.");
            if(data==IntPtr.Zero||bytes<2||bytes>8*1024*1024||bytes%2!=0)throw new InvalidDataException("WIM XML buffer has invalid size.");
            var raw=new byte[bytes];Marshal.Copy(data,raw,0,raw.Length);using(var rawFile=new FileStream(Path.Combine(folder,"wim-container.xml"),FileMode.CreateNew,FileAccess.Write,FileShare.None))rawFile.Write(raw);
            string xml=System.Text.Encoding.Unicode.GetString(raw).TrimEnd('\0').TrimStart('\uFEFF');
            return (xml,checked((int)imageCount));
        } finally {
            bool cleanupFailed=false;
            if(data!=IntPtr.Zero){IntPtr remaining=LocalFree(data);observations.Add(new("LocalFree(XML buffer)",remaining==IntPtr.Zero?"NULL (freed)":"non-NULL (free failed)",remaining==IntPtr.Zero?null:Marshal.GetLastWin32Error()));cleanupFailed|=remaining!=IntPtr.Zero;}
            bool closeConfirmed=handle==IntPtr.Zero;
            if(handle!=IntPtr.Zero&&close!=null){bool ok=close(handle);observations.Add(new("WIMCloseHandle",ok.ToString(),ok?null:Marshal.GetLastWin32Error()));cleanupFailed|=!ok;closeConfirmed=ok;}
            if(closeConfirmed){bool unloaded=FreeLibrary(library);observations.Add(new("FreeLibrary",unloaded.ToString(),unloaded?null:Marshal.GetLastWin32Error()));cleanupFailed|=!unloaded;}
            else {cleanupFailed=true;observations.Add(new("ModuleRetained","WIM handle closure unconfirmed; module loader reference retained until process exit. Cleanup required; no release claim.",null));}
            if(cleanupFailed)throw new IOException("WIMGAPI cleanup returned failure; see native WIM observations. Inspection rejected.");
        }
    }
    public static List<Edition> Parse(string xml,int expectedCount)
    {
        var settings=new XmlReaderSettings{DtdProcessing=DtdProcessing.Prohibit,XmlResolver=null,MaxCharactersInDocument=4*1024*1024};
        using var input=new StringReader(xml);using var reader=XmlReader.Create(input,settings);var document=XDocument.Load(reader);
        if(document.Root?.Name!=XName.Get("WIM"))throw new InvalidDataException("Unexpected WIM metadata root.");
        string One(XElement element,string name){var nodes=element.Elements(name).ToArray();if(nodes.Length!=1||string.IsNullOrWhiteSpace(nodes[0].Value))throw new InvalidDataException("Missing or duplicate WIM metadata: "+name);return nodes[0].Value.Trim();}
        XElement Element(XElement element,string name){var nodes=element.Elements(name).ToArray();if(nodes.Length!=1)throw new InvalidDataException("Missing or duplicate WIM element: "+name);return nodes[0];}
        int Number(XElement element,string name){if(!int.TryParse(One(element,name),out int n)||n<0)throw new InvalidDataException("Invalid numeric WIM field: "+name);return n;}
        var images=document.Root.Elements("IMAGE").ToArray();
        if(expectedCount<1||images.Length!=expectedCount)throw new InvalidDataException("WIM image count/XML mismatch.");
        var result=new List<Edition>();
        foreach(var image in images){
            var attribute=image.Attribute("INDEX");if(attribute==null||!int.TryParse(attribute.Value,out int index)||index<1)throw new InvalidDataException("Invalid WIM image index.");
            var windows=Element(image,"WINDOWS");var version=Element(windows,"VERSION");
            int major=Number(version,"MAJOR"),minor=Number(version,"MINOR"),build=Number(version,"BUILD"),revision=Number(version,"SPBUILD");
            if(major!=10||minor!=0||build<22000)throw new InvalidDataException("Unrecognized Windows client version in WIM metadata.");
            string installation=One(windows,"INSTALLATIONTYPE");if(installation!="Client")throw new InvalidDataException("Server/unknown installation type rejected.");
            int arch=Number(windows,"ARCH");string architecture=arch switch{9=>"x64",12=>"arm64",_=>throw new InvalidDataException("Unsupported Windows 11 architecture code: "+arch)};
            string editionId=One(windows,"EDITIONID");
            var languageRoot=Element(windows,"LANGUAGES");var languages=languageRoot.Elements("LANGUAGE").Select(e=>e.Value.Trim()).ToList();
            if(languages.Count==0||languages.Any(string.IsNullOrWhiteSpace)||languages.Distinct(StringComparer.OrdinalIgnoreCase).Count()!=languages.Count)throw new InvalidDataException("Missing or duplicate WIM language metadata.");
            result.Add(new(index,One(image,"NAME"),architecture,$"{major}.{minor}.{build}.{revision}",build,installation,editionId,languages));
        }
        if(!result.Select(e=>e.Index).Order().SequenceEqual(Enumerable.Range(1,expectedCount))||result.Select(e=>e.Build).Distinct().Count()!=1||result.Select(e=>e.Architecture).Distinct().Count()!=1)throw new InvalidDataException("Noncontiguous, duplicate or inconsistent WIM image metadata.");
        return result;
    }
}
