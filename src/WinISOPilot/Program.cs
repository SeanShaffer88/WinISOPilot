using System.Text.Json;
namespace ImageGuide;
internal static class Program
{
    [STAThread]
    static int Main(string[] args)
    {
        if(args.Length==0){ApplicationConfiguration.Initialize();Application.Run(new GuideForm());return 0;}
        try{return Run(args).GetAwaiter().GetResult();}catch(Exception ex){Console.Error.WriteLine(ex.Message);return 1;}
    }
    static async Task<int> Run(string[] args)
    {
        if(args.SequenceEqual(new[]{"--controlled-query-fixture"})){Console.WriteLine("fixture-start");await Task.Delay(300);Console.Error.WriteLine("fixture-error-end");Console.WriteLine("fixture-end");return 0;}
        if(args.SequenceEqual(new[]{"--controlled-query-failure-fixture"})){await Task.Delay(100);Console.Error.WriteLine("controlled failure");return 17;}
        if(args.SequenceEqual(new[]{"--self-test"})){int code=SelfTests.Run();if(code!=0)return code;CoreControlSelfTests.Run();return 0;}
        if(args.Length==1&&args[0] is "--help" or "-h"){Console.WriteLine("WinISOPilot inspect --source <local.iso> --output <NEW directory>\nWinISOPilot plan --inspection <inspection.json> [--choices <choices.json>] --output <NEW directory>\nWinISOPilot --self-test\nNo arguments opens the read-only GUI. No servicing/build commands exist.");return 0;}
        if(args.Length<1||args[0] is not("inspect" or "plan"))throw new ArgumentException("Use --help for supported commands.");
        var options=new Dictionary<string,string>(StringComparer.Ordinal);
        for(int i=1;i<args.Length;i+=2){if(i+1>=args.Length||!args[i].StartsWith("--",StringComparison.Ordinal)||!options.TryAdd(args[i],args[i+1]))throw new ArgumentException("Malformed or duplicate option.");}
        var allowed=args[0]=="inspect"?new[]{"--source","--output"}:new[]{"--inspection","--choices","--output"};
        if(options.Keys.Any(k=>!allowed.Contains(k)))throw new ArgumentException("Unknown option.");
        string Need(string key)=>options.TryGetValue(key,out var value)?value:throw new ArgumentException("Missing "+key);
        if(args[0]=="inspect"){var result=await Engine.Inspect(Need("--source"),Need("--output"));Console.WriteLine(JsonSerializer.Serialize(result,Engine.Json));}
        else {var choices=options.TryGetValue("--choices",out var file)?JsonSerializer.Deserialize<Choices>(await File.ReadAllTextAsync(file),Engine.Json)??throw new InvalidDataException("Invalid choices."):new Choices();Console.WriteLine(await Engine.Plan(Need("--inspection"),choices,Need("--output")));}
        return 0;
    }
}
