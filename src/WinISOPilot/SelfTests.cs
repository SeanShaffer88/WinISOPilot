using System.Text.Json;
namespace ImageGuide;
public static class SelfTests
{
    public static int Run()
    {
        int passed=0;
        string fixture="Index : 2\r\nName : Windows 11 Pro\r\nArchitecture : x64\r\nVersion : 10.0.26200\r\nInstallation : Client\r\n";
        void Check(string name,Action test){test();Console.WriteLine("PASS "+name);passed++;}
        void Reject(Action test){try{test();}catch(Exception ex) when(ex is InvalidDataException or IOException or System.Xml.XmlException){return;}throw new Exception("Expected rejection");}
        Check("English metadata",()=>{var e=Engine.ParseEdition(fixture,2);if(e.Build!=26200||e.Index!=2)throw new Exception();});
        Check("native OPEN ABI",()=>{if(NativeIso.OpenParameterSize!=44||NativeIso.VersionOffset!=0||NativeIso.RWDepthOffset!=4)throw new Exception("Native OPEN layout mismatch");});
        string xml="<WIM><IMAGE INDEX=\"1\"><NAME>Windows 11 Pro</NAME><WINDOWS><ARCH>9</ARCH><EDITIONID>Professional</EDITIONID><INSTALLATIONTYPE>Client</INSTALLATIONTYPE><LANGUAGES><LANGUAGE>en-US</LANGUAGE><DEFAULT>en-US</DEFAULT></LANGUAGES><VERSION><MAJOR>10</MAJOR><MINOR>0</MINOR><BUILD>26300</BUILD><SPBUILD>9457</SPBUILD></VERSION></WINDOWS></IMAGE></WIM>";
        Check("WIMGAPI XML actual fields",()=>{var e=WimMetadata.Parse(xml,1).Single();if(e.Version!="10.0.26300.9457"||e.Architecture!="x64"||e.EditionId!="Professional"||e.Languages?.Single()!="en-US")throw new Exception();});
        Check("WIMGAPI ARM64 XML",()=>{if(WimMetadata.Parse(xml.Replace("<ARCH>9</ARCH>","<ARCH>12</ARCH>"),1).Single().Architecture!="arm64")throw new Exception();});
        Check("WIMGAPI DTD rejected",()=>Reject(()=>WimMetadata.Parse("<!DOCTYPE WIM [<!ENTITY x 'value'>]>"+xml,1)));
        Check("WIMGAPI malformed XML rejected",()=>Reject(()=>WimMetadata.Parse(xml.Replace("</WINDOWS>","</WRONG>"),1)));
        Check("WIMGAPI oversized XML rejected",()=>Reject(()=>WimMetadata.Parse("<WIM>"+new string(' ',4*1024*1024)+"</WIM>",1)));
        Check("catalog debloat alias",()=>{if(!CapabilityCatalog.Search("debloat").Any(x=>x.Name=="Deep component stripping"))throw new Exception();});
        Check("catalog apps plural alias",()=>{if(!CapabilityCatalog.Search("apps").Any(x=>x.Group=="Apps"))throw new Exception();});
        Check("catalog Wi-Fi alias",()=>{if(!CapabilityCatalog.Search("Wi-Fi").Any(x=>x.Group=="Drivers"))throw new Exception();});
        Check("catalog unattended alias",()=>{if(!CapabilityCatalog.Search("unattended").Any(x=>x.Name=="Answer-file planning"))throw new Exception();});
        Check("catalog mixed case alias",()=>{if(CapabilityCatalog.Search("DeBlOaT").Count!=CapabilityCatalog.Search("debloat").Count)throw new Exception();});
        Check("catalog no results",()=>{if(CapabilityCatalog.Search("not-a-reference-family-XYZ").Count!=0)throw new Exception();});
        Check("catalog combined filters",()=>{var rows=CapabilityCatalog.Search("debloat","Features",CapabilityCatalog.Unsupported);if(rows.Count!=1||rows[0].Name!="Deep component stripping")throw new Exception();});
        Check("shell canonical fixed window",()=>{if(ShellGeometry.CanonicalClient!=new Size(920,660)||ShellGeometry.CanResize||ShellGeometry.CanMaximize)throw new Exception();});
        Check("shell canonical 96 DPI fit",()=>{if(ShellGeometry.Fit(ShellGeometry.CanonicalClient,96,new Size(1920,1080),new Size(16,39))!=new Size(920,660))throw new Exception();});
        foreach(int dpi in new[]{96,144,192})Check("shell working-area bounds at "+dpi+" DPI",()=>{var working=new Size(1920,1080);var chrome=new Size((int)Math.Round(16*dpi/96d),(int)Math.Round(39*dpi/96d));var size=ShellGeometry.Fit(ShellGeometry.CanonicalClient,dpi,working,chrome);int margin=(int)Math.Round(12*dpi/96d);if(size.Width<=0||size.Height<=0||size.Width+chrome.Width+2*margin>working.Width||size.Height+chrome.Height+2*margin>working.Height)throw new Exception();});
        Check("shell small working area clamps",()=>{var size=ShellGeometry.Fit(ShellGeometry.CanonicalClient,96,new Size(800,600),new Size(16,39));if(size.Width>760||size.Height>537)throw new Exception();});
        Check("shell long path preserved",()=>{string value=@"D:\"+new string('p',4000)+@"\Windows11.iso";if(ShellGeometry.FullPathText(value)!=value)throw new Exception();});
        foreach(bool dark in new[]{false,true})Check("shell "+(dark?"dark":"light")+" text contrast",()=>{var palette=ShellPalette.For(dark);if(ShellPalette.Contrast(palette.Text,palette.Field)<4.5||ShellPalette.Contrast(palette.Muted,palette.Surface)<4.5||ShellPalette.Contrast(palette.Disabled,palette.Button)<4.5||ShellPalette.Contrast(palette.OnAccent,palette.Accent)<4.5)throw new Exception("Palette contrast below 4.5:1");});
        Check("shell high-contrast colors respected",()=>{var palette=ShellPalette.For(true,true);if(palette.Field!=SystemColors.Window||palette.Text!=SystemColors.ControlText||palette.FieldText!=SystemColors.WindowText||palette.Accent!=SystemColors.Highlight)throw new Exception();});
        Check("shell selected and disabled button contrast",()=>{foreach(bool dark in new[]{false,true})foreach(bool selected in new[]{false,true})foreach(bool enabled in new[]{false,true}){var colors=ShellPalette.For(dark).ButtonColors(selected,enabled);if(ShellPalette.Contrast(colors.Foreground,colors.Background)<4.5)throw new Exception("Unreadable button state");}});
        void Layout(Control control){control.PerformLayout();foreach(Control child in control.Controls)Layout(child);}
        GuideForm HiddenShell(){var previous=SynchronizationContext.Current;GuideForm? form=null;try{form=new GuideForm{AutoScaleMode=AutoScaleMode.None,ClientSize=ShellGeometry.CanonicalClient};_ = form.Handle;for(int i=0;i<3;i++)Layout(form);return form;}catch{form?.Dispose();throw;}finally{SynchronizationContext.SetSynchronizationContext(previous);}}
        Check("shell measured WinForms header and stepper bounds",()=>{using var form=HiddenShell();var root=(TableLayoutPanel)form.Controls[0];var rows=root.GetRowHeights();if(rows[0]!=58||rows[1]!=36||rows[2]<400||rows.Skip(3).Sum()>115)throw new Exception("Rendered table allocation exceeded compact budget: "+string.Join(",",rows));});
        Check("shell preview navigation retains export gate",()=>{using var form=HiddenShell();var flags=System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic;var go=typeof(GuideForm).GetMethod("Go",flags)!;go.Invoke(form,new object[]{3});if((int)typeof(GuideForm).GetField("step",flags)!.GetValue(form)! !=3)throw new Exception("Reference preview inaccessible");go.Invoke(form,new object[]{4});if((int)typeof(GuideForm).GetField("step",flags)!.GetValue(form)! !=3||((Button)typeof(GuideForm).GetField("next",flags)!.GetValue(form)!).Enabled||((Button)typeof(GuideForm).GetField("export",flags)!.GetValue(form)!).Enabled)throw new Exception("Uninspected review/export enabled");});
        Check("shell reference preview fills remaining content",()=>{using var form=HiddenShell();typeof(GuideForm).GetMethod("Go",System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic)!.Invoke(form,new object[]{3});for(int i=0;i<3;i++)Layout(form);var root=(TableLayoutPanel)form.Controls[0];var viewport=(Panel)root.GetControlFromPosition(0,2)!;var page=viewport.Controls.OfType<TableLayoutPanel>().Single(x=>x.Dock==DockStyle.Fill);if(page.Height<viewport.ClientSize.Height-viewport.Padding.Vertical-2)throw new Exception("Reference content left unused height");});
        Check("shell Inspect ISO action and quiet hints",()=>{using var form=HiddenShell();var flags=System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic;var action=(Button)typeof(GuideForm).GetField("inspect",flags)!.GetValue(form)!;var hints=(ToolTip)typeof(GuideForm).GetField("tips",flags)!.GetValue(form)!;if(action.Text!="Inspect ISO"||hints.Active||hints.ShowAlways||hints.UseAnimation||hints.UseFading||hints.InitialDelay!=1500||hints.ReshowDelay!=1000||hints.AutoPopDelay!=3000)throw new Exception("Action label or hint policy changed");foreach(var name in new[]{"source","summary","review","status","reason"})if(!string.IsNullOrEmpty(hints.GetToolTip((Control)typeof(GuideForm).GetField(name,flags)!.GetValue(form)!)))throw new Exception("Large field or label has hover popup");if(!string.IsNullOrEmpty(hints.GetToolTip(form)))throw new Exception("Form has hover popup");});
        Check("shell Help and F1 binding policy",()=>{using var form=HiddenShell();var flags=System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic;var button=(Button)typeof(GuideForm).GetField("helpButton",flags)!.GetValue(form)!;if(button.Text!="?"||button.AccessibleName!="Help for this step"||!form.KeyPreview||!HelpPolicy.IsHelpShortcut(Keys.F1)||HelpPolicy.IsHelpShortcut(Keys.Escape))throw new Exception("Help action mismatch");});
        Check("shell header children and text stay contained",()=>{using var form=HiddenShell();var header=((TableLayoutPanel)form.Controls[0]).GetControlFromPosition(0,0)!;void CheckInside(Control parent){foreach(Control child in parent.Controls){if(child.Bounds.Left<0||child.Bounds.Top<0||child.Bounds.Right>parent.ClientSize.Width+1||child.Bounds.Bottom>parent.ClientSize.Height+1)throw new Exception("Header control outside bounds: "+child.GetType().Name+" "+child.Text);if(child is Label&&!string.IsNullOrEmpty(child.Text)){var measured=TextRenderer.MeasureText(child.Text,child.Font,new Size(Math.Max(1,child.Width),0),TextFormatFlags.WordBreak|TextFormatFlags.NoPrefix);if(measured.Height>child.Height+2)throw new Exception("Header label would clip: "+child.Text);}CheckInside(child);}}CheckInside(header);});
        Check("WIMGAPI duplicate field rejected",()=>Reject(()=>WimMetadata.Parse(xml.Replace("<ARCH>9</ARCH>","<ARCH>9</ARCH><ARCH>9</ARCH>"),1)));
        Check("WIMGAPI missing revision rejected",()=>Reject(()=>WimMetadata.Parse(xml.Replace("<SPBUILD>9457</SPBUILD>",""),1)));
        Check("WIMGAPI count mismatch rejected",()=>Reject(()=>WimMetadata.Parse(xml,2)));
        Check("WIMGAPI noncontiguous index rejected",()=>Reject(()=>WimMetadata.Parse(xml.Replace("INDEX=\"1\"","INDEX=\"2\""),1)));
        Check("WIMGAPI unknown architecture rejected",()=>Reject(()=>WimMetadata.Parse(xml.Replace("<ARCH>9</ARCH>","<ARCH>0</ARCH>"),1)));
        Check("WIMGAPI server rejected",()=>Reject(()=>WimMetadata.Parse(xml.Replace("<INSTALLATIONTYPE>Client</INSTALLATIONTYPE>","<INSTALLATIONTYPE>Server</INSTALLATIONTYPE>"),1)));
        Check("ARM64 metadata",()=>{if(Engine.ParseEdition(fixture.Replace("x64","arm64"),2).Architecture!="arm64")throw new Exception();});
        Check("localized fields rejected",()=>Reject(()=>Engine.ParseEdition(fixture.Replace("Architecture","Architektur"),2)));
        Check("duplicate field rejected",()=>Reject(()=>Engine.ParseEdition(fixture+"Index : 2\r\n",2)));
        Check("missing version rejected",()=>Reject(()=>Engine.ParseEdition(fixture.Replace("Version : 10.0.26200\r\n",""),2)));
        Check("index mismatch rejected",()=>Reject(()=>Engine.ParseEdition(fixture,1)));
        Check("filename does not determine build",()=>Reject(()=>Engine.ParseEdition(fixture.Replace("10.0.26200","not-a-version"),2)));
        Check("unknown architecture rejected",()=>Reject(()=>Engine.ParseEdition(fixture.Replace("x64","unknown"),2)));
        Check("x86 rejected",()=>Reject(()=>Engine.ParseEdition(fixture.Replace("x64","x86"),2)));
        Check("server rejected",()=>Reject(()=>Engine.ParseEdition(fixture.Replace("Installation : Client","Installation : Server"),2)));
        Check("forged version rejected",()=>Reject(()=>Engine.ParseEdition(fixture.Replace("10.0.26200","99.0.26200"),2)));
        Check("known profile",()=>{if(!Engine.Compatibility(26200,new[]{new Tool("ADK DISM","not-executed","10.0.26100.9457")}).StartsWith("Documented"))throw new Exception();});
        Check("preview profile unknown",()=>{if(!Engine.Compatibility(26300,new[]{new Tool("ADK DISM","not-executed","10.0.26100.9457")}).StartsWith("Unknown"))throw new Exception();});
        Check("missing ADK unknown",()=>{if(!Engine.Compatibility(26200,Array.Empty<Tool>()).StartsWith("Unknown"))throw new Exception();});
        Check("UNC rejected",()=>Reject(()=>Engine.ValidateSource(@"\\server\share\a.iso")));
        Check("URI rejected",()=>Reject(()=>Engine.ValidateSource("https://example.invalid/a.iso")));
        string folder=Path.Combine(Path.GetTempPath(),"ImageGuide-tests-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(folder);
        try{
            string path=Path.Combine(folder,"literal '[]&;.iso");File.WriteAllText(path,"fixture only; never mounted");
            Check("literal path accepted",()=>{if(Engine.ValidateSource(path)!=path)throw new Exception();});
            Check("source identity changes",()=>{string first=Engine.Hash(path);File.AppendAllText(path,"changed");if(first==Engine.Hash(path))throw new Exception();});
            Check("existing output rejected",()=>Reject(()=>Engine.NewFolder(folder)));
            Check("new output created once",()=>{string output=Path.Combine(folder,"new");Engine.NewFolder(output);Reject(()=>Engine.NewFolder(output));});
            var inspection=new Inspection(1,path,Engine.Hash(path),DateTimeOffset.UtcNow.ToString("O"),"HandleReleased","HandleScopedAttach",new(){Engine.ParseEdition(fixture,2)},new(),"Untrusted fixture","Test metadata only");
            string inspectionFile=Path.Combine(folder,"fixture-inspection.json");
            void Save(Inspection value)=>File.WriteAllText(inspectionFile,JsonSerializer.Serialize(value,Engine.Json));Save(inspection);
            Check("plan nonexecuting export",()=>{string plan=Engine.Plan(inspectionFile,new Choices(2,Updates:true),Path.Combine(folder,"plan")).GetAwaiter().GetResult();using var document=JsonDocument.Parse(File.ReadAllText(plan));if(document.RootElement.GetProperty("Executable").GetBoolean()||!document.RootElement.GetProperty("Choices").GetProperty("Updates").GetBoolean()||document.RootElement.GetProperty("Operations")[0].GetProperty("Status").GetString()!="NotImplemented")throw new Exception();});
            Check("plan missing index rejected",()=>Reject(()=>Engine.Plan(inspectionFile,new Choices(99),Path.Combine(folder,"bad-index")).GetAwaiter().GetResult()));
            Check("plan incomplete cleanup rejected",()=>{Save(inspection with{Cleanup="Unknown"});Reject(()=>Engine.Plan(inspectionFile,new Choices(2),Path.Combine(folder,"bad-cleanup")).GetAwaiter().GetResult());Save(inspection);});
            Check("plan forged version rejected",()=>{Save(inspection with{Editions=new(){inspection.Editions[0] with{Version="99.0.26200"}}});Reject(()=>Engine.Plan(inspectionFile,new Choices(2),Path.Combine(folder,"bad-version")).GetAwaiter().GetResult());Save(inspection);});
            Check("plan changed source rejected",()=>{File.AppendAllText(path,"different");Reject(()=>Engine.Plan(inspectionFile,new Choices(2),Path.Combine(folder,"bad-hash")).GetAwaiter().GetResult());});
        }finally{Directory.Delete(folder,true);}
        Console.WriteLine($"{passed} self-tests passed. No ISO mounted or serviced.");return 0;
    }
}
