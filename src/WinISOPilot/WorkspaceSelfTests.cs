using System.Reflection;
namespace ImageGuide;
public static class WorkspaceSelfTests
{
    public static void Run()
    {
        int passed=0;void Check(string name,Action test){test();Console.WriteLine("PASS "+name);passed++;}void Reject(Action action){try{action();}catch(Exception ex)when(ex is IOException or InvalidDataException or System.Xml.XmlException){return;}throw new Exception("Expected rejection");}
        string pe="<WIM><IMAGE INDEX=\"1\"><NAME>Windows PE</NAME><WINDOWS><ARCH>9</ARCH><INSTALLATIONTYPE>WindowsPE</INSTALLATIONTYPE><VERSION><MAJOR>10</MAJOR><MINOR>0</MINOR><BUILD>26100</BUILD><SPBUILD>9457</SPBUILD></VERSION></WINDOWS></IMAGE></WIM>";
        Check("generic inventory accepts WinPE without client edition fields",()=>{var item=ContainerInventory.Parse(pe,1).Single();if(item.InstallationType!="WindowsPE"||item.EditionId!=null||item.Version!="10.0.26100.9457")throw new Exception();});
        Check("strict client adapter still rejects WinPE",()=>Reject(()=>WimMetadata.Parse(pe,1)));
        Check("generic optional fields remain unknown",()=>{var item=ContainerInventory.Parse("<WIM><IMAGE INDEX=\"1\"><NAME>Recovery</NAME></IMAGE></WIM>",1).Single();if(item.Architecture!=null||item.Version!=null||item.InstallationType!=null)throw new Exception();});
        Check("generic duplicate fields rejected",()=>Reject(()=>ContainerInventory.Parse(pe.Replace("<ARCH>9</ARCH>","<ARCH>9</ARCH><ARCH>9</ARCH>"),1)));
        Check("generic duplicate index rejected",()=>Reject(()=>ContainerInventory.Parse(pe.Replace("</WIM>",pe[5..]),2)));
        Check("generic DTD rejected",()=>Reject(()=>ContainerInventory.Parse("<!DOCTYPE WIM [<!ENTITY x 'a'>]>"+pe,1)));
        Check("generic malformed version rejected",()=>Reject(()=>ContainerInventory.Parse(pe.Replace("<BUILD>26100</BUILD>","<BUILD>bad</BUILD>"),1)));
        Check("recipe never grants execution or predicts savings",()=>{string result=RecipePreview.Create(false,null,new[]{"Apps"},true,"Keep source region",null);if(!result.Contains("SIMULATION ONLY")||!result.Contains("NOT IMPLEMENTED")||!result.Contains("blocked")||!result.Contains("unknown, not simulated"))throw new Exception();});
        var flags=BindingFlags.Instance|BindingFlags.NonPublic;
        GuideForm Form(){var previous=SynchronizationContext.Current;try{return new GuideForm{AutoScaleMode=AutoScaleMode.None,ClientSize=ShellGeometry.CanonicalClient};}finally{SynchronizationContext.SetSynchronizationContext(previous);}}
        void Layout(Control parent){parent.PerformLayout();foreach(Control child in parent.Controls)Layout(child);}
        Check("ten distinct workspace pages and sidebar bounds",()=>{using var form=Form();_ = form.Handle;for(int i=0;i<3;i++)Layout(form);var buttons=(List<Button>)typeof(GuideForm).GetField("steps",flags)!.GetValue(form)!;var pages=(List<TableLayoutPanel>)typeof(GuideForm).GetField("pages",flags)!.GetValue(form)!;if(buttons.Count!=10||pages.Count!=10||buttons.Select(x=>x.Text).Distinct().Count()!=10)throw new Exception();foreach(var button in buttons)if(button.Bounds.Right>button.Parent!.ClientSize.Width||button.Height<form.Font.Height+6)throw new Exception("Sidebar clipping");});
        Check("workspace simulation immediately follows undo",()=>{using var form=Form();var goals=(Dictionary<string,CheckBox>)typeof(GuideForm).GetField("goals",flags)!.GetValue(form)!;goals["Apps"].Checked=true;var preview=(TextBox)typeof(GuideForm).GetField("simulation",flags)!.GetValue(form)!;if(!preview.Text.Contains("Apps: NOT IMPLEMENTED"))throw new Exception("Choice not shared");typeof(GuideForm).GetMethod("Undo",flags)!.Invoke(form,null);if(preview.Text.Contains("Apps: NOT IMPLEMENTED"))throw new Exception("Undo preview stale");});
        Check("read-only navigation stays available during active work",()=>{using var form=Form();typeof(GuideForm).GetField("busy",flags)!.SetValue(form,true);typeof(GuideForm).GetMethod("Go",flags)!.Invoke(form,new object[]{0});var buttons=(List<Button>)typeof(GuideForm).GetField("steps",flags)!.GetValue(form)!;if(buttons.Any(x=>!x.Enabled)||((Button)typeof(GuideForm).GetField("build",flags)!.GetValue(form)!).Enabled||((ComboBox)typeof(GuideForm).GetField("previewRegion",flags)!.GetValue(form)!).Enabled)throw new Exception("Job navigation or mutation guards mismatch");});
        Check("telemetry starts unknown and reports real byte observations",()=>{using var form=Form();var box=(TextBox)typeof(GuideForm).GetField("telemetry",flags)!.GetValue(form)!;if(!box.Text.Contains("No measured"))throw new Exception();typeof(GuideForm).GetMethod("ObserveTelemetry",flags)!.Invoke(form,new object[]{new JobProgress("Copying fixture","Measured",100,200)});if(!box.Text.Contains("100 bytes observed / 200"))throw new Exception();});
        Check("mount readiness never authorizes unimplemented worker",()=>{if(MountReadiness.Evaluate(true,true,true,true).Executable||MountReadiness.Evaluate(false,false,false,false).Reasons.Length<4)throw new Exception();});
        Check("scratch requirements reject unknown expanded servicing size",()=>{if(ScratchPreflight.RequiredBytes(1000,null)!=null||ScratchPreflight.RequiredBytes(1000,2000)!=3*1000+2000+1024L*1024*1024)throw new Exception();});
        string temp=Path.Combine(Path.GetTempPath(),"WinISOPilot-workspace-tests-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(temp);
        try{
            string swm=Path.Combine(temp,"install.swm");File.WriteAllText(swm,"unopened fixture");Check("split SWM recognized and blocked before querying",()=>Reject(()=>ContainerInventory.Validate(swm)));
            Check("scratch existing destination rejected",()=>Reject(()=>ScratchPreflight.Inspect(temp,1000,null)));
            Check("scratch unknown expansion never creates destination",()=>{string destination=Path.Combine(temp,"new-scratch");var assessment=ScratchPreflight.Inspect(destination,1000,null);if(assessment.Approved||assessment.RequiredBytes!=null||Directory.Exists(destination))throw new Exception();});
            Check("canceled inventory starts no output or native query",()=>{using var control=new JobControl();control.RequestCancel();string output=Path.Combine(temp,"canceled");try{ContainerInventory.Inspect(swm,output,control).GetAwaiter().GetResult();throw new Exception("Cancel not honored");}catch(OperationCanceledException){}if(Directory.Exists(output))throw new Exception();});
        }finally{Directory.Delete(temp,true);}
        Console.WriteLine($"{passed} workspace and generic inventory tests passed.");
    }
}
