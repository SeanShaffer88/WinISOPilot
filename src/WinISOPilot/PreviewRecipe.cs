using System.Text.Json;
using System.Text.Json.Serialization;
namespace ImageGuide;

public sealed record PreviewOption(string Id,string Group,string Name,string Consequence);
public sealed record PreviewRecipe(int SchemaVersion,bool Executable,string[] Requested,Dictionary<string,string> Services,string Region)
{
    public static PreviewRecipe Empty=>new(1,false,[],new(StringComparer.Ordinal),"Keep source region");
    public static readonly PreviewOption[] Options=[
        new("tpm","Hardware","TPM check override","Build-specific and unverified. May prevent supported installation."),
        new("secure-boot","Hardware","Secure Boot check override","Does not enable Secure Boot or establish hardware support."),
        new("ram","Hardware","Memory check override","Does not make low-memory devices reliable."),
        new("cpu","Hardware","Processor check override","Build-specific; unsupported hardware remains unsupported."),
        new("storage","Hardware","Storage check override","Does not create required disk space."),
        new("local-account","Hardware","Local account setup preference","OOBE behavior varies by Windows build. Not implemented."),
        new("encryption","Hardware","Prevent automatic device encryption","Would change a security default. No change runs here."),
        new("xbox","Apps","Xbox apps","Actual package identities and dependencies have not been inventoried."),
        new("phone-link","Apps","Phone Link","Removal is not implemented or assessed for this image."),
        new("copilot","Apps","Copilot","Installed form varies by image and build."),
        new("clipchamp","Apps","Clipchamp","No package footprint or removal safety has been measured."),
        new("news","Apps","News and Weather","Dependencies and applicability are unknown."),
        new("solitaire","Apps","Solitaire","No installed package inventory has run."),
        new("cleanup","Components","Superseded component cleanup","Requires supported servicing and post-change verification."),
        new("resetbase","Components","ResetBase cleanup","Would remove rollback for existing updates. Not reversible by selection Undo."),
        new("wordpad","Components","Legacy WordPad","May already be absent on newer releases. Applicability unknown."),
        new("media-player","Components","Legacy Media Player","Features and dependencies must be inventoried first."),
        new("ie-mode","Components","Internet Explorer compatibility components","Could affect applications that depend on embedded web components."),
        new("winre","Components","Windows Recovery Environment","Would remove recovery functionality. No removal runs here."),
        new("context-menu","Registry","Classic context menu preference","Version-dependent shell behavior; not verified."),
        new("bing-search","Registry","Reduce web results in Start","Policy support and edition applicability vary."),
        new("telemetry","Registry","Diagnostic-data preference","A registry value does not guarantee zero diagnostic data on every edition."),
        new("extensions","Registry","Show file extensions and hidden files","Preview wish only; no offline hive is loaded."),
        new("dark-theme","Registry","Dark Windows theme preference","Does not change this app's independent theme toggle."),
        new("lock-ads","Registry","Reduce lock-screen suggestions","Applicability is build-specific and not checked."),
        new("delivery","Registry","Delivery Optimization preference","Does not disable Windows Update; no policy is applied."),
        new("this-pc","Registry","Open Explorer at This PC","User-default behavior would need first-boot testing."),
        new("drivers","Integration","Stage INF driver packages","No files selected, signature verification or image injection occurs."),
        new("updates","Integration","Stage CAB/MSU updates","Prerequisite ordering and compatibility are not evaluated.")];
    public static readonly string[] ServiceNames=["DiagTrack","WerSvc","MapsBroker","RemoteRegistry","WbioSrvc","Spooler","SysMain"];
    public static readonly string[] Regions=["Keep source region","Preview: English (United States)","Preview: English (United Kingdom)"];
    public static readonly string[] StartupStates=["Unchanged","Automatic","Manual","Disabled"];
    static readonly JsonSerializerOptions Json=new(){WriteIndented=true,UnmappedMemberHandling=JsonUnmappedMemberHandling.Disallow};
    public static PreviewRecipe Validate(PreviewRecipe recipe)
    {
        if(recipe.SchemaVersion!=1||recipe.Executable||recipe.Requested==null||recipe.Services==null||recipe.Requested.Length>Options.Length||recipe.Services.Count>ServiceNames.Length||!Regions.Contains(recipe.Region))throw new InvalidDataException("Unsupported preview recipe schema or values.");
        if(recipe.Requested.Distinct(StringComparer.Ordinal).Count()!=recipe.Requested.Length||recipe.Requested.Any(id=>!Options.Any(x=>x.Id==id))||recipe.Services.Any(x=>!ServiceNames.Contains(x.Key)||!StartupStates.Contains(x.Value)))throw new InvalidDataException("Unknown or duplicate preview option. No changes imported.");
        return new(1,false,recipe.Requested.Order(StringComparer.Ordinal).ToArray(),recipe.Services.OrderBy(x=>x.Key,StringComparer.Ordinal).ToDictionary(x=>x.Key,x=>x.Value,StringComparer.Ordinal),recipe.Region);
    }
    public static string Serialize(PreviewRecipe recipe)=>JsonSerializer.Serialize(Validate(recipe),Json);
    public static PreviewRecipe Parse(string json)
    {
        if(json.Length>65536)throw new InvalidDataException("Preview recipes are limited to 64 KiB.");
        try{using var doc=JsonDocument.Parse(json,new JsonDocumentOptions{MaxDepth=12});void Unique(JsonElement e){if(e.ValueKind==JsonValueKind.Object){var seen=new HashSet<string>(StringComparer.Ordinal);foreach(var p in e.EnumerateObject()){if(!seen.Add(p.Name))throw new InvalidDataException("Duplicate JSON property.");Unique(p.Value);}}else if(e.ValueKind==JsonValueKind.Array)foreach(var item in e.EnumerateArray())Unique(item);}Unique(doc.RootElement);return Validate(JsonSerializer.Deserialize<PreviewRecipe>(json,Json)??throw new InvalidDataException("Empty preview recipe."));}catch(JsonException ex){throw new InvalidDataException("Invalid preview recipe JSON.",ex);}
    }
    public static string Diff(PreviewRecipe before,PreviewRecipe after)
    {
        before=Validate(before);after=Validate(after);var lines=new List<string>{"PREVIEW DIFF — no commands or image changes",$"Region: {before.Region} → {after.Region}"};
        foreach(var option in Options){bool old=before.Requested.Contains(option.Id),next=after.Requested.Contains(option.Id);if(old!=next)lines.Add($"{option.Name}: {(old?"Requested":"Keep")} → {(next?"Requested":"Keep")}");}
        foreach(string service in ServiceNames){string old=before.Services.GetValueOrDefault(service,"Unchanged"),next=after.Services.GetValueOrDefault(service,"Unchanged");if(old!=next)lines.Add($"{service}: {old} → {next}");}if(lines.Count==2)lines.Add("No option or service changes.");return string.Join(Environment.NewLine,lines);
    }
}
