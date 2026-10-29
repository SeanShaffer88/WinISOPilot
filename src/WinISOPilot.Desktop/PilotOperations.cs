using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using ImageGuide;
using Microsoft.Win32;
namespace WinISOPilot.Desktop;
public sealed partial class PilotWindow
{
    void BrowseSource()
    {
        if(busy)return;busy=true;UpdateChrome();try{var picker=new OpenFileDialog{Title="Choose your Windows 11 ISO",Filter="Windows ISO (*.iso)|*.iso",CheckFileExists=true,Multiselect=false};if(picker.ShowDialog(this)==true){busy=false;SelectSource(picker.FileName);}}finally{busy=false;Render();}
    }
    void SelectSource(string path)
    {
        if(busy)return;try{string canonical=Engine.ValidateSource(path);session.Invalidate();session.Source=canonical;sourceBytes=0;sourceModified=DateTime.MinValue;SetStatus("ISO selected. Choose Inspect ISO to read its real editions; no job started automatically.");Navigate(0);}catch(Exception ex){SetStatus("Cannot use this source: "+ex.Message);}
    }
    void CheckSource()
    {
        if(session.Inspection==null)return;try{var file=new FileInfo(session.Source);if(!file.Exists||file.Length!=sourceBytes||file.LastWriteTimeUtc!=sourceModified){session.Invalidate();SetStatus("Source changed or disappeared. Inspect it again before building.");Navigate(0);}}catch(Exception ex){session.Invalidate();SetStatus("Source could not be rechecked: "+ex.Message);Navigate(0);}
    }
    string? NewOutputFolder(string title)
    {
        var picker=new OpenFolderDialog{Title=title,Multiselect=false};return picker.ShowDialog(this)==true?Path.Combine(picker.FolderName,"WinISOPilot-"+DateTime.Now.ToString("yyyyMMdd-HHmmss")+"-"+Guid.NewGuid().ToString("N")[..6]):null;
    }
    bool Prepare()
    {
        if(busy)return false;if(Engine.RestartRequired){SetStatus("A native cleanup uncertainty requires restarting the app. Keep the evidence; no new work was started.");return false;}busy=true;critical=false;UpdateChrome();return true;
    }
    void BeginWork(string output)
    {
        job=new JobControl();lastOutput=output;session.Result=null;progress=null;elapsed=TimeSpan.Zero;clock.Restart();fullStatus="Job started. Source protection and engine verification remain active.";Render();
    }
    IProgress<JobProgress> ProgressFor(JobControl active)=>new UiProgress(this,active);
    sealed class UiProgress(PilotWindow owner,JobControl active):IProgress<JobProgress>
    {
        public void Report(JobProgress value){if(owner.Dispatcher.HasShutdownStarted)return;owner.Dispatcher.BeginInvoke(()=>{if(owner.job!=active)return;owner.progress=value;owner.critical=value.Phase.Contains("Final",StringComparison.OrdinalIgnoreCase)||value.Phase.Contains("Cleanup",StringComparison.OrdinalIgnoreCase)||value.Phase=="Releasing ISO handle"||value.Phase=="Verifying source identity";owner.fullStatus=value.Phase+": "+value.Message;owner.UpdateChrome();});}
    }
    void EndWork()
    {
        clock.Stop();elapsed=clock.Elapsed;job?.Dispose();job=null;busy=false;critical=false;CheckSource();Render();
    }
    async Task InspectSource()
    {
        if(session.Source.Length==0||!Prepare())return;string? output=null;bool canceled=false;Guid id=Guid.Empty;
        try{output=NewOutputFolder("Choose a parent folder for a NEW read-only inspection");if(output==null)return;session.Invalidate();BeginWork(output);var active=job!;id=active.RunId;var inspected=await Engine.Inspect(session.Source,output,active,ProgressFor(active));var file=new FileInfo(session.Source);sourceBytes=file.Length;sourceModified=file.LastWriteTimeUtc;session.CompleteInspection(inspected,Path.Combine(output,"inspection.json"));SetStatus($"Inspection complete: {inspected.Editions.Count} editions observed. Original unchanged. Confirm your selected edition.");}
        catch(OperationCanceledException){canceled=true;session.Invalidate();SetStatus("Inspection canceled safely after required verification and cleanup. No image was customized.");}
        catch(Exception ex){session.Invalidate();SetStatus("Inspection failed: "+ex.Message+". Keep reports for diagnosis.");}
        finally{EndWork();}
        if(canceled&&output!=null)OfferCleanup(output,id,false);else if(session.Inspection!=null){workspace=0;Render();FocusTarget("Edition list");}
    }
    async Task InspectContainer()
    {
        if(!Prepare())return;string? output=null;Guid id=Guid.Empty;bool canceled=false;
        try{var picker=new OpenFileDialog{Title="Inspect image containers without changing them",Filter="Windows images|*.iso;*.wim;*.esd;*.swm",CheckFileExists=true};if(picker.ShowDialog(this)!=true)return;output=NewOutputFolder("Choose a parent folder for a NEW container inventory");if(output==null)return;BeginWork(output);var active=job!;id=active.RunId;var result=await ContainerInventory.Inspect(picker.FileName,output,active,ProgressFor(active));clock.Stop();elapsed=clock.Elapsed;SetStatus($"Container inventory complete: {result.Containers.Count} containers / {result.Containers.Sum(x=>x.Images.Count)} indexes. Sources unchanged. No writable mount ran.");ShowText("Observed container inventory",JsonSerializer.Serialize(result,Engine.Json));}
        catch(OperationCanceledException){canceled=true;SetStatus("Container inventory canceled safely. Reports retained; guarded deletion is unavailable for this operation.");}
        catch(Exception ex){SetStatus("Container inventory failed: "+ex.Message+". Partial reports are retained.");}
        finally{EndWork();}
        if(canceled&&output!=null)OfferCleanup(output,id,false,true);
    }
    async Task BuildIso()
    {
        if(busy||session.Inspection==null||session.Edition==null||!session.EditionConfirmed)return;CheckSource();if(session.Inspection==null)return;
        if(!ConfirmBuild())return;CheckSource();if(session.Inspection==null||session.Edition==null||!session.EditionConfirmed)return;if(!Prepare())return;string? output=null;Guid id=Guid.Empty;bool canceled=false;
        try{output=NewOutputFolder("Choose a parent folder for a NEW ISO build — allow 3× source size + 1 GB");if(output==null)return;var inspected=session.Inspection;var choices=new MediaBuildChoices(session.EditionIndex,session.PreferEdition,session.ExecutableServicing);BeginWork(output);session.GuideStage=3;var active=job!;id=active.RunId;var created=await Engine.BuildIso(inspected,choices,output,active,ProgressFor(active));session.Result=created;session.GuideStage=4;SetStatus("Separate ISO created and read back. Original unchanged. Boot and Windows installation still need testing.");}
        catch(OperationCanceledException){canceled=true;session.Result=null;SetStatus("Build canceled after safe native completion and original-file verification. No completed ISO was published.");}
        catch(Exception ex){session.Result=null;SetStatus("ISO build failed: "+ex.Message+". Keep the run folder and its evidence. No output is approved for use.");}
        finally{EndWork();}
        if(canceled&&output!=null)OfferCleanup(output,id,true);if(session.Result!=null)Navigate(9);
    }
    async Task ExportPlan()
    {
        if(busy||session.InspectionPath==null||session.Edition==null||!Prepare())return;try{string? output=NewOutputFolder("Choose a parent folder for a NEW future-plan export");if(output==null)return;clock.Restart();lastOutput=output;string report=session.InspectionPath;int index=session.EditionIndex;string path=await Task.Run(()=>Engine.Plan(report,new Choices(index),output));SetStatus("Nonexecuting future plan exported: "+path+". No preview wish was executed.");}catch(Exception ex){SetStatus("Plan export failed: "+ex.Message);}finally{EndWork();}
    }
    bool ConfirmBuild()
    {
        bool approved=false;var panel=new StackPanel();panel.Children.Add(ReviewSection("Offline customization",ServicingSummary()));panel.Children.Add(ReviewSection("What will change",session.PreferEdition?"A separate ISO gets the selected edition preference (Retail). No license is supplied.":"A separate ISO is rebuilt with original setup settings."));panel.Children.Add(ReviewSection("What stays unchanged","Your original ISO and other editions. Only the selected edition is serviced when requested. No preview wish is applied and no running Windows setting is changed."));panel.Children.Add(ReviewSection("What has not been tested","Boot, installation, edition-preference behavior, activation, updates and recovery."));panel.Children.Add(ReviewSection("Output","Choose a new run-folder location next. Existing outputs will not be overwritten."));panel.Children.Add(Muted("Pause/cancel wait for safe checkpoints and native work. Cancel offers Keep by default; optional cleanup deletes only guarded eligible generated files. Undo never rewrites a completed ISO."));var dialog=Dialog("Review new ISO — original stays untouched",panel,800,610);var row=Row();row.Children.Add(DialogClose(dialog,"Go back",true));var create=ActionButton("Create new ISO",()=>{approved=true;dialog.Close();},true);row.Children.Add(create);panel.Children.Add(row);dialog.ShowDialog();return approved;
    }
    void CleanupDialog(string output,Guid id,bool build,bool inventory=false)
    {
        clock.Stop();var preview=inventory?new CleanupPreview(false,[],0,["Reports kept. This inventory has no guarded deletion journal; automatic cleanup is unavailable."]):build?Engine.PreviewBuildCleanup(output,id):Engine.PreviewCanceledCleanup(output,id);var panel=new StackPanel();panel.Children.Add(Text("Keep files is the default.",22));panel.Children.Add(Muted($"{preview.Files.Count} unchanged generated files ({preview.TotalBytes:N0} bytes) are eligible. Folders, changed files and unrelated entries are retained."));panel.Children.Add(TextBoxRead(output+"\n"+string.Join("\n",preview.Files.Select(x=>x.RelativePath+" ("+x.Bytes+" bytes)").Concat(preview.Warnings)),280));var dialog=Dialog(inventory?"Canceled inventory — reports kept":"Canceled job — keep leftover files?",panel,820,600);var actions=Row();actions.Children.Add(DialogClose(dialog,"Keep files (recommended)",true));var remove=ActionButton("Delete eligible files from this run",()=>{var outcome=build?Engine.CleanupBuild(output,id):Engine.CleanupCanceledJob(output,id);ShowText("Cleanup result",$"Removed: {outcome.Removed.Count}\nRetained: {outcome.Retained.Count}\nErrors: {outcome.Errors.Count}\n"+string.Join("\n",outcome.Retained.Concat(outcome.Errors)),dialog);dialog.Close();});remove.IsEnabled=preview.CanDelete&&!inventory;actions.Children.Add(remove);panel.Children.Add(actions);dialog.ShowDialog();
    }
    void OfferCleanup(string output,Guid id,bool build,bool inventory=false){try{CleanupDialog(output,id,build,inventory);}catch(Exception ex){SetStatus("Leftover files retained. Cleanup could not be confirmed: "+ex.Message);}}
    void SavePreview()
    {
        if(busy)return;var picker=new SaveFileDialog{Title="Export a NEW nonexecuting preview recipe",Filter="JSON preview|*.json",AddExtension=true,OverwritePrompt=true};if(picker.ShowDialog(this)!=true)return;try{Engine.CheckPath(picker.FileName);using var file=new FileStream(picker.FileName,FileMode.CreateNew,FileAccess.Write,FileShare.None);using var writer=new StreamWriter(file);writer.Write(PreviewRecipe.Serialize(session.Preview));SetStatus("Preview recipe saved. No image changes were made.");}catch(Exception ex){SetStatus("Preview export failed: "+ex.Message);}
    }
    void LoadPreview()
    {
        if(busy)return;var picker=new OpenFileDialog{Title="Import validated preview JSON",Filter="JSON preview|*.json",CheckFileExists=true};if(picker.ShowDialog(this)!=true)return;try{Engine.CheckPath(picker.FileName);using var file=new FileStream(picker.FileName,FileMode.Open,FileAccess.Read,FileShare.Read);if(file.Length>65536)throw new InvalidDataException("Preview recipes are limited to 64 KiB.");using var reader=new StreamReader(file);var parsed=PreviewRecipe.Parse(reader.ReadToEnd());comparison=session.Preview;Mutate(()=>session.Preview=parsed);SetStatus("Preview imported. No commands were generated or executed.");}catch(Exception ex){SetStatus("Preview import rejected; choices unchanged: "+ex.Message);}
    }
}
