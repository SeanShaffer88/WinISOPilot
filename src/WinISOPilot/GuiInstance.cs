using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;
using Microsoft.Win32.SafeHandles;

namespace ImageGuide;

/// <summary>One GUI for this Windows user/session. The activation channel carries no data.</summary>
public sealed class GuiInstance : IDisposable
{
    readonly SafeWaitHandle mutex;
    readonly EventWaitHandle activation;
    RegisteredWaitHandle? listener;
    bool disposed;
    internal bool WasAbandoned { get; }

    GuiInstance(SafeWaitHandle mutex, EventWaitHandle activation, bool abandoned)
    {
        this.mutex = mutex;
        this.activation = activation;
        WasAbandoned = abandoned;
    }

    public static GuiInstance? Acquire() => AcquireCore("");
    internal static GuiInstance? AcquireFixture(string suffix) => AcquireCore(".Fixture." + Guid.Parse(suffix).ToString("N"));

    static GuiInstance? AcquireCore(string suffix)
    {
        using var identity = WindowsIdentity.GetCurrent();
        string sid = identity.User?.Value ?? throw new InvalidOperationException("Windows user identity is unavailable.");
        string name = @"Local\WinISOPilot.GUI." + sid + suffix;
        // Medium mandatory label permits the same user's unelevated launcher to contact
        // an elevated primary. No other user receives object access and no payload is sent.
        string sddl = "D:P(A;;GA;;;SY)(A;;GA;;;" + sid + ")S:(ML;;NW;;;ME)";
        if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl, 1, out nint descriptor, out _))
            throw new Win32Exception(Marshal.GetLastWin32Error());
        SafeWaitHandle? mutex = null;
        EventWaitHandle? activation = null;
        bool acquired = false;
        try
        {
            var attributes = new SecurityAttributes { Length = Marshal.SizeOf<SecurityAttributes>(), Descriptor = descriptor };
            mutex = CreateMutexExW(ref attributes, name + ".Owner", 0, 0x001F0001);
            if (mutex.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
            var eventHandle = CreateEventExW(ref attributes, name + ".Activate", 0, 0x001F0003);
            if (eventHandle.IsInvalid) { eventHandle.Dispose(); throw new Win32Exception(Marshal.GetLastWin32Error()); }
            activation = new EventWaitHandle(false, EventResetMode.AutoReset);
            activation.SafeWaitHandle = eventHandle;
            uint result = WaitForSingleObject(mutex, 0);
            if (result == 0x102)
            {
                if (!activation.Set()) throw new Win32Exception(Marshal.GetLastWin32Error());
                return null;
            }
            if (result != 0 && result != 0x80) throw new Win32Exception(Marshal.GetLastWin32Error());
            acquired = true;
            var owner = new GuiInstance(mutex, activation, result == 0x80);
            mutex = null;
            activation = null;
            return owner;
        }
        finally
        {
            if (mutex != null && acquired) ReleaseMutex(mutex);
            activation?.Dispose();
            mutex?.Dispose();
            LocalFree(descriptor);
        }
    }

    public void Listen(Action activate)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (listener != null) throw new InvalidOperationException("Activation listener is already registered.");
        listener = ThreadPool.RegisterWaitForSingleObject(activation, (_, timedOut) =>
        {
            if (!timedOut) { try { activate(); } catch (Exception) { /* Shutdown races must not crash a worker thread. */ } }
        }, null, Timeout.Infinite, false);
    }

    public static void ActivateWindow(nint mainWindow)
    {
        if (mainWindow == 0 || !IsWindow(mainWindow)) return;
        if (IsIconic(mainWindow)) ShowWindowAsync(mainWindow, 9);
        nint target = GetLastActivePopup(mainWindow);
        if (target == 0 || !IsWindowVisible(target)) target = mainWindow;
        if (IsIconic(target)) ShowWindowAsync(target, 9);
        if (!SetForegroundWindow(target))
        {
            var flash = new FlashInfo { Size = (uint)Marshal.SizeOf<FlashInfo>(), Window = target, Flags = 3, Count = 3 };
            FlashWindowEx(ref flash);
        }
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        // Called by the STA owner after Application.Run returns. Never release a mutex
        // from a continuation or background thread.
        if (listener != null)
        {
            using var drained = new ManualResetEvent(false);
            if (listener.Unregister(drained)) drained.WaitOne();
        }
        activation.Dispose();
        if (!ReleaseMutex(mutex)) throw new Win32Exception(Marshal.GetLastWin32Error());
        mutex.Dispose();
    }

    [StructLayout(LayoutKind.Sequential)] struct SecurityAttributes { public int Length; public nint Descriptor; public int Inherit; }
    [StructLayout(LayoutKind.Sequential)] struct FlashInfo { public uint Size; public nint Window; public uint Flags; public uint Count; public uint Timeout; }
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)] static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(string sddl, uint revision, out nint descriptor, out uint size);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern SafeWaitHandle CreateMutexExW(ref SecurityAttributes attributes, string name, uint flags, uint access);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern SafeWaitHandle CreateEventExW(ref SecurityAttributes attributes, string name, uint flags, uint access);
    [DllImport("kernel32.dll", SetLastError = true)] static extern uint WaitForSingleObject(SafeWaitHandle handle, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)] static extern bool ReleaseMutex(SafeWaitHandle handle);
    [DllImport("kernel32.dll")] static extern nint LocalFree(nint memory);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] static extern bool IsWindow(nint window);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] static extern bool IsWindowVisible(nint window);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] static extern bool IsIconic(nint window);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] static extern bool ShowWindowAsync(nint window, int command);
    [DllImport("user32.dll")] static extern nint GetLastActivePopup(nint window);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] static extern bool SetForegroundWindow(nint window);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] static extern bool FlashWindowEx(ref FlashInfo info);
}
