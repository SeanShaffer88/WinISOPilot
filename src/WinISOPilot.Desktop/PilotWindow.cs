using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Effects;
using System.Windows.Threading;
using ImageGuide;
using Microsoft.Win32;

namespace WinISOPilot.Desktop;
public sealed partial class PilotWindow:Window
{
    public static readonly string[] WorkspaceNames=["Image & Indexes","Hardware & OOBE","Apps & Packages","System Components","Registry Settings","Services & Tasks","Drivers & Updates","Unattended Setup","Presets & Compare","Build & Export"];
    readonly Session session=new();readonly Grid shell=new(),body=new();readonly StackPanel navigation=new();readonly UniformGrid stepper=new(){Rows=1,Columns=5};readonly ContentControl content=new();readonly TextBlock observed=new(),status=new(),title=new();readonly Dictionary<string,FrameworkElement> targets=new(StringComparer.Ordinal);readonly List<Button> navButtons=new();readonly Button back=new(),next=new(),pause=new(),cancel=new(),undo=new(),changes=new(),mode=new();readonly DispatcherTimer clockTimer=new(){Interval=TimeSpan.FromSeconds(1)},highlightTimer=new(){Interval=TimeSpan.FromSeconds(2)};readonly Stopwatch clock=new();ProgressRing? mainRing;
    bool advanced,dark=true,busy,critical;int workspace;JobControl? job;JobProgress? progress;string fullStatus="No job has run.",lastOutput="";long sourceBytes;DateTime sourceModified;TimeSpan elapsed;Window? jobWindow;TextBlock? liveJobText,mainJobText;TextBox? mainEvidence;FrameworkElement? highlighted;Border? highlightedCard;Thickness previousCardThickness;Effect? previousHighlightEffect;PreviewRecipe comparison=PreviewRecipe.Empty;
    public PilotWindow()
    {
        Title="WinISOPilot";Width=1280;Height=800;MinWidth=0;MinHeight=0;ResizeMode=ResizeMode.CanMinimize;WindowStartupLocation=WindowStartupLocation.CenterScreen;FontFamily=new FontFamily("Segoe UI");FontSize=15;
        Resources.MergedDictionaries.Add(new ResourceDictionary{Source=new Uri("/WinISOPilot.Desktop;component/Theme.xaml",UriKind.Relative)});SetResourceReference(BackgroundProperty,"PageBrush");SetResourceReference(ForegroundProperty,"TextBrush");
        shell.Margin=new Thickness(18,12,18,12);shell.RowDefinitions.Add(new(){Height=GridLength.Auto});shell.RowDefinitions.Add(new(){Height=GridLength.Auto});shell.RowDefinitions.Add(new(){Height=GridLength.Auto});shell.RowDefinitions.Add(new(){Height=new GridLength(1,GridUnitType.Star)});shell.RowDefinitions.Add(new(){Height=GridLength.Auto});Content=shell;
        var header=new DockPanel{LastChildFill=true,Margin=new Thickness(0,0,0,10)};var tools=new WrapPanel{MaxWidth=850,HorizontalAlignment=HorizontalAlignment.Right};SizeChanged+=(_,_)=>tools.MaxWidth=Math.Max(250,ActualWidth-320);tools.Children.Add(ActionButton("Search · Ctrl+K",()=>SearchDialog()));mode.Content="Advanced workspaces";mode.Click+=(_,_)=>{advanced=!advanced;Render();};tools.Children.Add(mode);tools.Children.Add(ActionButton("Light / dark",()=>{dark=!dark;ApplyPalette();}));changes.Click+=(_,_)=>ChangesDialog();tools.Children.Add(changes);tools.Children.Add(ActionButton("?",()=>HelpDialog()));DockPanel.SetDock(tools,Dock.Right);header.Children.Add(tools);var brand=Text("◈  WinISOPilot",26);brand.FontWeight=FontWeights.SemiBold;header.Children.Add(brand);shell.Children.Add(header);
        observed.FontSize=14;observed.Margin=new Thickness(2,0,0,12);observed.SetResourceReference(TextBlock.ForegroundProperty,"MutedBrush");Grid.SetRow(observed,1);shell.Children.Add(observed);stepper.Margin=new Thickness(0,0,0,14);Grid.SetRow(stepper,2);shell.Children.Add(stepper);
        body.ColumnDefinitions.Add(new(){Width=new GridLength(220)});body.ColumnDefinitions.Add(new(){Width=new GridLength(1,GridUnitType.Star)});var navScroll=Scroll(navigation);navScroll.Margin=new Thickness(0,0,16,0);body.Children.Add(navScroll);Grid.SetColumn(content,1);body.Children.Add(content);Grid.SetRow(body,3);shell.Children.Add(body);
        var footer=new StackPanel{Margin=new Thickness(0,12,0,0)};status.VerticalAlignment=VerticalAlignment.Center;status.FontSize=14;footer.Children.Add(status);var buttons=new WrapPanel{HorizontalAlignment=HorizontalAlignment.Right};buttons.Children.Add(ActionButton("Your job",()=>ShowJob()));undo.Content="Undo choice";undo.Click+=(_,_)=>{if(!busy){session.Undo();Render();}};buttons.Children.Add(undo);pause.Content="Pause";pause.Click+=(_,_)=>PauseJob();buttons.Children.Add(pause);cancel.Content="Cancel safely";cancel.Click+=(_,_)=>CancelJob();buttons.Children.Add(cancel);back.Content="Back";back.Click+=(_,_)=>NavigateBack();buttons.Children.Add(back);next.SetResourceReference(StyleProperty,"PrimaryButton");next.Click+=(_,_)=>ContinueGuide();buttons.Children.Add(next);footer.Children.Add(buttons);Grid.SetRow(footer,4);shell.Children.Add(footer);
        KeyDown+=(_,e)=>{if(e.Key==Key.F1){e.Handled=true;HelpDialog();}if(e.Key==Key.K&&(Keyboard.Modifiers&ModifierKeys.Control)!=0){e.Handled=true;SearchDialog();}};
        Loaded+=(_,_)=>{Width=Math.Min(1280,SystemParameters.WorkArea.Width-24);Height=Math.Min(800,SystemParameters.WorkArea.Height-24);Left=SystemParameters.WorkArea.Left+(SystemParameters.WorkArea.Width-Width)/2;Top=SystemParameters.WorkArea.Top+(SystemParameters.WorkArea.Height-Height)/2;};Closing+=OnClosing;Closed+=(_,_)=>{clockTimer.Stop();highlightTimer.Stop();jobWindow?.Close();job?.Dispose();SystemParameters.StaticPropertyChanged-=SystemPreferenceChanged;};SystemParameters.StaticPropertyChanged+=SystemPreferenceChanged;
        clockTimer.Tick+=(_,_)=>{if(!busy)CheckSource();UpdateChrome();};clockTimer.Start();highlightTimer.Tick+=(_,_)=>ClearHighlight();Render();ApplyPalette();
    }
    void SystemPreferenceChanged(object? sender,PropertyChangedEventArgs e){if(e.PropertyName==nameof(SystemParameters.HighContrast))Dispatcher.BeginInvoke(ApplyPalette);}
    internal Session State=>session;
    internal bool Busy=>busy;
    internal int WorkspaceIndex=>workspace;
    internal void MeasureForTests(double width,double height,bool advancedMode=false){advanced=advancedMode;Width=width;Height=height;Render();for(int i=0;i<5;i++){shell.Measure(new Size(width-36,height-62));shell.Arrange(new Rect(0,0,width-36,height-62));shell.UpdateLayout();}}
    internal FrameworkElement LayoutRoot=>shell;
    internal FrameworkElement ContentHost=>content;
    internal IReadOnlyList<Button> WorkspaceButtons=>navButtons;
    internal FrameworkElement Stepper=>stepper;
    internal FrameworkElement? FindTarget(string id)=>targets.GetValueOrDefault(id);
    static StackPanel Row()=>new(){Orientation=Orientation.Horizontal};
    TextBlock Text(string text,double size=15){var block=new TextBlock{Text=text,FontSize=size,TextWrapping=TextWrapping.Wrap};AutomationProperties.SetName(block,text);return block;}
    TextBlock Muted(string text){var block=Text(text);block.SetResourceReference(TextBlock.ForegroundProperty,"MutedBrush");return block;}
    TextBlock ListText(string text,double size=15){var block=Text(text,size);block.SetBinding(TextBlock.ForegroundProperty,new System.Windows.Data.Binding("Foreground"){RelativeSource=new System.Windows.Data.RelativeSource(System.Windows.Data.RelativeSourceMode.FindAncestor,typeof(ListBoxItem),1)});return block;}
    Button ActionButton(string text,Action action,bool primary=false,string? id=null){var button=new Button{Content=text};if(primary){button.SetResourceReference(StyleProperty,"PrimaryButton");if(!SystemParameters.HighContrast)button.Effect=new DropShadowEffect{Color=Color.FromRgb(54,217,244),BlurRadius=14,ShadowDepth=0,Opacity=.25};}button.Click+=(_,_)=>action();Register(button,id??text,text);return button;}
    void Register(FrameworkElement element,string id,string description){AutomationProperties.SetAutomationId(element,id);AutomationProperties.SetName(element,description);AutomationProperties.SetHelpText(element,description);targets[id]=element;}
    Border Card(string heading,UIElement child,string? badge=null)
    {
        var stack=new StackPanel();if(badge!=null){var state=Text(badge,13);state.SetResourceReference(TextBlock.ForegroundProperty,"AccentBrush");stack.Children.Add(state);}var label=Text(heading,20);label.FontWeight=FontWeights.SemiBold;stack.Children.Add(label);stack.Children.Add(child);var border=new Border{CornerRadius=new CornerRadius(15),Padding=new Thickness(18),Margin=new Thickness(0,0,0,14),BorderThickness=new Thickness(1),Child=stack};border.SetResourceReference(Border.BackgroundProperty,"CardBrush");border.SetResourceReference(Border.BorderBrushProperty,"LineBrush");if(!SystemParameters.HighContrast)border.Effect=new DropShadowEffect{Color=Colors.Black,Opacity=.18,BlurRadius=18,ShadowDepth=5};return border;
    }
    static ScrollViewer Scroll(UIElement child){var viewer=new ScrollViewer{Content=child,VerticalScrollBarVisibility=ScrollBarVisibility.Auto,HorizontalScrollBarVisibility=ScrollBarVisibility.Disabled,HorizontalContentAlignment=HorizontalAlignment.Stretch,PanningMode=PanningMode.VerticalOnly};if(child is FrameworkElement element)viewer.LayoutUpdated+=(_,_)=>{double available=Math.Max(0,viewer.ActualWidth-SystemParameters.VerticalScrollBarWidth-4);if(available>0&&Math.Abs(element.MaxWidth-available)>1)element.MaxWidth=available;};return viewer;}
    Grid Split(UIElement left,UIElement right,double ratio=1.5){var grid=new Grid();var l=new Border{Child=left};grid.Children.Add(l);grid.Children.Add(right);void Arrange(){bool wide=grid.ActualWidth>=740;grid.ColumnDefinitions.Clear();grid.RowDefinitions.Clear();if(wide){grid.ColumnDefinitions.Add(new(){Width=new GridLength(ratio,GridUnitType.Star)});grid.ColumnDefinitions.Add(new(){Width=new GridLength(1,GridUnitType.Star)});Grid.SetRow(right,0);Grid.SetColumn(right,1);l.Margin=new Thickness(0,0,16,0);}else{grid.RowDefinitions.Add(new(){Height=GridLength.Auto});grid.RowDefinitions.Add(new(){Height=GridLength.Auto});Grid.SetRow(right,1);Grid.SetColumn(right,0);l.Margin=new Thickness(0,0,0,12);}}bool? wasWide=null;grid.SizeChanged+=(_,_)=>{bool wide=grid.ActualWidth>=740;if(wasWide==wide)return;wasWide=wide;Arrange();};Arrange();return grid;}
    StackPanel Page(string heading,string description){var page=new StackPanel();title.Text=heading;page.Children.Add(Text(heading,27));page.Children.Add(Muted(description));return page;}
    void Mutate(Action action)
    {
        if(busy)return;string? focus=(Keyboard.FocusedElement as DependencyObject) is {} focused?AutomationProperties.GetAutomationId(focused):null;session.Change(action);RenderWorkspace();UpdateChrome();if(focus!=null&&targets.TryGetValue(focus,out var target))Dispatcher.BeginInvoke(()=>{target.BringIntoView();target.Focus();Keyboard.Focus(target);},DispatcherPriority.ContextIdle);
    }
    void Render(){RenderNavigation();RenderStepper();RenderWorkspace();UpdateChrome();}
    void RenderNavigation()
    {
        navigation.Children.Clear();navButtons.Clear();body.ColumnDefinitions[0].Width=advanced?new GridLength(220):new GridLength(0);navigation.Visibility=advanced?Visibility.Visible:Visibility.Collapsed;
        for(int i=0;i<WorkspaceNames.Length;i++){if(i is 0 or 1 or 8){var group=Muted(i==0?"SOURCE":i==1?"CUSTOMIZE · SOME PREVIEWS":"FINISH");group.FontSize=12;group.Margin=new Thickness(0,3,0,3);navigation.Children.Add(group);}int destination=i;var button=ActionButton(WorkspaceNames[i],()=>Navigate(destination),i==workspace);button.HorizontalContentAlignment=HorizontalAlignment.Left;button.Padding=new Thickness(8,2,8,2);button.MinHeight=0;button.Margin=new Thickness(0,0,0,2);var caption=new StackPanel();var name=Text(WorkspaceNames[i],14.67);name.Margin=new Thickness(0);name.SetResourceReference(TextBlock.ForegroundProperty,i==workspace?"OnAccentBrush":"TextBrush");caption.Children.Add(name);var state=Text(session.Capability(i,Engine.RestartRequired),12);state.Margin=new Thickness(0);state.SetResourceReference(TextBlock.ForegroundProperty,i==workspace?"OnAccentBrush":"MutedBrush");caption.Children.Add(state);button.Content=caption;navButtons.Add(button);navigation.Children.Add(button);}
    }
    void RenderStepper()
    {
        stepper.Children.Clear();string[] labels=["1  Source","2  Confirm edition","3  Preference","4  Review & create","5  Result"];
        for(int i=0;i<labels.Length;i++){int stage=i;var button=ActionButton(labels[i],()=>{if(busy)return;if(stage>=1&&session.Inspection==null){SetStatus("Inspect an ISO before choosing its edition.");return;}if(stage>=2&&!session.EditionConfirmed){SetStatus("Confirm the observed Windows edition first.");return;}if(stage==4&&session.Result==null){SetStatus("No verified ISO result exists for these choices.");return;}session.GuideStage=stage;Navigate(stage<2?0:9);},i==session.GuideStage);var label=Text(labels[i],15);label.SetResourceReference(TextBlock.ForegroundProperty,i==session.GuideStage?"OnAccentBrush":"TextBrush");button.Content=label;button.Padding=new Thickness(10,9,10,9);stepper.Children.Add(button);}
    }
    internal void Navigate(int destination){if(destination<0||destination>=WorkspaceNames.Length)return;workspace=destination;Render();}
    void RenderWorkspace()
    {
        ClearHighlight();targets.Clear();mainRing=null;mainJobText=null;mainEvidence=null;content.Content=Scroll(workspace switch{0=>SourcePage(),1=>HardwarePage(),2=>PackagesPage(),3=>ComponentsPage(),4=>OptionsPage("Offline registry settings","Registry"),5=>ServicesPage(),6=>OptionsPage("Drivers & updates","Integration"),7=>UnattendedPage(),8=>PresetsPage(),_=>BuildPage()});
    }
    void UpdateChrome()
    {
        observed.Text=$"Original ISO protected  •  {(session.Inspection==null?"Source: not inspected":"Edition: "+session.Edition?.Name+" / "+session.Edition?.Architecture)}  •  Total choices: {session.ChangeCount}  •  Savings: not measured";AutomationProperties.SetHelpText(observed,session.Source+"\n"+observed.Text);mode.Content=advanced?"Guided view":"Advanced workspaces";changes.Content=$"Your changes · {session.ChangeCount}";undo.IsEnabled=!busy&&session.UndoCount>0;pause.Content=job?.PauseRequested==true?"Resume":"Pause";pause.IsEnabled=CanControl;cancel.IsEnabled=CanControl;back.IsEnabled=!busy&&(advanced?workspace>0:session.GuideStage>0);next.IsEnabled=!busy;next.Content=advanced?"Continue guide →":session.GuideStage switch{0=>"Choose / inspect ISO →",1=>"Confirm edition →",2=>"Review choices →",3=>"Review new ISO →",_=>"View result →"};status.Text=busy?progress?.IsPaused==true?"Paused at a safe checkpoint":critical?"Finishing verification and cleanup…":progress?.Phase??"Working…":fullStatus.Length>95?"Status available · Your job":fullStatus;liveJobText?.SetCurrentValue(TextBlock.TextProperty,JobDescription());
        if(busy)elapsed=clock.Elapsed;if(jobPause!=null){jobPause.IsEnabled=CanControl;jobPause.Content=job?.PauseRequested==true?"Resume":"Pause";}if(jobCancel!=null)jobCancel.IsEnabled=CanControl;mainRing?.Observe(progress);if(mainJobText!=null)mainJobText.Text=JobDescription();if(mainEvidence!=null)mainEvidence.Text=fullStatus;
    }
    bool CanControl=>busy&&job!=null&&(!critical||job.IsPaused)&&!job.CancellationRequested;
    void ContinueGuide()
    {
        if(busy)return;if(session.Inspection==null){session.GuideStage=0;Navigate(0);FocusTarget(string.IsNullOrEmpty(session.Source)?"Choose ISO":"Inspect ISO");return;}
        if(!session.EditionConfirmed){session.GuideStage=1;Navigate(0);FocusTarget("Confirm edition");return;}
        if(session.GuideStage<3){session.GuideStage=3;Navigate(9);FocusTarget("Create new ISO");return;}if(session.Result!=null){ResultDialog();return;}Navigate(9);FocusTarget("Create new ISO");
    }
    void NavigateBack(){if(busy)return;if(advanced){Navigate(Math.Max(0,workspace-1));return;}session.GuideStage=Math.Max(0,session.GuideStage-1);Navigate(session.GuideStage<2?0:9);}
    void FocusTarget(string id){if(targets.TryGetValue(id,out var target)){target.BringIntoView();target.Focus();}}
    void SetStatus(string message){fullStatus=message;UpdateChrome();}
    void PauseJob(){if(!CanControl)return;if(job!.PauseRequested||job.IsPaused)job.Resume();else job.RequestPause();UpdateChrome();}
    void CancelJob(){if(!CanControl)return;job!.RequestCancel();SetStatus("Cancellation requested. Active work and final integrity checks finish first.");}
    void OnClosing(object? sender,CancelEventArgs e){if(!busy)return;e.Cancel=true;job?.RequestCancel();SetStatus("Safe cancellation requested. Wait for active work and cleanup before closing.");}
    void ApplyPalette()
    {
        bool high=SystemParameters.HighContrast;Brush Brush(string hex)=>new SolidColorBrush((Color)ColorConverter.ConvertFromString(hex));Resources["TextBrush"]=high?SystemColors.WindowTextBrush:Brush(dark?"#EDF6FF":"#14283D");Resources["MutedBrush"]=high?SystemColors.WindowTextBrush:Brush(dark?"#B3C6DA":"#40556C");Resources["LineBrush"]=high?SystemColors.WindowTextBrush:Brush(dark?"#52677E":"#7D94AB");Resources["FieldBrush"]=high?SystemColors.WindowBrush:Brush(dark?"#111C2B":"#F6F9FF");Resources["PageBrush"]=high?SystemColors.WindowBrush:new LinearGradientBrush((Color)ColorConverter.ConvertFromString(dark?"#1B2934":"#F5F9FF"),(Color)ColorConverter.ConvertFromString(dark?"#111B30":"#E8F1FF"),45);Resources["CardBrush"]=high?SystemColors.WindowBrush:new LinearGradientBrush((Color)ColorConverter.ConvertFromString(dark?"#2A3948":"#FFFFFF"),(Color)ColorConverter.ConvertFromString(dark?"#1C293D":"#EAF2FF"),45);Resources["AccentBrush"]=high?SystemColors.HighlightBrush:Brush(dark?"#36D9F4":"#00617D");Resources["OnAccentBrush"]=high?SystemColors.HighlightTextBrush:Brush("#071624");Resources["AccentGradient"]=high?SystemColors.HighlightBrush:new LinearGradientBrush(Color.FromRgb(54,217,244),Color.FromRgb(59,130,246),0);Resources["BlueBrush"]=high?SystemColors.HighlightBrush:Brush(dark?"#3B82F6":"#245CA8");Render();
    }
    void HighlightTarget(string id)
    {
        ClearHighlight();content.UpdateLayout();if(!targets.TryGetValue(id,out var target)){content.Focus();return;}target.BringIntoView();target.Focus();Keyboard.Focus(target);FocusManager.SetFocusedElement(this,target);highlighted=target;previousHighlightEffect=target.Effect;for(DependencyObject? current=target;current!=null;current=LogicalTreeHelper.GetParent(current))if(current is Border border&&border.CornerRadius.TopLeft==15){highlightedCard=border;previousCardThickness=border.BorderThickness;border.BorderThickness=new Thickness(2);border.SetResourceReference(Border.BorderBrushProperty,"AccentBrush");break;}if(!SystemParameters.HighContrast)target.Effect=new DropShadowEffect{Color=Color.FromRgb(54,217,244),BlurRadius=20,ShadowDepth=0,Opacity=.9};highlightTimer.Start();
    }
    void ClearHighlight(){highlightTimer.Stop();if(highlighted!=null){highlighted.Effect=previousHighlightEffect;highlighted=null;previousHighlightEffect=null;}if(highlightedCard!=null){highlightedCard.BorderThickness=previousCardThickness;highlightedCard.SetResourceReference(Border.BorderBrushProperty,"LineBrush");highlightedCard=null;}}
}
