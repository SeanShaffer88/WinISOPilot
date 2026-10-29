namespace ImageGuide;

public sealed partial class GuideForm
{
    enum Workspace{ImageIndexes,HardwareOobe,AppsPackages,Components,Registry,ServicesTasks,DriversUpdates,Unattended,PresetsCompare,BuildExport}
    readonly Dictionary<string,CheckBox> previewChoices=new(StringComparer.Ordinal);
    readonly Dictionary<string,ComboBox> previewServices=new(StringComparer.Ordinal);
    readonly List<Control> recipeMutations=new();
    readonly Label measuredStrip=new(){AutoSize=false,Dock=DockStyle.Fill,UseMnemonic=false};
    readonly TextBox recipeDiff=ReadBox(160),recipeSummary=ReadBox(140);
    PreviewRecipe comparison=PreviewRecipe.Empty;
    void OpenWorkspace(Workspace value)=>Go((int)value);
    TableLayoutPanel Card(string title,string statusText)
    {
        var card=Stack();card.Padding=new Padding(12);card.Margin=new Padding(0,7,0,7);card.Tag="card";card.BorderStyle=BorderStyle.FixedSingle;card.CellBorderStyle=TableLayoutPanelCellBorderStyle.None;
        var badge=TextLabel(statusText,"Capability status: "+statusText);badge.Font=stepBold;card.Controls.Add(badge);var heading=TextLabel(title,title);heading.Font=stepBold;card.Controls.Add(heading);return card;
    }
    TableLayoutPanel ImageWorkspace()
    {
        var p=Page("1. Image Mount & Index Manager","Choose an ISO. Inspection is read-only; writable WIM mounting is not implemented.","Inspect the ISO, then select an observed Windows edition below.");
        var sourceCard=Card("Source image","AVAILABLE • read-only inspection");var original=SourcePage();while(original.Controls.Count>3){var control=original.Controls[3];original.Controls.Remove(control);sourceCard.Controls.Add(control);}original.Dispose();p.Controls.Add(sourceCard);
        var editionCard=Card("Image indexes","OBSERVED only after inspection");var edition=EditionPage();while(edition.Controls.Count>3){var control=edition.Controls[3];edition.Controls.Remove(control);editionCard.Controls.Add(control);}edition.Dispose();p.Controls.Add(editionCard);
        var recoveryCard=Card("Mount readiness and recovery","UNAVAILABLE • no mount worker");Field(recoveryCard,"Readiness and observations",recovery,"No writable WIM mount, force unmount or foreign mount cleanup is available.");p.Controls.Add(recoveryCard);return p;
    }
    TableLayoutPanel PreviewCards(string title,string group)
    {
        var p=Page(title,"These choices are preview wishes. Installed items and removal safety are unknown.","Keep unchanged is the default. Preview selections never run or become build instructions.");
        var search=new TextBox{Dock=DockStyle.Top,PlaceholderText="Filter preview choices…"};Help(search,"Filter this preview list by name or consequence. Does not inspect the image.");search.AccessibleName="Filter preview choices";p.Controls.Add(search);
        var cards=new List<TableLayoutPanel>();var grid=new TableLayoutPanel{Dock=DockStyle.Top,AutoSize=true,AutoSizeMode=AutoSizeMode.GrowAndShrink,ColumnCount=1};grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100));p.Controls.Add(grid);
        foreach(var option in PreviewRecipe.Options.Where(x=>x.Group==group)){
            var card=Card(option.Name,"PREVIEW • not implemented");var toggle=new CheckBox{Text="Request in preview",AutoSize=true,UseMnemonic=false};previewChoices.Add(option.Id,toggle);recipeMutations.Add(toggle);Help(toggle,option.Consequence+" No operation executes.");toggle.AccessibleName=option.Name+" request in preview";toggle.CheckedChanged+=(_,_)=>Changed();card.Controls.Add(toggle);card.Controls.Add(TextLabel(option.Consequence,option.Consequence));cards.Add(card);grid.Controls.Add(card);
        }
        var visibleCards=cards.ToHashSet();var matches=TextLabel($"{cards.Count} preview choices","Matching preview choice count. No match does not prove absence from an image.");matches.AccessibleName="Matching preview choices";p.Controls.Add(matches);p.Controls.SetChildIndex(matches,4);
        bool arranging=false;void Arrange(){if(arranging)return;arranging=true;try{int columns=group is "Hardware" or "Components"&&grid.ClientSize.Width>=620?2:1;grid.SuspendLayout();grid.ColumnCount=columns;grid.ColumnStyles.Clear();for(int c=0;c<columns;c++)grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,100f/columns));grid.RowStyles.Clear();grid.RowCount=cards.Count;for(int row=0;row<cards.Count;row++)grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));int active=0,hidden=visibleCards.Count;foreach(var card in cards){bool shown=visibleCards.Contains(card);int cell=shown?active++:hidden++;card.Visible=shown;card.Dock=DockStyle.Fill;card.Margin=new Padding(0,6,cell%columns==columns-1?0:8,6);grid.SetCellPosition(card,new TableLayoutPanelCellPosition(cell%columns,cell/columns));}grid.ResumeLayout(true);}finally{arranging=false;}}grid.SizeChanged+=(_,_)=>Arrange();Arrange();
        search.TextChanged+=(_,_)=>{visibleCards=cards.Where(card=>string.Join(" ",card.Controls.Cast<Control>().Select(x=>x.Text)).Contains(search.Text,StringComparison.OrdinalIgnoreCase)).ToHashSet();matches.Text=visibleCards.Count==0?"No preview choices match. Clear or broaden the filter.":$"{visibleCards.Count} of {cards.Count} preview choices";Arrange();};
        return p;
    }
    TableLayoutPanel ServiceWorkspace()
    {
        var p=Page("6. Services & Scheduled Tasks","No offline service or task inventory has run.","Choose startup wishes for the preview only. No task file or service is changed.");
        var servicesCard=Card("Service startup wishes","PREVIEW • actual states and dependencies unknown");var rows=new TableLayoutPanel{Dock=DockStyle.Top,AutoSize=true,ColumnCount=2};rows.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,60));rows.ColumnStyles.Add(new ColumnStyle(SizeType.Percent,40));servicesCard.Controls.Add(rows);
        string[] friendly={"Diagnostic tracking","Windows error reporting","Downloaded maps","Remote registry access","Biometric sign-in","Printing","Memory prefetch"};string[] consequences={"Diagnostic collection behavior varies by policy and build.","Disabling error reports can reduce troubleshooting information.","Offline maps may stop updating.","Remote registry tools may stop working.","Windows Hello biometric sign-in could stop working.","Printing depends on Print Spooler.","Startup and memory behavior may change; performance is unmeasured."};
        for(int i=0;i<PreviewRecipe.ServiceNames.Length;i++){string service=PreviewRecipe.ServiceNames[i];var label=TextLabel(friendly[i]+" — "+service,consequences[i]);rows.Controls.Add(label,0,i);var choice=new ComboBox{DropDownStyle=ComboBoxStyle.DropDownList,Dock=DockStyle.Top,Margin=new Padding(4,5,0,7)};choice.Items.AddRange(PreviewRecipe.StartupStates);choice.SelectedIndex=0;previewServices.Add(service,choice);recipeMutations.Add(choice);choice.AccessibleName=service+" requested startup";Help(choice,consequences[i]+" Unchanged is the default; actual startup is unknown. No service changes execute.");choice.AccessibleName=friendly[i]+" requested startup (preview)";rows.Controls.Add(choice,1,i);choice.SelectedIndexChanged+=(_,_)=>Changed();}p.Controls.Add(servicesCard);
        var tasks=Card("Scheduled tasks","NOT IMPLEMENTED");tasks.Controls.Add(TextLabel("Task enumeration, dependency checks and offline policy changes remain future work. No task is disabled.","No task XML is read or altered."));p.Controls.Add(tasks);return p;
    }
    TableLayoutPanel AppWorkspace()
    {
        var p=PreviewCards("3. Appx & Modern Packages","Apps");var references=Button("Read package and feature references…");Help(references,"Open reference information, not installed package inventory.");references.Click+=(_,_)=>{using var d=Dialog("Package reference families",new Size(820,560));d.Controls.Add(Catalog(new[]{"Apps","Features"}));d.ShowDialog(this);};p.Controls.Add(references);return p;
    }
    TableLayoutPanel IntegrationWorkspace()
    {
        var p=PreviewCards("7. Driver & Update Slipstreaming","Integration");var info=Card("Package queue","NOT IMPLEMENTED • no payloads staged");info.Controls.Add(TextLabel("No driver or update file has been selected. Signature checks, target scopes and dependency ordering are unavailable. Preview requests cannot be injected.","No fictional package rows, sizes or signature results are shown."));p.Controls.Add(info);return p;
    }
    TableLayoutPanel PresetWorkspace()
    {
        var p=Page("9. Preset Profiles & Diff Engine","Preview recipes contain only known choices. They exclude source paths, credentials and commands.","Import or export JSON preview wishes; compare them with unchanged defaults. No preset executes.");
        var card=Card("Preview recipe • JSON","AVAILABLE • nonexecuting data only");Field(card,"Preview recipe summary",recipeSummary,"This recipe is never passed to the ISO engine.");var actions=Flow();var save=Button("Export preview JSON…");var load=Button("Import preview JSON…");var reset=Button("Reset preview choices");Help(save,"Save known preview choices to a new JSON file; existing files cannot be overwritten.");Help(load,"Validate a small strict-schema JSON recipe, then replace preview choices atomically. Never executes changes.");Help(reset,"Restore unchanged preview defaults. Undo can restore the preceding preview.");actions.Controls.Add(save);actions.Controls.Add(load);actions.Controls.Add(reset);card.Controls.Add(actions);recipeMutations.AddRange(new Control[]{save,load,reset});
        save.Click+=(_,_)=>SavePreview();load.Click+=(_,_)=>LoadPreview();reset.Click+=(_,_)=>{if(busy)return;ApplyPreview(PreviewRecipe.Empty);Changed();};p.Controls.Add(card);
        var diff=Card("Comparison • defaults or last-import baseline","PREVIEW DIFF • no footprint estimate");Field(diff,"Recipe comparison",recipeDiff,"Exact requested-choice differences only. No stock-image inventory or size savings are inferred.");p.Controls.Add(diff);return p;
    }
    PreviewRecipe CapturePreview()=>PreviewRecipe.Validate(new(1,false,previewChoices.Where(x=>x.Value.Checked).Select(x=>x.Key).ToArray(),previewServices.Where(x=>x.Value.SelectedIndex>0).ToDictionary(x=>x.Key,x=>x.Value.SelectedItem?.ToString()??"Unchanged",StringComparer.Ordinal),previewRegion.SelectedItem?.ToString()??"Keep source region"));
    void ApplyPreview(PreviewRecipe recipe)
    {
        recipe=PreviewRecipe.Validate(recipe);bool old=restoring;restoring=true;try{foreach(var pair in previewChoices)pair.Value.Checked=recipe.Requested.Contains(pair.Key);foreach(var pair in previewServices)pair.Value.SelectedItem=recipe.Services.GetValueOrDefault(pair.Key,"Unchanged");previewRegion.SelectedItem=recipe.Region;}finally{restoring=old;}
    }
    void RefreshReferenceState()
    {
        foreach(var control in recipeMutations)control.Enabled=!busy;
        var recipe=CapturePreview();recipeSummary.Text=$"PREVIEW ONLY • {recipe.Requested.Length} requested options / {recipe.Services.Count} service wishes\r\nRegion: {recipe.Region}\r\nExecutable: false. All requested image operations remain unimplemented.\r\nSize savings: unknown; no footprint measurement has run.";recipeDiff.Text=PreviewRecipe.Diff(comparison,recipe);
        string iso=result==null?"Source: not inspected":"Source: inspected";string edition=editions.SelectedItem is Edition e?"Edition index: "+e.Index+" / "+e.Architecture:"Edition: unknown";measuredStrip.Text=$"{iso}  •  {edition}\r\nActivity: {(busy?"Working":"Idle")}  •  Reclaimed: not measured  •  Preview: {recipe.Requested.Length+recipe.Services.Count+(recipe.Region==PreviewRecipe.Regions[0]?0:1)} requests";measuredStrip.AccessibleName="Observed source and preview status";measuredStrip.AccessibleDescription=measuredStrip.Text+(result==null?"":"\r\nFull source: "+result.Source)+(lastProgress==null?"":"\r\nObserved phase: "+lastProgress.Phase);
    }
    void SavePreview()
    {
        if(busy)return;using var d=new SaveFileDialog{Title="Export a NEW nonexecuting preview recipe",Filter="JSON preview (*.json)|*.json",AddExtension=true,OverwritePrompt=true};if(d.ShowDialog(this)!=DialogResult.OK)return;
        try{Engine.CheckPath(d.FileName);using var file=new FileStream(d.FileName,FileMode.CreateNew,FileAccess.Write,FileShare.None);using var writer=new StreamWriter(file);writer.Write(PreviewRecipe.Serialize(CapturePreview()));SetStatus("Nonexecuting preview recipe exported. No image changes were made.");}catch(Exception ex){SetStatus("Preview export failed: "+ex.Message);}
    }
    void LoadPreview()
    {
        if(busy)return;using var d=new OpenFileDialog{Title="Import nonexecuting preview JSON",Filter="JSON preview (*.json)|*.json",CheckFileExists=true};if(d.ShowDialog(this)!=DialogResult.OK)return;
        try{Engine.CheckPath(d.FileName);using var file=new FileStream(d.FileName,FileMode.Open,FileAccess.Read,FileShare.Read);if(file.Length>65536)throw new InvalidDataException("Preview recipes are limited to 64 KiB.");using var reader=new StreamReader(file);var parsed=PreviewRecipe.Parse(reader.ReadToEnd());comparison=CapturePreview();ApplyPreview(parsed);Changed();SetStatus("Preview recipe imported. No image commands were generated or executed.");}catch(Exception ex){SetStatus("Preview import rejected; choices unchanged: "+ex.Message);}
    }
}
