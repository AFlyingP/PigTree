using System.ComponentModel;
using System.Runtime.InteropServices;

namespace PigTree.Ipc;

public sealed class Win32JobObject : IDisposable
{
    private IntPtr _jobHandle;
    private bool _disposed;

    public IntPtr Handle => _jobHandle;

    private Win32JobObject(IntPtr handle)
    {
        _jobHandle = handle;
    }

    public static Win32JobObject CreateKillOnClose()
    {
        IntPtr hJob = Win32Native.CreateJobObject(IntPtr.Zero, null);
        if (hJob == IntPtr.Zero)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateJobObject failed");
        }

        try
        {
            var info = new Win32Native.JOBOBJECT_EXTENDED_LIMIT_INFORMATION
            {
                BasicLimitInformation = new Win32Native.JOBOBJECT_BASIC_LIMIT_INFORMATION
                {
                    LimitFlags = Win32Native.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                }
            };

            int length = Marshal.SizeOf(typeof(Win32Native.JOBOBJECT_EXTENDED_LIMIT_INFORMATION));
            IntPtr infoPtr = Marshal.AllocHGlobal(length);
            try
            {
                Marshal.StructureToPtr(info, infoPtr, false);
                if (!Win32Native.SetInformationJobObject(
                    hJob,
                    Win32Native.JobObjectExtendedLimitInformation,
                    infoPtr,
                    (uint)length))
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "SetInformationJobObject failed");
                }
            }
            finally
            {
                Marshal.FreeHGlobal(infoPtr);
            }

            return new Win32JobObject(hJob);
        }
        catch
        {
            Win32Native.CloseHandle(hJob);
            throw;
        }
    }

    public void AssignProcess(IntPtr hProcess)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        if (!Win32Native.AssignProcessToJobObject(_jobHandle, hProcess))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "AssignProcessToJobObject failed");
        }
    }

    public void Dispose()
    {
        if (!_disposed)
        {
            if (_jobHandle != IntPtr.Zero)
            {
                Win32Native.CloseHandle(_jobHandle);
                _jobHandle = IntPtr.Zero;
            }
            _disposed = true;
        }
    }
}
