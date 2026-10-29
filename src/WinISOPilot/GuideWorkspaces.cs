using System.Diagnostics;
using System.Text.Json;
namespace ImageGuide;

public sealed partial class GuideForm
{
    readonly TextBox overview=ReadBox(145),telemetry=ReadBox(125),simulation=ReadBox(210),recovery=ReadBox(250);
    readonly ComboBox previewRegion=new(){DropDownStyle=ComboBoxStyle.DropDownList,Dock=DockStyle.Fill};
    Button? containerAction;readonly Stopwatch jobClock=new();JobProgress? lastProgress;TimeSpan lastElapsed;string containerSummary="No container inspection has run.";
    TableLayoutPanel OverviewPage()
    {
        var p=Page("Overview","Your original files remain protected.","Start with Source, choose an edition, then review Build & Export.");
        Field(p,"Current session • measured state",overview,"Actual inspected source and choices, never sample data.");
        Field(p,"Live job activity • measured only",telemetry,"Observed phase, bytes and elapsed time. No predicted completion time or invented savings.");
        var actions=Flow();var start=Button("Choose a source");start.Click+=(_,_)=>OpenWorkspace(Workspace.ImageIndexes);Help(start,"Open the Source workspace. No job starts.");var recipe=Button("Preview recipe");recipe.Click+=(_,_)=>OpenWorkspace(Workspace.Unattended);Help(recipe,"Open the nonexecuting recipe simulation.");actions.Controls.Add(start);actions.Controls.Add(recipe);p.Controls.Add(actions);return p;
    }
    TableLayoutPanel ReferencePage(string title,params string[] groups)
    {
        var p=Page(title+" • reference preview","These are researched possibilities, not your image's installed items.","Search and filter. Actual removal and integration are not implemented.");p.Dock=DockStyle.Fill;p.AutoSize=false;p.RowCount=4;for(int i=0;i<4;i++)p.RowStyles.Add(new RowStyle(i==3?SizeType.Percent:SizeType.AutoSize,i==3?100:0));p.Controls.Add(Catalog(groups),0,3);return p;
    }
    TableLayoutPanel SimulationPage()
    {
        var p=Page("Unattended Setup • simulation only","This preview uses the edition and goals chosen in other workspaces.","Review checks. No answer file, account, password, disk layout or command is generated.");
        previewRegion.Items.AddRange(PreviewRecipe.Regions);previewRegion.SelectedIndex=0;previewRegion.SelectedIndexChanged+=(_,_)=>Changed();Field(p,"Region wish • preview only",previewRegion,"This changes this simulation only; it does not change Windows or exported build settings.");
        var refresh=Button("Simulate current recipe");Help(refresh,"Recalculate prerequisite checks from current choices. Does not execute commands or estimate savings.");refresh.Click+=(_,_)=>RefreshWorkspaceState();p.Controls.Add(refresh);Field(p,"Simulation result • no changes execute",simulation,"Declarative preview and checks. Not a build manifest and never passed to the image engine.");return p;
    }
    TableLayoutPanel RecoveryPage()
    {
        var p=Page("Recovery & Logs","Current observations and recovery limits.","Read full operation details when work fails. Unknown mounts are never cleaned automatically.");Field(p,"Readiness and observations",recovery,"No unmount or recovery command executes here. Real mount support remains unavailable.");var details=Button("Open operation evidence");Help(details,"Show the latest full operation message. Logs may include private local paths.");details.Click+=(_,_)=>ShowDetails("Operation evidence",fullStatus.Length==0?"No operation has reported evidence yet.":fullStatus);p.Controls.Add(details);return p;
    }
    void BeginTelemetry(){lastProgress=null;lastElapsed=TimeSpan.Zero;jobClock.Restart();RefreshWorkspaceState();}
    void ObserveTelemetry(JobProgress value){lastProgress=value;RefreshWorkspaceState();}
    void EndTelemetry(){if(jobClock.IsRunning){jobClock.Stop();lastElapsed=jobClock.Elapsed;}RefreshWorkspaceState();}
    void RefreshWorkspaceState()
    {
        if(IsDisposed)return;RefreshReferenceState();RefreshGuidance();var selected=editions.SelectedItem as Edition;string[] selectedGoals=Keys.Where(k=>goals.TryGetValue(k,out var c)&&c.Checked).ToArray();
        overview.Text=$"State: {(busy?"Working — selections locked":"Idle")}\r\nISO: {(result==null?"Not inspected":Path.GetFileName(result.Source))}\r\nEdition: {selected?.Name??"None selected"}\r\nAvailable: inspection, container inventory, separate ISO creation.\r\nPreview: recipe checks. Servicing and slimming: not implemented.";
        var elapsed=jobClock.IsRunning?jobClock.Elapsed:lastElapsed;string measured=lastProgress?.Completed is long done?$"{done:N0} bytes observed"+(lastProgress.Total is long total?$" / {total:N0} in this phase":""):"Byte progress: not reported for this phase";
        telemetry.Text=lastProgress==null?"No measured job activity yet.\r\nNo ETA or size savings are inferred.":$"Phase: {lastProgress.Phase}\r\n{measured}\r\nElapsed: {elapsed:hh\\:mm\\:ss} (includes safe pauses)\r\n{(job?.IsPaused==true?"Paused at a safe checkpoint":job?.CancellationRequested==true?"Cancellation requested; waiting for a safe boundary":jobClock.IsRunning?"Active":"Last observed job has ended; see operation evidence for outcome")}";
        simulation.Text=RecipePreview.Create(result!=null,selected?.Name,selectedGoals,preferEdition.Checked,previewRegion.SelectedItem?.ToString()??"Keep source region",result?.Compatibility)+"\r\n\r\n"+PreviewRecipe.Diff(PreviewRecipe.Empty,CapturePreview());
        previewRegion.Enabled=!busy;
        recovery.Text=$"Image mounting: not implemented; no privileged worker runs.\r\nElevation: {(ContainerInventory.IsElevated?"Elevated token observed":"Standard token observed; mounting requires elevation")}\r\nServicing profile: {result?.Compatibility??"Unknown until an ISO is inspected"}\r\nScratch expansion: unknown; writable servicing is blocked.\r\nRecovery: no forced unmount, global mount cleanup or unrelated deletion.\r\nNative safety latch: {(Engine.RestartRequired?"Restart required; keep evidence":"No unresolved native cleanup reported in this process")}\r\n\r\nContainer inventory: {containerSummary}";
    }
    async Task InspectContainers()
    {
        if(busy)return;using var picker=new OpenFileDialog{Title="Inspect image containers without changing them",Filter="Windows images|*.iso;*.wim;*.esd;*.swm",CheckFileExists=true};if(picker.ShowDialog(this)!=DialogResult.OK)return;string? folder=NewFolder("Choose a parent folder for a NEW container inventory");if(folder==null)return;
        using var active=new JobControl();job=active;busy=true;critical=false;activity=Activity.Inspecting;BeginTelemetry();RefreshActions();
        try{var progress=new Progress<JobProgress>(p=>{if(job!=active)return;ObserveTelemetry(p);critical=p.Phase.StartsWith("Final",StringComparison.Ordinal);SetStatus(p.Phase+": "+p.Message,p.Phase);RefreshActions();});var observed=await ContainerInventory.Inspect(picker.FileName,folder,active,progress);EndTelemetry();containerSummary=$"{observed.Containers.Count} containers / {observed.Containers.Sum(c=>c.Images.Count)} indexes; originals unchanged. WinRE inside install images was not searched.";SetStatus("Container inventory complete: "+folder+"\n"+containerSummary);ShowDetails("Container inventory — self-reported metadata",JsonSerializer.Serialize(observed,Engine.Json));}
        catch(OperationCanceledException){EndTelemetry();SetStatus("Container inventory canceled safely. Reports retained; automatic cleanup is unavailable for this operation.");using var kept=BuildCleanupDialog(folder,active.RunId,new CleanupPreview(false,new(),0,new(){"Reports kept. This operation has no guarded deletion journal; automatic cleanup is unavailable."}));kept.Text="Canceled inventory — reports kept";kept.ShowDialog(this);}
        catch(Exception ex){SetStatus("Container inventory failed: "+ex.Message+". Partial reports are retained.");}
        finally{EndTelemetry();job=null;busy=false;critical=false;activity=Activity.Idle;RefreshActions();RefreshWorkspaceState();}
    }
}

public static class RecipePreview
{
    public static string Create(bool inspected,string? edition,IEnumerable<string> goals,bool prefer,string region,string? compatibility)
    {
        string[] requested=goals.ToArray();return $"SIMULATION ONLY — no changes execute\r\nSource check: {(inspected?"inspected ISO present":"blocked — inspect an ISO first")}\r\nEdition: {edition??"not selected"}\r\nSeparate ISO preference: {(prefer?"Retail selected edition; no license provided":"unchanged")}\r\nRegion wish: {region} (not applied)\r\n"+string.Join("\r\n",requested.Select(x=>$"{x}: NOT IMPLEMENTED — inventory, dependencies and compatible servicing required"))+$"\r\nServicing check: {compatibility??"unknown"}\r\nDisk formatting, scripts, security changes: no actions generated.\r\nSize savings, install behavior and boot success: unknown, not simulated.";
    }
}
