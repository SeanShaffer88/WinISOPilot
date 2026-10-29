using System.Windows;
using System.Windows.Controls;
using ImageGuide;

namespace WinISOPilot.Desktop;
public sealed partial class PilotWindow
{
    string ServicingSummary()
    {
        if (session.ExecutableServicing == null) return "No executable app removal or component cleanup requested. Preview wishes remain nonexecuting.";
        return "Selected edition only: " + (session.Edition?.Name ?? "unknown") + "\nRemove provisioning: " + (session.RemoveApps.Length == 0 ? "none" : string.Join("\n", session.RemoveApps)) +
            "\nClean replaced Windows components: " + (session.CleanupComponents ? "Yes — StartComponentCleanup; no ResetBase" : "No") +
            "\nOther editions remain. Original ISO stays untouched. Undo changes choices; after a build, create another copy from the original to change the result. Pause/cancel waits for native DISM completion. Boot/install/update/recovery tests have not run.";
    }
    UIElement ServicingAppsPage()
    {
        var page = Page("Apps in your selected edition", "Read actual app identities, then choose removals for a separate ISO. Your running Windows apps and original ISO are not changed.");
        var readiness = Engine.CheckServicing(session.Edition);
        var summary = new StackPanel();
        summary.Children.Add(Muted(readiness.Available ? "Tool profile available. The engine will check source, WIM format, space and ownership again before work." : string.Join("\n", readiness.Reasons)));
        var read = ActionButton("Read apps in this edition", () => _ = ReadApps(), true, "Read apps");
        read.IsEnabled = !busy && session.EditionConfirmed && session.Inspection != null && readiness.Available;
        summary.Children.Add(read);
        summary.Children.Add(Muted("This copies install.wim to a private folder and reads a read-only mount. Keep is the default after cancellation; native-created files are retained when safe deletion is unproven."));
        page.Children.Add(Card("1. Read your edition", summary, readiness.Available ? session.AppInventory == null ? "NEEDS INVENTORY" : "OBSERVED INVENTORY" : "BLOCKED — NO SERVICING STARTED"));
        if (session.AppInventory == null)
        {
            page.Children.Add(Card("2. Choose observed apps", Muted("No app inventory is loaded. No catalog label or imported preview can authorize app removal. The first removal list covers Clipchamp, Solitaire, News, Weather, Get Help and Get Started when actually present."), "NOT READ"));
            return page;
        }
        var list = new StackPanel();
        list.Children.Add(Muted("Filter observed app identities"));
        var filter = new TextBox(); Register(filter, "Filter observed apps", "Filter actual provisioned package identities"); list.Children.Add(filter);
        var rows = new StackPanel(); list.Children.Add(rows);
        void Populate()
        {
            rows.Children.Clear();
            foreach (var app in session.AppInventory.Apps.Where(x => (x.DisplayName + " " + x.PackageName).Contains(filter.Text, StringComparison.OrdinalIgnoreCase)))
            {
                var body = new StackPanel();
                var choice = new CheckBox { Content = "Remove from new users of this edition", IsChecked = session.RemoveApps.Contains(app.PackageName, StringComparer.Ordinal), IsEnabled = !busy && app.Removable };
                Register(choice, "app:" + app.PackageName, app.PackageName + "; " + app.Explanation);
                choice.Click += (_, _) => Mutate(() => session.RemoveApps = choice.IsChecked == true ? session.RemoveApps.Append(app.PackageName).Distinct(StringComparer.Ordinal).ToArray() : session.RemoveApps.Where(x => x != app.PackageName).ToArray());
                body.Children.Add(choice); body.Children.Add(Muted(app.PackageName)); body.Children.Add(Muted(app.Explanation));
                rows.Children.Add(Card(app.DisplayName, body, app.Removable ? "OBSERVED — OPTIONAL REMOVAL" : "KEEP — PROTECTED / NOT SUPPORTED"));
            }
            if (rows.Children.Count == 0) rows.Children.Add(Muted("No observed apps match this filter."));
        }
        filter.TextChanged += (_, _) => Populate(); Populate();
        page.Children.Add(Card("2. Choose observed apps", list, "NOT A GENERAL DEPENDENCY CHECK"));
        return page;
    }
    UIElement ServicingComponentsPage()
    {
        var page = Page("Clean replaced Windows components", "Use the supported Windows component-store cleanup on the selected edition's private copy. No manual system-file stripping runs.");
        var panel = new StackPanel();
        var readiness = Engine.CheckServicing(session.Edition);
        var cleanup = new CheckBox { Content = "Clean replaced Windows components in the new ISO", IsChecked = session.CleanupComponents, IsEnabled = !busy && session.AppInventory != null && readiness.Available };
        Register(cleanup, "servicing:cleanup", "StartComponentCleanup on the selected edition only. No ResetBase; savings are not promised.");
        cleanup.Click += (_, _) => Mutate(() => session.CleanupComponents = cleanup.IsChecked == true);
        panel.Children.Add(cleanup);
        panel.Children.Add(Muted("DISM /StartComponentCleanup removes eligible superseded components. It may make no size difference. /ResetBase is never used. Undo changes this request; it cannot reverse cleanup in an already created ISO. Rebuild from the original instead."));
        panel.Children.Add(Muted(session.AppInventory == null ? "Read apps in the Apps workspace first. That binds these executable choices to your source, edition and tool." : "Bound to the observed app inventory for your selected edition."));
        if (!readiness.Available) panel.Children.Add(Muted(string.Join("\n", readiness.Reasons)));
        panel.Children.Add(ActionButton("Go to Apps inventory", () => Navigate(2)));
        page.Children.Add(Card("Supported cleanup", panel, session.AppInventory != null && readiness.Available ? "AVAILABLE — SEPARATE COPY ONLY" : "NEEDS SERVICING PREFLIGHT / INVENTORY"));
        page.Children.Add(Card("Other component removal", Muted("Arbitrary component stripping, recovery removal, ResetBase, optional-feature payload removal and registry ownership changes remain unavailable. No dependency graph or safe-removal score is invented."), "NOT IMPLEMENTED"));
        return page;
    }
    async Task ReadApps()
    {
        if (busy || session.Inspection == null || session.Edition == null || !session.EditionConfirmed) return;
        CheckSource(); if (session.Inspection == null || !Prepare()) return;
        string? output = null; bool canceled = false; Guid id = Guid.Empty;
        try
        {
            output = NewOutputFolder("Choose a parent folder for a NEW private app inventory — local NTFS, ample space required");
            if (output == null) return;
            var inspection = session.Inspection; int index = session.EditionIndex;
            session.ClearServicing(); BeginWork(output); var active = job!; id = active.RunId;
            var result = await Engine.ReadServicingApps(inspection, index, output, active, ProgressFor(active));
            session.CompleteAppInventory(result); SetStatus($"Read {result.Apps.Length} provisioned apps. Original verified unchanged. Choose removals explicitly; no image customization has run.");
        }
        catch (OperationCanceledException) { canceled = true; session.ClearServicing(); SetStatus("App reading canceled. Original verification and owned mount release completed. Keep native-generated files."); }
        catch (Exception ex) { session.ClearServicing(); SetStatus("Apps could not be read: " + ex.Message + ". Keep all evidence; no removal was approved."); }
        finally { EndWork(); }
        if (canceled && output != null) OfferCleanup(output, id, false, true);
        Navigate(2);
    }
}
