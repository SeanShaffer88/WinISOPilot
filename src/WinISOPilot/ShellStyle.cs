using System.ComponentModel;
namespace ImageGuide;

public static class ShellGeometry
{
    public static readonly Size CanonicalClient=new(920,660);
    public const bool CanResize=false,CanMaximize=false;
    public static Size Fit(Size logical,int dpi,Size working,Size chrome)
    {
        if(dpi<48||working.Width<1||working.Height<1||logical.Width<1||logical.Height<1)throw new ArgumentOutOfRangeException(nameof(dpi));
        int margin=Math.Max(8,(int)Math.Round(12*dpi/96d));
        int width=Math.Max(1,working.Width-Math.Max(0,chrome.Width)-2*margin),height=Math.Max(1,working.Height-Math.Max(0,chrome.Height)-2*margin);
        return new(Math.Min(width,(int)Math.Round(logical.Width*dpi/96d)),Math.Min(height,(int)Math.Round(logical.Height*dpi/96d)));
    }
    public static string FullPathText(string value)=>value;
}
public sealed record ShellPalette(Color Surface,Color Field,Color Text,Color Muted,Color Button,Color Border,Color Accent,Color OnAccent,Color Disabled)
{
    public Color FieldText{get;init;}=Text;
    public (Color Background,Color Foreground) ButtonColors(bool selected,bool enabled)=>(selected&&enabled?Accent:Button,!enabled?Disabled:selected?OnAccent:Text);
    public static ShellPalette For(bool dark,bool highContrast=false)
    {
        if(highContrast)return new(SystemColors.Control,SystemColors.Window,SystemColors.ControlText,SystemColors.ControlText,SystemColors.Control,SystemColors.WindowText,SystemColors.Highlight,SystemColors.HighlightText,SystemColors.GrayText){FieldText=SystemColors.WindowText};
        return dark?new(Color.FromArgb(23,35,50),Color.FromArgb(15,23,35),Color.FromArgb(240,245,250),Color.FromArgb(192,204,218),Color.FromArgb(36,54,75),Color.FromArgb(93,116,140),Color.FromArgb(185,28,66),Color.White,Color.FromArgb(178,193,208)):
            new(Color.White,Color.White,Color.FromArgb(23,35,51),Color.FromArgb(70,91,112),Color.FromArgb(231,237,243),Color.FromArgb(120,136,152),Color.FromArgb(165,20,55),Color.White,Color.FromArgb(91,107,123));
    }
    public static double Contrast(Color a,Color b)
    {
        static double L(Color c){static double V(byte value){double v=value/255d;return v<=0.04045?v/12.92:Math.Pow((v+0.055)/1.055,2.4);}return .2126*V(c.R)+.7152*V(c.G)+.0722*V(c.B);}
        double x=L(a),y=L(b);return (Math.Max(x,y)+.05)/(Math.Min(x,y)+.05);
    }
}
public sealed class ShellButton:Button
{
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public ShellPalette Palette{get;set;}=ShellPalette.For(false);
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public bool Selected{get;set;}
    public ShellButton(){UseCompatibleTextRendering=false;AutoSize=true;AutoSizeMode=AutoSizeMode.GrowAndShrink;FlatStyle=FlatStyle.Flat;UseVisualStyleBackColor=false;Padding=new Padding(5,2,5,2);}
    protected override void OnTextChanged(EventArgs e){base.OnTextChanged(e);AccessibleName=Text;}
    protected override void OnPaint(PaintEventArgs e)
    {
        if(SystemInformation.HighContrast){base.OnPaint(e);return;}
        var p=Palette;var state=p.ButtonColors(Selected,Enabled);Color background=state.Background;using var fill=new SolidBrush(background);e.Graphics.FillRectangle(fill,ClientRectangle);
        using var border=new Pen(Selected?p.Accent:p.Border);e.Graphics.DrawRectangle(border,0,0,Math.Max(0,Width-1),Math.Max(0,Height-1));
        Color text=state.Foreground;
        TextRenderer.DrawText(e.Graphics,Text,Font,Rectangle.Inflate(ClientRectangle,-6,-3),text,TextFormatFlags.HorizontalCenter|TextFormatFlags.VerticalCenter|TextFormatFlags.WordBreak|TextFormatFlags.NoPrefix);
        if(Focused&&ShowFocusCues)ControlPaint.DrawFocusRectangle(e.Graphics,Rectangle.Inflate(ClientRectangle,-4,-4),text,background);
    }
}
public sealed class FamilyListBox:ListBox
{
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public ShellPalette Palette{get;set;}=ShellPalette.For(false);
    int lastWidth;
    public FamilyListBox(){DrawMode=DrawMode.OwnerDrawVariable;IntegralHeight=false;ScrollAlwaysVisible=true;}
    protected override void OnMeasureItem(MeasureItemEventArgs e)
    {
        if(e.Index<0||e.Index>=Items.Count)return;string text=Items[e.Index] is CapabilityFamily item?item.Name:Items[e.Index]?.ToString()??"";
        e.ItemHeight=Math.Max(Font.Height+12,TextRenderer.MeasureText(text,Font,new Size(Math.Max(40,ClientSize.Width-SystemInformation.VerticalScrollBarWidth-16),0),TextFormatFlags.WordBreak|TextFormatFlags.NoPrefix).Height+12);e.ItemWidth=ClientSize.Width;
    }
    protected override void OnDrawItem(DrawItemEventArgs e)
    {
        if(e.Index<0||e.Index>=Items.Count)return;var p=Palette;bool selected=(e.State&DrawItemState.Selected)!=0;Color bg=selected?p.Accent:p.Field,fg=selected?p.OnAccent:p.FieldText;
        using var brush=new SolidBrush(bg);e.Graphics.FillRectangle(brush,e.Bounds);string text=Items[e.Index] is CapabilityFamily item?item.Name:Items[e.Index]?.ToString()??"";
        TextRenderer.DrawText(e.Graphics,text,Font,Rectangle.Inflate(e.Bounds,-7,-5),fg,TextFormatFlags.WordBreak|TextFormatFlags.NoPrefix);e.DrawFocusRectangle();
    }
    protected override void OnSizeChanged(EventArgs e){base.OnSizeChanged(e);if(lastWidth!=ClientSize.Width){lastWidth=ClientSize.Width;RefreshItems();}}
}
