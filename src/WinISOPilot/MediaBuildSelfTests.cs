namespace ImageGuide;
public static partial class Engine
{
    internal static void BuildSelfTests()
    {
        int count=0;void Test(string name,Action action){action();Console.WriteLine("PASS "+name);count++;}
        void Reject(Action action){try{action();}catch(Exception ex) when(ex is InvalidDataException or IOException){return;}throw new Exception("Expected rejection.");}
        var home=new Edition(1,"Home","x64","10.0.26300.9457",26300,"Client","Core");
        Test("build exact Retail edition configuration",()=>{if(EditionConfiguration(home)!="[EditionID]\r\nCore\r\n[Channel]\r\nRetail\r\n[VL]\r\n0\r\n")throw new Exception();});
        Test("build unknown edition cannot inject configuration",()=>Reject(()=>EditionConfiguration(home with{EditionId="Core\r\n[Channel]"})));
        Test("build server preference rejected",()=>Reject(()=>EditionConfiguration(home with{Installation="Server"})));
        string parent=Path.Combine(Path.GetTempPath(),"WinISOPilot-build-tests-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(parent);
        Test("build relative containment rejects escape",()=>Reject(()=>BuildRelative(parent,Path.Combine(parent,"..","foreign.txt"))));
        var conflict=Path.Combine(parent,"conflicts");Directory.CreateDirectory(Path.Combine(conflict,"sources"));File.WriteAllText(Path.Combine(conflict,"sources","pid.txt"),"test marker, not a key");
        Test("build existing product-id config blocks edition preference",()=>Reject(()=>CheckSetupConflicts(conflict)));
        File.Delete(Path.Combine(conflict,"sources","pid.txt"));File.WriteAllText(Path.Combine(conflict,"autounattend.xml"),"fixture");
        Test("build answer file blocks edition preference",()=>Reject(()=>CheckSetupConflicts(conflict)));
        Test("build rejects missing boot layout",()=>Reject(()=>ValidateMediaLayout(conflict)));
        BuildRun Fixture(string name){string root=NewFolder(Path.Combine(parent,name));var id=Guid.NewGuid();var run=new BuildRun(root,Path.Combine(parent,"original.iso"),id);run.Directories.Add(root,CleanupNative.DirectoryIdentity(root));buildRuns[id]=run;BuildWrite(run,"generated.txt","owned file");run.Terminal="Canceled";run.Safe=true;foreach(string path in run.Created.Keys){var snap=CleanupNative.Snapshot(path,false,out _,true);if(snap.Identity==run.Created[path])run.Frozen[path]=snap;}return run;}
        Test("build cleanup deletes only unchanged owned file",()=>{var r=Fixture("unchanged");var preview=PreviewBuildCleanup(r.Root,r.Id);if(!preview.CanDelete||preview.Files.Count!=1)throw new Exception();var cleaned=CleanupBuild(r.Root,r.Id);if(cleaned.Removed.Count!=1||File.Exists(Path.Combine(r.Root,"generated.txt"))||!Directory.Exists(r.Root))throw new Exception();});
        Test("build cleanup wrong run rejected",()=>{var r=Fixture("wrong");if(PreviewBuildCleanup(r.Root,Guid.NewGuid()).CanDelete)throw new Exception();});
        Test("build completed output never cleanup eligible",()=>{var r=Fixture("complete");r.Terminal="Complete";if(PreviewBuildCleanup(r.Root,r.Id).CanDelete)throw new Exception();});
        Test("build changed file before preview retained",()=>{var r=Fixture("changed");File.WriteAllText(Path.Combine(r.Root,"generated.txt"),"user replacement");if(PreviewBuildCleanup(r.Root,r.Id).CanDelete)throw new Exception();});
        Test("build changed file after preview retained",()=>{var r=Fixture("changed-after");_ = PreviewBuildCleanup(r.Root,r.Id);File.WriteAllText(Path.Combine(r.Root,"generated.txt"),"user replacement");if(CleanupBuild(r.Root,r.Id).Removed.Count!=0)throw new Exception();});
        Test("build replacement identity before freeze retained",()=>{var r=Fixture("replaced");string file=Path.Combine(r.Root,"generated.txt");File.Move(file,file+".original");File.WriteAllText(file,"owned file");var now=CleanupNative.Snapshot(file,false,out _,true);r.Frozen.Clear();if(now.Identity==r.Created[file])throw new Exception("Fixture identity not replaced");if(PreviewBuildCleanup(r.Root,r.Id).CanDelete)throw new Exception();});
        Test("build same-identity content change before freeze retained",()=>{var r=Fixture("changed-before-freeze");string path=Path.Combine(r.Root,"generated.txt");File.WriteAllText(path,"foreign bytes");var now=CleanupNative.Snapshot(path,false,out _,true);r.Frozen.Clear();if(now.Identity==r.Created[path]&&now.Sha256==r.ExpectedHash[path])r.Frozen[path]=now;if(PreviewBuildCleanup(r.Root,r.Id).CanDelete)throw new Exception();});
        Test("build unknown entries and directories retained",()=>{var r=Fixture("foreign");string unrelated=Path.Combine(r.Root,"keep.txt");File.WriteAllText(unrelated,"user");_ = PreviewBuildCleanup(r.Root,r.Id);_ = CleanupBuild(r.Root,r.Id);if(!File.Exists(unrelated)||!Directory.Exists(r.Root))throw new Exception();});
        Test("build parent directory replacement blocks cleanup",()=>{var r=Fixture("dir-replaced");_ = PreviewBuildCleanup(r.Root,r.Id);Directory.Move(r.Root,r.Root+"-original");Directory.CreateDirectory(r.Root);File.WriteAllText(Path.Combine(r.Root,"generated.txt"),"foreign");if(CleanupBuild(r.Root,r.Id).Removed.Count!=0)throw new Exception();});
        Test("build large snapshot opt-in preserves inspection cap",()=>{string file=Path.Combine(parent,"large.bin");using(var stream=new FileStream(file,FileMode.CreateNew)){stream.SetLength(33*1024*1024);}Reject(()=>CleanupNative.Snapshot(file,false,out _));if(CleanupNative.Snapshot(file,false,out _,true).Bytes!=33*1024*1024)throw new Exception();});
        // Delete only exact fixture paths under this freshly-created test root, no recursion.
        foreach(string file in Directory.EnumerateFiles(parent,"*",SearchOption.AllDirectories))File.Delete(file);
        foreach(string dir in Directory.EnumerateDirectories(parent,"*",SearchOption.AllDirectories).OrderByDescending(x=>x.Length))Directory.Delete(dir,false);Directory.Delete(parent,false);
        Console.WriteLine(count+" media build safety tests passed. No source ISO or Windows image was modified.");
    }
}
