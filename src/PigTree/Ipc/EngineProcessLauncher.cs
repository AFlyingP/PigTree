using System.IO;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace PigTree.Ipc;

public sealed class SpawnedEngineProcess : IDisposable
{
    private IntPtr _processHandle;
    private bool _disposed;

    public IntPtr ProcessHandle => _processHandle;
    public uint ProcessId { get; }

    public SpawnedEngineProcess(IntPtr processHandle, uint processId)
    {
        _processHandle = processHandle;
        ProcessId = processId;
    }

    public void Terminate(uint exitCode = 1)
    {
        if (_processHandle != IntPtr.Zero)
        {
            Win32Native.TerminateProcess(_processHandle, exitCode);
        }
    }

    public void Dispose()
    {
        if (!_disposed)
        {
            if (_processHandle != IntPtr.Zero)
            {
                Win32Native.CloseHandle(_processHandle);
                _processHandle = IntPtr.Zero;
            }
            _disposed = true;
        }
    }
}

public static class EngineProcessLauncher
{
    public static string FindEngineBinary(string? overridePath = null)
    {
        if (!string.IsNullOrWhiteSpace(overridePath))
        {
            if (File.Exists(overridePath))
            {
                return Path.GetFullPath(overridePath);
            }
            throw new FileNotFoundException($"Specified engine binary override path not found: '{overridePath}'", overridePath);
        }

        string baseDir = AppContext.BaseDirectory;

#if DEBUG
        const string preferredConfig = "debug";
        const string fallbackConfig = "release";
#else
        const string preferredConfig = "release";
        const string fallbackConfig = "debug";
#endif

        string[] candidates =
        {
            // 1. AppContext.BaseDirectory first
            Path.Combine(baseDir, "pigtree-engine.exe"),

            // 2. Relative cargo target dirs from AppContext.BaseDirectory (preferred config, then fallback)
            Path.Combine(baseDir, "..", "..", "..", "..", "..", "target", preferredConfig, "pigtree-engine.exe"),
            Path.Combine(baseDir, "..", "..", "..", "..", "..", "target", fallbackConfig, "pigtree-engine.exe"),
            Path.Combine(baseDir, "..", "target", preferredConfig, "pigtree-engine.exe"),
            Path.Combine(baseDir, "..", "target", fallbackConfig, "pigtree-engine.exe"),

            // 3. Current working directory cargo target dirs
            Path.Combine(Directory.GetCurrentDirectory(), "target", preferredConfig, "pigtree-engine.exe"),
            Path.Combine(Directory.GetCurrentDirectory(), "target", fallbackConfig, "pigtree-engine.exe"),
        };

        foreach (string candidate in candidates)
        {
            try
            {
                if (File.Exists(candidate))
                {
                    return Path.GetFullPath(candidate);
                }
            }
            catch
            {
                // Ignore invalid candidate paths
            }
        }

        throw new FileNotFoundException("Could not locate pigtree-engine.exe binary. Ensure it is built or provide an explicit path.");
    }

    public static SpawnedEngineProcess Spawn(
        string engineExePath,
        string pipeName,
        string sessionId,
        Win32BootstrapPipe bootstrapPipe,
        Win32JobObject jobObject)
    {
        ArgumentNullException.ThrowIfNull(engineExePath);
        ArgumentNullException.ThrowIfNull(pipeName);
        ArgumentNullException.ThrowIfNull(sessionId);
        ArgumentNullException.ThrowIfNull(bootstrapPipe);
        ArgumentNullException.ThrowIfNull(jobObject);

        IntPtr hRead = bootstrapPipe.ReadHandle.DangerousGetHandle();
        string commandLine = $"\"{engineExePath}\" --pipe-name \"{pipeName}\" --session-id \"{sessionId}\" --bootstrap-handle {hRead.ToInt64()}";

        // Initialize attribute list for handle list inheritance confinement
        IntPtr size = IntPtr.Zero;
        Win32Native.InitializeProcThreadAttributeList(IntPtr.Zero, 1, 0, ref size);

        IntPtr pAttrList = Marshal.AllocHGlobal(size);
        try
        {
            if (!Win32Native.InitializeProcThreadAttributeList(pAttrList, 1, 0, ref size))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "InitializeProcThreadAttributeList failed");
            }

            IntPtr pHandles = Marshal.AllocHGlobal(IntPtr.Size);
            try
            {
                Marshal.WriteIntPtr(pHandles, hRead);
                if (!Win32Native.UpdateProcThreadAttribute(
                    pAttrList,
                    0,
                    (IntPtr)Win32Native.PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                    pHandles,
                    (IntPtr)IntPtr.Size,
                    IntPtr.Zero,
                    IntPtr.Zero))
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "UpdateProcThreadAttribute failed");
                }

                var siex = new Win32Native.STARTUPINFOEX();
                siex.StartupInfo.cb = Marshal.SizeOf<Win32Native.STARTUPINFOEX>();
                siex.lpAttributeList = pAttrList;

                uint creationFlags = Win32Native.EXTENDED_STARTUPINFO_PRESENT |
                                     Win32Native.CREATE_NO_WINDOW |
                                     Win32Native.CREATE_SUSPENDED;

                if (!Win32Native.CreateProcessW(
                    null,
                    commandLine,
                    IntPtr.Zero,
                    IntPtr.Zero,
                    true, // Inherit only whitelisted handles
                    creationFlags,
                    IntPtr.Zero,
                    null,
                    ref siex,
                    out var pi))
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), $"CreateProcessW failed for '{engineExePath}'");
                }

                try
                {
                    // Confine in Job Object BEFORE resuming thread
                    jobObject.AssignProcess(pi.hProcess);

                    // Resume main thread
                    if (Win32Native.ResumeThread(pi.hThread) == Win32Native.INFINITE)
                    {
                        throw new Win32Exception(Marshal.GetLastWin32Error(), "ResumeThread failed");
                    }

                    return new SpawnedEngineProcess(pi.hProcess, pi.dwProcessId);
                }
                catch
                {
                    Win32Native.TerminateProcess(pi.hProcess, 1);
                    Win32Native.CloseHandle(pi.hProcess);
                    throw;
                }
                finally
                {
                    Win32Native.CloseHandle(pi.hThread);
                }
            }
            finally
            {
                Marshal.FreeHGlobal(pHandles);
            }
        }
        finally
        {
            Win32Native.DeleteProcThreadAttributeList(pAttrList);
            Marshal.FreeHGlobal(pAttrList);
        }
    }
}