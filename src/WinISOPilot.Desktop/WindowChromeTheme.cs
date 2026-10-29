using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace WinISOPilot.Desktop;

// DWM keeps the native caption controls, hit testing and accessibility. Unsupported
// attributes are ignored by Windows; high contrast restores the system defaults.
internal static class WindowChromeTheme
{
    [DllImport("dwmapi.dll", ExactSpelling=true)]
    static extern int DwmSetWindowAttribute(IntPtr hwnd,int attribute,ref int value,int size);

    internal static void Apply(Window window,bool dark,bool highContrast)
    {
        var handle=new WindowInteropHelper(window).Handle;
        if(handle==IntPtr.Zero)return;
        void Set(int attribute,int value)=>_ = DwmSetWindowAttribute(handle,attribute,ref value,sizeof(int));
        Set(20,dark&&!highContrast?1:0); // DWMWA_USE_IMMERSIVE_DARK_MODE
        Set(35,highContrast?-1:dark?0x001C1818:0x00E9E5E9); // DWMWA_CAPTION_COLOR, COLORREF
        Set(36,highContrast?-1:dark?0x00F2F0F2:0x002E2117); // DWMWA_TEXT_COLOR
    }
}