using System.IO;
using System.ComponentModel;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace PigTree.Ipc;

public sealed class Win32BootstrapPipe : IDisposable
{
    private SafeFileHandle? _readHandle;
    private SafeFileHandle? _writeHandle;
    private bool _disposed;

    public SafeFileHandle ReadHandle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_disposed || _readHandle == null, this);
            return _readHandle;
        }
    }

    public static Win32BootstrapPipe Create()
    {
        var sa = new Win32Native.SECURITY_ATTRIBUTES
        {
            nLength = Marshal.SizeOf<Win32Native.SECURITY_ATTRIBUTES>(),
            lpSecurityDescriptor = IntPtr.Zero,
            bInheritHandle = true
        };

        if (!Win32Native.CreatePipe(out var hRead, out var hWrite, ref sa, 4096))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "CreatePipe failed");
        }

        // Only the read handle must be inheritable; clear inherit flag on write handle
        if (!Win32Native.SetHandleInformation(hWrite, Win32Native.HANDLE_FLAG_INHERIT, 0))
        {
            int err = Marshal.GetLastWin32Error();
            hRead.Dispose();
            hWrite.Dispose();
            throw new Win32Exception(err, "SetHandleInformation on bootstrap write pipe failed");
        }

        return new Win32BootstrapPipe { _readHandle = hRead, _writeHandle = hWrite };
    }

    public void WriteNonce(ReadOnlySpan<byte> nonce)
    {
        ObjectDisposedException.ThrowIf(_disposed || _writeHandle == null, this);

        using var fs = new FileStream(_writeHandle, FileAccess.Write, 4096, false);
        fs.Write(nonce);
        fs.Flush();

        // Close write handle so child gets EOF after reading
        _writeHandle.Dispose();
        _writeHandle = null;
    }

    public void Dispose()
    {
        if (!_disposed)
        {
            _writeHandle?.Dispose();
            _writeHandle = null;
            _readHandle?.Dispose();
            _readHandle = null;
            _disposed = true;
        }
    }
}