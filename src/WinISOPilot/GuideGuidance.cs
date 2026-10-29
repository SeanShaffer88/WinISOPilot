namespace ImageGuide;

public static class GuidanceState
{
    public static string Capability(int workspace,bool inspected,bool hasEdition,bool needsAttention)=>needsAttention?"Needs attention":workspace==0?"Available":workspace==9?inspected&&hasEdition?"Available":"Needs inspection":workspace==8?"Available":"Preview only";
    public static string Milestones(bool inspected,bool hasEdition)=>$"1 Choose ISO {(inspected?"✓":"•")}  →  2 Confirm edition {(hasEdition?"✓":"•")}  →  3 Optional edition preference  →  4 Review & create  →  5 Result";
}
public sealed partial class GuideForm
{
    readonly CheckBox guidedMode=new(){Text="Guided mode",AutoSize=true,Checked=true,UseMnemonic=false};
    readonly Button changesButton=Button("Your changes (0)"),guideContinue=Button("Continue guide");
    readonly TableLayoutPanel guideBar=new(){Dock=DockStyle.Top,AutoSize=true,ColumnCount=1,Margin=Padding.Empty,Padding=new Padding(2,2,8,5)};
    readonly Label guideMilestones=new(){AutoSize=true,Dock=DockStyle.Fill,UseMnemonic=false},workspaceCapability=new(){AutoSize=true,Dock=DockStyle.Fill,UseMnemonic=false};
    readonly Button anotherCopy=Button("Start another copy");
    TableLayoutPanel? guidanceRoot;Control? advancedNavigation;int guideStage;MediaBuildResult? lastBuildResult;
    void InitializeGuidance(Panel viewport,FlowLayoutPanel actions,Control navigation,TableLayoutPanel shell)
    {
        guidanceRoot=shell;advancedNavigation=navigation;guideBar.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));guideMilestones.MaximumSize=new Size(850,0);guideBar.Controls.Add(guideMilestones);guideBar.Controls.Add(workspaceCapability);var guideActions=Flow();guideActions.Controls.Add(guideContinue);guideActions.Controls.Add(anotherCopy);guideBar.Controls.Add(guideActions);viewport.Controls.Add(guideBar);viewport.Controls.SetChildIndex(guideBar,viewport.Controls.Count-1);
        Help(guidedMode,"Show the five-stage primary guide. Clear this to explore all ten advanced workspaces. Changing modes never executes work.");Help(guideContinue,"Open the next useful workspace and focus its control. This never starts inspection or a build.");Help(changesButton,"Review actual media choices and preview wishes. Removing one choice does not delete files or change Windows.");Help(anotherCopy,"Start a new guided journey using the current source. No files or choices are deleted.");anotherCopy.Click+=(_,_)=>{if(busy)return;lastBuildResult=null;guideStage=1;OpenWorkspace(Workspace.ImageIndexes);ScrollTo(editions);editions.Focus();};actions.Controls.Add(guidedMode);actions.Controls.Add(changesButton);
        guidedMode.CheckedChanged+=(_,_)=>RefreshGuidance();guideContinue.Click+=(_,_)=>ContinueGuide();changesButton.Click+=(_,_)=>ShowChanges();
    }
    void RefreshGuidance()
    {
        guideBar.Visible=true;guideMilestones.Visible=guidedMode.Checked;guideContinue.Visible=guidedMode.Checked;if(advancedNavigation!=null)advancedNavigation.Visible=!guidedMode.Checked;if(guidanceRoot!=null)guidanceRoot.ColumnStyles[0].Width=guidedMode.Checked?0:190;guideMilestones.MaximumSize=new Size(guidedMode.Checked?850:650,0);guideMilestones.Text=GuidanceState.Milestones(result!=null,guideStage>=2&&editions.SelectedItem is Edition);guideMilestones.AccessibleName="Guide milestones";guideMilestones.AccessibleDescription=guideMilestones.Text;workspaceCapability.Text=Names[step]+" • "+GuidanceState.Capability(step,result!=null,editions.SelectedItem is Edition,Engine.RestartRequired)+(guidedMode.Checked&&result!=null&&editions.SelectedItem is Edition selected?"\r\nSelected edition: "+selected.Name+" — change it below, then confirm.":"");workspaceCapability.AccessibleName="Current workspace capability";anotherCopy.Visible=guidedMode.Checked&&lastBuildResult!=null;
        guideContinue.Text=lastBuildResult!=null?"View created ISO result…":result==null?"Choose and inspect ISO…":editions.SelectedItem is not Edition?"Choose Windows edition…":guideStage<2?"Confirm selected edition…":"Review and create a new ISO…";
        guideContinue.Enabled=!busy;anotherCopy.Enabled=!busy;changesButton.Text=$"Your changes ({ChangeRows().Count})";if(guidedMode.Checked){back.Enabled=!busy&&guideStage>1;next.Visible=true;next.Enabled=!busy;next.Text=result==null?"Choose ISO…":guideStage<2?"Confirm edition…":"Review copy…";}else next.Text="Next →";
        for(int i=0;i<steps.Count;i++){string state=GuidanceState.Capability(i,result!=null,editions.SelectedItem is Edition,Engine.RestartRequired);steps[i].AccessibleName=Names[i]+" — "+state;steps[i].AccessibleDescription="Workspace status: "+state+". Opening this workspace never executes changes.";}
    }
    void ContinueGuide()
    {
        if(busy)return;if(lastBuildResult!=null){ShowDetails("Created ISO result",System.Text.Json.JsonSerializer.Serialize(lastBuildResult,Engine.Json));return;}if(result==null){OpenWorkspace(Workspace.ImageIndexes);ScrollTo(source);if(source.Text.Length==0)browse.Focus();else inspect.Focus();}
        else if(editions.SelectedItem is not Edition){OpenWorkspace(Workspace.ImageIndexes);ScrollTo(editions);editions.Focus();}
        else{OpenWorkspace(Workspace.BuildExport);if(guideStage<2){guideStage=2;ScrollTo(preferEdition);preferEdition.Focus();}else{guideStage=3;ScrollTo(build);build.Focus();}RefreshGuidance();}
    }
    static void ScrollTo(Control control){for(Control? parent=control.Parent;parent!=null;parent=parent.Parent)if(parent is ScrollableControl scroll&&scroll.AutoScroll)scroll.ScrollControlIntoView(control);}
    sealed record ChangeRow(string Id,string Name,string Scope,Workspace Workspace,string Details);
    List<ChangeRow> ChangeRows()
    {
        var rows=new List<ChangeRow>();if(preferEdition.Checked)rows.Add(new("media:edition","Retail edition preference","Actual media action",Workspace.BuildExport,"Writes the chosen Home/Pro preference to the new copy only. No license or edition removal."));
        foreach(string key in Keys)if(goals.TryGetValue(key,out var box)&&box.Checked)rows.Add(new("goal:"+key,key+" goal","Preview only",Workspace.BuildExport,"This broad goal is not applied by Create ISO."));
        var recipe=CapturePreview();foreach(string id in recipe.Requested){var option=PreviewRecipe.Options.Single(x=>x.Id==id);Workspace workspace=option.Group switch{"Hardware"=>Workspace.HardwareOobe,"Apps"=>Workspace.AppsPackages,"Components"=>Workspace.Components,"Registry"=>Workspace.Registry,_=>Workspace.DriversUpdates};rows.Add(new("option:"+id,option.Name,"Preview only",workspace,option.Consequence));}
        foreach(var service in recipe.Services)rows.Add(new("service:"+service.Key,service.Key+": "+service.Value,"Preview only",Workspace.ServicesTasks,"Requested startup only. Actual state and dependencies remain unknown."));
        if(recipe.Region!=PreviewRecipe.Regions[0])rows.Add(new("region",recipe.Region,"Preview only",Workspace.Unattended,"No region or answer file is applied."));return rows;
    }
    void RemoveChange(string id)
    {
        if(busy)return;bool wasRestoring=restoring;restoring=true;try{if(id=="media:edition")preferEdition.Checked=false;else if(id.StartsWith("goal:",StringComparison.Ordinal)&&goals.TryGetValue(id[5..],out var goal))goal.Checked=false;else if(id.StartsWith("option:",StringComparison.Ordinal)&&previewChoices.TryGetValue(id[7..],out var choice))choice.Checked=false;else if(id.StartsWith("service:",StringComparison.Ordinal)&&previewServices.TryGetValue(id[8..],out var service))service.SelectedIndex=0;else if(id=="region")previewRegion.SelectedIndex=0;}finally{restoring=wasRestoring;}Changed();
    }
    void ShowChanges()
    {
        using var dialog=Dialog("Your changes — choices only",new Size(760,520));var root=new TableLayoutPanel{Dock=DockStyle.Fill,ColumnCount=1,RowCount=3,Padding=new Padding(12)};root.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));root.RowStyles.Add(new RowStyle(SizeType.AutoSize));root.RowStyles.Add(new RowStyle(SizeType.Percent,100));root.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        var context=TextLabel("Edition: "+(editions.SelectedItem is Edition e?e.Name:"not selected")+". Removing a choice never deletes files or changes Windows.","Edition is context only; use Image & Indexes to choose it.");context.MaximumSize=new Size(700,0);root.Controls.Add(context,0,0);var scroll=new Panel{Dock=DockStyle.Fill,AutoScroll=true};var list=Stack();scroll.Controls.Add(list);root.Controls.Add(scroll,0,1);
        void Render(){list.SuspendLayout();foreach(Control child in list.Controls.Cast<Control>().ToArray())child.Dispose();list.Controls.Clear();var rows=ChangeRows();if(rows.Count==0)list.Controls.Add(TextLabel("No requested changes. Your source is still protected.","No removal actions are available."));foreach(var row in rows){var card=Card(row.Name,row.Scope);card.Controls.Add(TextLabel(row.Details,row.Details));var buttons=Flow();var remove=Button("Remove choice");Help(remove,"Restore only this choice to Keep or Unchanged. Global Undo can restore it. No files are deleted.");remove.Enabled=!busy;remove.Click+=(_,_)=>{if(busy)return;RemoveChange(row.Id);Render();};var go=Button("Go to choice");Help(go,"Close this view and open the relevant workspace; no change executes.");go.Click+=(_,_)=>{dialog.Close();OpenWorkspace(row.Workspace);};buttons.Controls.Add(remove);buttons.Controls.Add(go);card.Controls.Add(buttons);list.Controls.Add(card);}list.ResumeLayout(true);ApplyTheme(dialog);}
        Render();var close=Button("Close changes");close.DialogResult=DialogResult.Cancel;Help(close,"Return without changing choices. This is the default action.");root.Controls.Add(close,0,2);dialog.Controls.Add(root);dialog.AcceptButton=close;dialog.CancelButton=close;dialog.ShowDialog(this);
    }
}
