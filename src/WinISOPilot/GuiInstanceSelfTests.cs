using System.Diagnostics;

namespace ImageGuide;

internal static class GuiInstanceSelfTests
{
    internal static int Fixture(string[] args)
    {
        using var owner = GuiInstance.AcquireFixture(args[1]);
        if (owner == null) { Console.WriteLine("duplicate"); return 0; }
        Console.WriteLine("owner"); Console.Out.Flush();
        if (args[2] == "abandon") Environment.Exit(0);
        Thread.Sleep(250); // A duplicate may signal before the UI/listener exists.
        owner.Listen(() => { Console.WriteLine("activation"); Console.Out.Flush(); });
        Thread.Sleep(int.Parse(args[2], System.Globalization.CultureInfo.InvariantCulture));
        return 0;
    }

    static Process Start(string id, string mode)
    {
        string exe = Environment.ProcessPath ?? throw new IOException("Process path missing.");
        var start = new ProcessStartInfo(exe) { UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
        foreach (string arg in new[] { "--gui-instance-fixture", id, mode }) start.ArgumentList.Add(arg);
        return Process.Start(start) ?? throw new IOException("Fixture did not start.");
    }

    static string Finish(Process process)
    {
        var output = process.StandardOutput.ReadToEndAsync();
        var error = process.StandardError.ReadToEndAsync();
        if (!process.WaitForExit(15000)) throw new Exception("Instance fixture timed out; process retained.");
        if (process.ExitCode != 0) throw new Exception("Instance fixture failed: " + error.GetAwaiter().GetResult());
        return output.GetAwaiter().GetResult();
    }

    public static void Run()
    {
        string id = Guid.NewGuid().ToString("N");
        using (var first = Start(id, "1500"))
        {
            if (first.StandardOutput.ReadLine() != "owner") throw new Exception("Primary did not acquire ownership.");
            using var duplicate = Start(id, "100");
            if (!Finish(duplicate).Contains("duplicate", StringComparison.Ordinal)) throw new Exception("Duplicate acquired ownership.");
            if (!Finish(first).Contains("activation", StringComparison.Ordinal)) throw new Exception("Duplicate failed to activate primary.");
        }
        Console.WriteLine("PASS duplicate process exits and signals owner");
        using (var next = Start(id, "50"))
            if (!Finish(next).Contains("owner", StringComparison.Ordinal)) throw new Exception("Orderly exit did not release ownership.");
        Console.WriteLine("PASS orderly GUI ownership release permits relaunch");
        using (var abandoned = Start(id, "abandon"))
            if (!Finish(abandoned).Contains("owner", StringComparison.Ordinal)) throw new Exception("Abandon fixture did not own guard.");
        using (var recovered = Start(id, "50"))
            if (!Finish(recovered).Contains("owner", StringComparison.Ordinal)) throw new Exception("Abandoned guard did not recover.");
        Console.WriteLine("PASS terminated owner permits safe relaunch");
        string abandonedId = Guid.NewGuid().ToString("N");
        using (var identity = System.Security.Principal.WindowsIdentity.GetCurrent())
        using (var survivingHandle = new Mutex(false, @"Local\WinISOPilot.GUI." + identity.User!.Value + ".Fixture." + abandonedId + ".Owner"))
        {
            var thread = new Thread(() => { using var mutex = new Mutex(false, @"Local\WinISOPilot.GUI." + identity.User!.Value + ".Fixture." + abandonedId + ".Owner"); if (!mutex.WaitOne(0)) throw new Exception("Cannot acquire abandon fixture."); });
            thread.Start(); thread.Join();
            using var recovered = GuiInstance.AcquireFixture(abandonedId);
            if (recovered == null || !recovered.WasAbandoned) throw new Exception("WAIT_ABANDONED was not recovered.");
        }
        Console.WriteLine("PASS abandoned mutex ownership is acquired explicitly");
        string race = Guid.NewGuid().ToString("N");
        using var a = Start(race, "1500"); using var b = Start(race, "1500");
        string both = Finish(a) + Finish(b);
        if (both.Split('\n').Count(x => x.Trim() == "owner") != 1 || both.Split('\n').Count(x => x.Trim() == "duplicate") != 1)
            throw new Exception("Concurrent launch did not produce exactly one owner.");
        Console.WriteLine("PASS simultaneous process launch has exactly one owner");
        Console.WriteLine("5 single-instance process/thread tests passed; cross-elevation activation requires live validation.");
    }
}
