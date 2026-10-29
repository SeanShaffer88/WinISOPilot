namespace ImageGuide;

public static class HelpPolicy
{
    public const string InspectAction="Inspect ISO";
    public const string CoreMessage="This app looks inside your Windows installation file. It does not change it. Your choices make a plan for future changes.";
    public const bool HintsEnabledByDefault=false;
    public const int InitialDelay=1500,ReshowDelay=1000,DisplayDuration=3000;
    public static bool IsHelpShortcut(Keys key)=>key==Keys.F1;
    public static bool CanHaveHint(Control control)=>control is Button or CheckBox or ComboBox;
    public static string ShortHint(string explanation)
    {
        int end=explanation.IndexOf(". ",StringComparison.Ordinal);string sentence=end>=0?explanation[..(end+1)]:explanation;
        return sentence.Length<=120?sentence:"Open Help for this action's explanation and requirements.";
    }
    public static string Availability(string status)=>status switch{CapabilityCatalog.ReadOnly=>"Can read information now",CapabilityCatalog.Future=>"Not available yet",CapabilityCatalog.Unsupported=>"Not supported in this version",_=>status};
    public static string VersionCheck(string raw)=>raw.StartsWith("Unknown",StringComparison.Ordinal)?"Changes not checked for this Windows version.":"A documented tool profile was found; changes have not been tested.";
    public static string Phase(string raw)=>raw switch{"Hashing source"=>"Checking the file","Querying editions" or "Reading WIM metadata"=>"Reading Windows versions inside the file","Attaching ISO"=>"Opening the file read-only","Releasing ISO handle"=>"Finishing safe file access","Verifying source identity"=>"Checking the file stayed unchanged","Finalizing"=>"Finishing",_=>raw};
}
