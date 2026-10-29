namespace WinISOPilot.Desktop;
internal static class Program
{
    [STAThread] static int Main(string[] args)
    {
        if(args.SequenceEqual(new[]{"--self-test"})){try{DesktopSelfTests.Run();return 0;}catch(Exception ex){System.Console.Error.WriteLine(ex);return 1;}}
        if(args.Length!=0)return 2;
        try
        {
            using var instance=ImageGuide.GuiInstance.Acquire();
            if(instance==null)return 0;
            var app=new System.Windows.Application{ShutdownMode=System.Windows.ShutdownMode.OnMainWindowClose};
            var window=new PilotWindow();
            window.SourceInitialized+=(_,_)=>instance.Listen(()=>
            {
                if(!window.Dispatcher.HasShutdownStarted)window.Dispatcher.BeginInvoke(()=>
                    ImageGuide.GuiInstance.ActivateWindow(new System.Windows.Interop.WindowInteropHelper(window).Handle));
            });
            app.Run(window);return 0;
        }
        catch(Exception ex){System.Windows.MessageBox.Show("WinISOPilot could not acquire or contact its single-instance guard. No second window was opened.\n\n"+ex.Message,"WinISOPilot",System.Windows.MessageBoxButton.OK,System.Windows.MessageBoxImage.Warning);return 1;}
    }
}
