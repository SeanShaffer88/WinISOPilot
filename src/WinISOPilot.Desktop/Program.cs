namespace WinISOPilot.Desktop;
internal static class Program
{
    [STAThread] static int Main(string[] args)
    {
        if(args.SequenceEqual(new[]{"--self-test"})){try{DesktopSelfTests.Run();return 0;}catch(Exception ex){System.Console.Error.WriteLine(ex);return 1;}}
        if(args.Length!=0)return 2;
        var app=new System.Windows.Application{ShutdownMode=System.Windows.ShutdownMode.OnMainWindowClose};app.Run(new PilotWindow());return 0;
    }
}
