using ImageGuide;
namespace WinISOPilot.Desktop;
public sealed record Selection(int EditionIndex,bool PreferEdition,string Preview,string Apps="",bool Cleanup=false);
public sealed record SearchEntry(string Id,string Title,string Description,int Workspace);
public sealed class Session
{
    public string Source{get;set;}="";public Inspection? Inspection{get;set;}public string? InspectionPath{get;set;}public int EditionIndex{get;set;}public bool EditionConfirmed{get;set;}public bool PreferEdition{get;set;}public PreviewRecipe Preview{get;set;}=PreviewRecipe.Empty;public MediaBuildResult? Result{get;set;}public int GuideStage{get;set;}
    public ServicingInventory? AppInventory{get;set;}
    public string[] RemoveApps{get;set;}=[];public bool CleanupComponents{get;set;}
    public ServicingChoices? ExecutableServicing=>AppInventory!=null&&(RemoveApps.Length>0||CleanupComponents)?new(AppInventory.Id,RemoveApps.ToArray(),CleanupComponents):null;
    public void ClearServicing(){AppInventory=null;RemoveApps=[];CleanupComponents=false;}
    public void CompleteAppInventory(ServicingInventory inventory){ClearServicing();AppInventory=inventory;history.Clear();}
    readonly List<Selection> history=new();public int UndoCount=>history.Count;
    public Selection Capture()=>new(EditionIndex,PreferEdition,PreviewRecipe.Serialize(Preview),string.Join("\n",RemoveApps.Order(StringComparer.Ordinal)),CleanupComponents);
    public void Change(Action change){var old=Capture();try{change();Preview=PreviewRecipe.Validate(Preview);}catch{EditionIndex=old.EditionIndex;PreferEdition=old.PreferEdition;Preview=PreviewRecipe.Parse(old.Preview);RemoveApps=old.Apps.Length==0?[]:old.Apps.Split('\n');CleanupComponents=old.Cleanup;throw;}if(Capture()==old)return;history.Add(old);if(history.Count>100)history.RemoveAt(0);if(old.EditionIndex!=EditionIndex){EditionConfirmed=false;ClearServicing();}ClearOutcome();}
    public void Undo(){if(history.Count==0)return;var previous=history[^1];history.RemoveAt(history.Count-1);bool changedEdition=EditionIndex!=previous.EditionIndex;if(changedEdition)EditionConfirmed=false;EditionIndex=previous.EditionIndex;PreferEdition=previous.PreferEdition;Preview=PreviewRecipe.Parse(previous.Preview);RemoveApps=previous.Apps.Length==0?[]:previous.Apps.Split('\n');CleanupComponents=previous.Cleanup;if(changedEdition)ClearServicing();ClearOutcome();}
    void ClearOutcome(){Result=null;if(GuideStage>=4)GuideStage=Inspection==null?0:EditionConfirmed?3:1;}
    public void Invalidate(){ClearServicing();Inspection=null;InspectionPath=null;EditionIndex=0;EditionConfirmed=false;Result=null;GuideStage=0;history.Clear();}
    public void CompleteInspection(Inspection inspection,string path){ClearServicing();Inspection=inspection;InspectionPath=path;EditionIndex=inspection.Editions.FirstOrDefault()?.Index??0;EditionConfirmed=false;GuideStage=1;Result=null;history.Clear();}
    public Edition? Edition=>Inspection?.Editions.SingleOrDefault(x=>x.Index==EditionIndex);
    public int ChangeCount=>Preview.Requested.Length+Preview.Services.Count+(Preview.Region==PreviewRecipe.Regions[0]?0:1)+(PreferEdition?1:0)+RemoveApps.Length+(CleanupComponents?1:0);
    public string Capability(int workspace,bool safetyLatch)
    {
        if(safetyLatch)return "Needs attention";
        if(workspace is 2 or 3)
        {
            if(Edition==null||!EditionConfirmed)return "Needs confirmed edition";
            var readiness=Engine.CheckServicing(Edition);
            if(!readiness.Available)return !Engine.IsAdministrator?"Needs administrator":"Unsupported / recovery needed";
            return AppInventory==null?"Needs app inventory":"Available • selected edition";
        }
        return workspace==0?"Available":workspace==9?Inspection!=null&&Edition!=null?"Available":"Needs inspection":workspace==8?"Available • preview data":"Preview only";
    }

    public void Remove(string id){Change(()=>{if(id=="servicing:cleanup")CleanupComponents=false;else if(id.StartsWith("app:",StringComparison.Ordinal))RemoveApps=RemoveApps.Where(x=>x!=id[4..]).ToArray();else if(id=="media:edition")PreferEdition=false;else if(id=="region")Preview=Preview with{Region=PreviewRecipe.Regions[0]};else if(id.StartsWith("option:",StringComparison.Ordinal))Preview=Preview with{Requested=Preview.Requested.Where(x=>x!=id[7..]).ToArray()};else if(id.StartsWith("service:",StringComparison.Ordinal))Preview=Preview with{Services=Preview.Services.Where(x=>x.Key!=id[8..]).ToDictionary(x=>x.Key,x=>x.Value,StringComparer.Ordinal)};});}
    public static IReadOnlyList<SearchEntry> Search(string query)
    {
        if(string.IsNullOrWhiteSpace(query))return[];query=query.Trim();var entries=new List<SearchEntry>();for(int i=0;i<PilotWindow.WorkspaceNames.Length;i++)entries.Add(new("workspace:"+i,PilotWindow.WorkspaceNames[i],"Open workspace; navigation only",i));
        entries.Add(new("Inspect ISO","Inspect ISO","Read-only source inspection; choose a local Windows installation file",0));entries.Add(new("Create new ISO","Create new ISO","Build a separate copy after confirmed inspection and explicit review",9));entries.Add(new("region","Region preference","Unattended regional locale preview, not an answer file",7));
        entries.AddRange(PreviewRecipe.Options.Select(x=>new SearchEntry("option:"+x.Id,x.Name,x.Consequence,x.Group switch{"Hardware"=>1,"Apps"=>2,"Components"=>3,"Registry"=>4,_=>6})));var descriptions=new Dictionary<string,string>{{"DiagTrack","Diagnostic tracking telemetry"},{"WerSvc","Windows error reporting"},{"MapsBroker","Downloaded maps"},{"RemoteRegistry","Remote registry access"},{"WbioSrvc","Biometric Windows Hello sign-in"},{"Spooler","Printing print spooler"},{"SysMain","Memory prefetch startup"}};entries.AddRange(PreviewRecipe.ServiceNames.Select(x=>new SearchEntry("service:"+x,x,descriptions[x]+" — service startup preview, not a Windows change",5)));entries.AddRange(CapabilityCatalog.Items.Select(x=>new SearchEntry("reference:"+x.Name,x.Name,"Research reference: "+x.Purpose+" "+x.Prerequisites+" "+x.Consequence,0)));return entries.Where(x=>(x.Title+" "+x.Description).Contains(query,StringComparison.OrdinalIgnoreCase)).Take(50).ToArray();
    }
}
