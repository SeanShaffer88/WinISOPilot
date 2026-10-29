using System.Text.Json;
namespace ImageGuide;
public static partial class Engine
{
    public static void ServicingSelfTests()
    {
        int count = 0;
        void Test(string name, Action action) { action(); count++; Console.WriteLine("PASS " + name); }
        void Reject(Action action) { try { action(); } catch (InvalidDataException) { return; } throw new Exception("Unsafe input accepted."); }
        const string app = "Clipchamp.Clipchamp_4.0.0.0_neutral_~_yxz26nhyzhsrt";
        const string other = "Microsoft.GetHelp_10.0.0.0_x64__8wekyb3d8bbwe";
        const string success = "\nThe operation completed successfully.\n";
        Test("servicing expanded-image space bound rejects unknown duplicate and DTD metadata",()=>{if(ReadExpandedBytes("<WIM><IMAGE INDEX='1'><TOTALBYTES>12345</TOTALBYTES></IMAGE></WIM>",1)!=12345)throw new Exception();Reject(()=>ReadExpandedBytes("<WIM><IMAGE INDEX='1'/></WIM>",1));Reject(()=>ReadExpandedBytes("<WIM><IMAGE INDEX='1'><TOTALBYTES>1</TOTALBYTES><TOTALBYTES>2</TOTALBYTES></IMAGE></WIM>",1));try{ReadExpandedBytes("<!DOCTYPE WIM [<!ENTITY x '1'>]><WIM><IMAGE INDEX='1'><TOTALBYTES>&x;</TOTALBYTES></IMAGE></WIM>",1);throw new Exception("DTD accepted");}catch(System.Xml.XmlException){}});
        Test("servicing inspection snapshots isolate asynchronous metadata authorization",()=>{var original=new Inspection(1,"source","hash","now","released","owned",[new(1,"Pro","x64","10.0.26100.1",26100,"Client",Languages:["en-US"])],[],"profile","coverage");var frozen=FreezeInspection(original);original.Editions[0].Languages!.Clear();original.Editions.Clear();if(frozen.Editions.Count!=1||frozen.Editions[0].Build!=26100||frozen.Editions[0].Languages!.Count!=1)throw new Exception("Inspection aliases caller state");});
        Test("servicing resource packages and unknown families cannot be removed", () =>
        {
            if (!DescribeApp(app).Removable || DescribeApp("Clipchamp.Clipchamp_4.0.0.0_neutral_en-us_yxz26nhyzhsrt").Removable || DescribeApp("Microsoft.WindowsStore_1.0.0.0_x64__8wekyb3d8bbwe").Removable) throw new Exception();
        });
        Test("servicing package parser requires complete unique valid records", () =>
        {
            var parsed = ParseProvisionedApps("DisplayName : Clipchamp\nPackageName : " + app + success);
            if (parsed.Length != 1 || !parsed[0].Removable) throw new Exception();
            Reject(() => ParseProvisionedApps("PackageName : " + app + success));
            Reject(() => ParseProvisionedApps("DisplayName : App\nPackageName : " + app));
            Reject(() => ParseProvisionedApps("DisplayName : A\nPackageName : " + app + "\nDisplayName : B\nPackageName : " + app + success));
            Reject(() => DescribeApp("/Online"));
            Reject(() => ParseProvisionedApps("DisplayName : A\nPackageName : "+app+"\nPackageName : \n"+success));
            Reject(() => ParseProvisionedApps("PackageName : bad\nDisplayName : A\nPackageName : "+app+success));
            if(DescribeApp("Clipchamp.Clipchamp_4.0.0.0_neutral_~_abcdefghijklm").Removable)throw new Exception("Publisher lookalike approved");
        });
        Test("servicing fixed argument allowlist rejects online resetbase package surgery and wildcards", () =>
        {
            foreach (string arg in new[] { "/Online", "/ResetBase", "/Remove-Package", "/Cleanup-Mountpoints", "/PackageName:*", "/Image:C:\\a\n/Online", "cmd.exe" }) Reject(() => ValidateDismArguments([arg]));
            ValidateDismArguments(["/English", "/Image:C:\\owned", "/Remove-ProvisionedAppxPackage", "/PackageName:" + app]);
        });
        Test("servicing profile does not approve newer Windows ISO", () =>
        {
            var result = CheckServicing(new(1, "Pro", "x64", "10.0.26300.9457", 26300, "Client"), [new("ADK DISM", "C:\\fixture", "10.0.26100.9457")]);
            if (result.Available || !result.Reasons.Any(x => x.Contains("26300"))) throw new Exception();
        });
        Test("servicing explicit inventory identity and protected app gates", () =>
        {
            var inventory = new ServicingInventory(Guid.NewGuid(), "SHA", 1, "version", "tool", "version", "hash", "wim", [DescribeApp(app), DescribeApp(other)]);
            ValidateServicingChoices(inventory, new(inventory.Id, [app], true));
            Reject(() => ValidateServicingChoices(inventory, new(Guid.NewGuid(), [app], false)));
            Reject(() => ValidateServicingChoices(inventory, new(inventory.Id, [app, app], false)));
            Reject(() => ValidateServicingChoices(inventory, new(inventory.Id, ["Microsoft.WindowsStore_1.0.0.0_x64__8wekyb3d8bbwe"], false)));
        });
        Test("servicing private authority rejects forged stale and caller-mutated inventories",()=>
        {
            string toolPath=Path.GetTempFileName();File.WriteAllText(toolPath,"tool identity fixture");
            var tool=new Tool("ADK DISM",toolPath,"fixture-version");
            var edition=new Edition(1,"Pro","x64","10.0.26100.1",26100,"Client");
            var inspection=new Inspection(1,"source","sourcehash","now","HandleReleased","owned",[edition],[],"profile","coverage");
            var inventory=new ServicingInventory(Guid.NewGuid(),inspection.SourceSha256,1,edition.Version,tool.Path,tool.Version,Hash(tool.Path),"wimhash",[DescribeApp(app)]);
            var choices=new ServicingChoices(inventory.Id,[app],false);
            RegisterServicingInventory(inventory);
            try
            {
                inventory.Apps[0]=DescribeApp(other); // Returned DTO never aliases private authorization.
                RequireInventory(inspection,edition,choices,tool);
                Reject(()=>RequireInventory(inspection,edition,choices with{RemovePackages=[other]},tool));
                Reject(()=>RequireInventory(inspection,edition,choices with{InventoryId=Guid.NewGuid()},tool));
                Reject(()=>RequireInventory(inspection with{SourceSha256="changed"},edition,choices,tool));
                Reject(()=>RequireInventory(inspection,edition with{Index=2},choices,tool));
                Reject(()=>RequireInventory(inspection,edition with{Version="10.0.26100.2"},choices,tool));
                Reject(()=>RequireInventory(inspection,edition,choices,tool with{Path=tool.Path+".other"}));
                Reject(()=>RequireInventory(inspection,edition,choices,tool with{Version="changed"}));
                File.WriteAllText(toolPath,"changed tool");Reject(()=>RequireInventory(inspection,edition,choices,tool));
            }
            finally{lock(inventoryGate)servicingInventories.Remove(inventory.Id);File.Delete(toolPath);}
        });
        Test("servicing mount parser binds full path index and status", () =>
        {
            var rows = ParseMountedImages("Mounted images:\nMount Dir : C:\\owned\\mount\nImage File : C:\\owned\\copy.wim\nImage Index : 6\nStatus : Ok\n" + success);
            if (rows.Length != 1 || rows[0].Index != 6 || rows[0].Wim != "C:\\owned\\copy.wim") throw new Exception();
            Reject(() => ParseMountedImages(success));
            Reject(() => ParseMountedImages("Mounted images:\nImage File : C:\\orphan.wim\n" + success));
            Reject(() => ParseMountedImages("Mounted images:\nMount Dir : C:\\a\nImage File : C:\\a.wim\nImage Index : 1\nStatus : Ok\nMount Dir malformed\n"+success));
        });
        Test("servicing global gate is released on its acquiring thread", () =>
        {
            Task.Run(() => { using var gate = new ServicingLease(); Task.Delay(2).GetAwaiter().GetResult(); }).GetAwaiter().GetResult();
            Task.Run(() => { using var gate = new ServicingLease(); }).GetAwaiter().GetResult();
        });
        Test("servicing actual session adapter completes exact mount and discard sequence", () => RunSessionFixture("success").GetAwaiter().GetResult());
        Test("servicing actual session cancellation waits and discards owned mount", () => RunSessionFixture("cancel").GetAwaiter().GetResult());
        Test("servicing native failure outranks pending cancellation", () => RunSessionFixture("failure-cancel").GetAwaiter().GetResult());
        Test("servicing pre-mutation query failure discards without committing", () => RunSessionFixture("query-failure").GetAwaiter().GetResult());
        Test("servicing postinventory verification retains every nonselected app",()=>{VerifyAppsAfter([DescribeApp(app),DescribeApp(other)],[DescribeApp(other)],[app]);try{VerifyAppsAfter([DescribeApp(app),DescribeApp(other)],[],[app]);throw new Exception("Nonselected app loss accepted");}catch(IOException){}});
        Test("servicing removal failure discards without publishing",()=>RunSessionFixture("remove-failure").GetAwaiter().GetResult());
        Test("servicing commit failure is retained as failure after discard",()=>RunSessionFixture("commit-failure").GetAwaiter().GetResult());
        Test("servicing pause acknowledges only after native completion",()=>RunSessionFixture("pause").GetAwaiter().GetResult());
        Test("servicing pending actions block mutation and commit",()=>RunSessionFixture("pending").GetAwaiter().GetResult());
        Test("servicing unproven healthy state discards instead of committing",()=>RunSessionFixture("health-failure").GetAwaiter().GetResult());
        Test("servicing native mount failure leaves recovery evidence",()=>RunSessionFixture("mount-failure").GetAwaiter().GetResult());
        Test("servicing discard failure latches recovery and retains pending journal",()=>RunSessionFixture("discard-failure").GetAwaiter().GetResult());
        Test("servicing mismatched mount association blocks operations", () => RunSessionFixture("association").GetAwaiter().GetResult());
        Console.WriteLine(count + " servicing tests passed.");
    }
    static async Task RunSessionFixture(string mode)
    {
        string root = Path.Combine(Path.GetTempPath(), "WinISOPilot-servicing-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root); string wim = Path.Combine(root, "copy.wim"), tool = Path.Combine(root, "test-tool.bin");
        File.WriteAllText(wim, "fixture WIM bytes; never mounted by real DISM"); File.WriteAllText(tool, "fixture runner; never executed");
        using var control = new JobControl(); bool mounted = false, discarded = false, committed = false, pauseAcknowledged = false; var commands = new List<string[]>();
        string journalRoot = Path.Combine(root, "recovery");
        ProcessOutput Result(string text, int code = 0) => new(new(text, false, text.Length), new("", false, 0), code, false);
        async Task<ProcessOutput> Runner(string[] args)
        {
            await Task.Yield(); commands.Add(args);
            string success = "\nThe operation completed successfully.\n";
            if (args.Contains("/Get-MountedWimInfo")) return Result("Mounted images:\n" + (mounted ? "Mount Dir : " + Path.Combine(root, "servicing-mount") + "\nImage File : " + (mode == "association" ? "C:\\foreign.wim" : wim) + "\nImage Index : 1\nStatus : Ok\n" : "") + success);
            if (args.Contains("/Mount-Image")) { if(mode=="mount-failure")return Result("mount failed",87);if(mode=="pending"){Directory.CreateDirectory(Path.Combine(root,"servicing-mount","Windows","WinSxS"));File.WriteAllText(Path.Combine(root,"servicing-mount","Windows","WinSxS","pending.xml"),"pending");}mounted = true; return Result(success); }
            if (args.Contains("/Get-ProvisionedAppxPackages")) return mode == "query-failure" ? Result("native query failed", 87) : Result("DisplayName : Clipchamp\nPackageName : Clipchamp.Clipchamp_4.0.0.0_neutral_~_yxz26nhyzhsrt\n" + success);
            if (args.Contains("/Remove-ProvisionedAppxPackage")) { if (mode is "cancel" or "failure-cancel" or "discard-failure") control.RequestCancel(); if(mode=="pause"){control.RequestPause();_ = Task.Run(async()=>{var deadline=DateTime.UtcNow.AddSeconds(5);while(!control.IsPaused&&DateTime.UtcNow<deadline)await Task.Delay(1);pauseAcknowledged=control.IsPaused;control.Resume();});} return Result(success, mode is "failure-cancel" or "remove-failure" ? 87 : 0); }
            if (args.Contains("/Unmount-Image")) { if(mode=="discard-failure"&&args.Contains("/Discard"))return Result("discard failed",87);if(mode=="commit-failure"&&args.Contains("/Commit"))return Result("commit failed",87);discarded = args.Contains("/Discard"); committed = args.Contains("/Commit"); mounted = false; return Result(success); }
            return Result(success);
        }
        var session = new OfflineSession(root, wim, 1, new("ADK DISM", tool, "fixture"), control, null,
            (name, text) => File.WriteAllText(Path.Combine(root, name), text), null, Runner, journalRoot);
        Exception? error = null;
        try { await session.Mount(false); session.CheckPendingActions(); await session.Apps(); await session.Remove("Clipchamp.Clipchamp_4.0.0.0_neutral_~_yxz26nhyzhsrt"); if(mode=="health-failure")await session.Health();await session.Finish(true); }
        catch (Exception ex) { error = ex; }
        finally { try { await session.Release(); } catch (Exception ex) { error = ex; } }
        if (mode == "success" && (error != null || !committed || mounted)) throw new Exception("Expected commit and verified release", error);
        if (mode == "cancel" && (error is not OperationCanceledException || !discarded || mounted || committed)) throw new Exception("Cancellation did not discard safely", error);
        if (mode is "failure-cancel" or "query-failure" or "remove-failure" or "commit-failure" && (error is not DismQueryException || !discarded || committed)) throw new Exception("Native failure lost priority", error);
        if(mode is "pending" or "health-failure" &&(error is not IOException||!discarded||committed))throw new Exception("Pending/unknown health committed",error);
        if(mode=="pause"&&(!pauseAcknowledged||error!=null||!committed))throw new Exception("Pause boundary failed",error);
        if(mode is "mount-failure" or "discard-failure" &&(error==null||!RestartRequired||!Directory.EnumerateFiles(journalRoot,"*.pending.json").Any()))throw new Exception("Uncertain teardown not latched",error);
        if (mode == "association" && (error == null || committed || !mounted)) throw new Exception("Foreign association adopted", error);
        if (commands.Any(args => args.Any(x => x == "/Online" || x == "/ResetBase"))) throw new Exception("Unsafe command");
        // Fixtures are retained outside the repository; no recursive deletion or production marker replay.
    }
}
