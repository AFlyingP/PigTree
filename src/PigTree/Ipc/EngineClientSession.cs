using System.IO;
using System.IO.Pipes;
using Google.Protobuf;
using PigTree.Model;
using PigTree.Protocol;
using PigTree.Session.V1;

namespace PigTree.Ipc;

public sealed class EngineClientSession : IEngineSession
{
    private readonly Win32JobObject _jobObject;
    private readonly SpawnedEngineProcess _childProcess;
    private readonly NamedPipeClientStream _pipeStream;
    private readonly SemaphoreSlim _writeLock = new(1, 1);
    private readonly string _sessionId;
    private readonly byte[] _channelKey;
    private ulong _sequenceNumber;
    private bool _disposed;

    public bool IsConnected => _pipeStream.IsConnected && !_disposed;
    public string SessionId => _sessionId;
    public uint EnginePid => _childProcess.ProcessId;
    public byte[] ChannelKey => (byte[])_channelKey.Clone();

    private EngineClientSession(
        Win32JobObject jobObject,
        SpawnedEngineProcess childProcess,
        NamedPipeClientStream pipeStream,
        string sessionId,
        byte[] channelKey)
    {
        _jobObject = jobObject;
        _childProcess = childProcess;
        _pipeStream = pipeStream;
        _sessionId = sessionId;
        _channelKey = channelKey;
    }

    public static async Task<EngineClientSession> LaunchAsync(string? engineExePath = null, CancellationToken cancellationToken = default)
    {
        string engineBinary = EngineProcessLauncher.FindEngineBinary(engineExePath);

        var jobObject = Win32JobObject.CreateKillOnClose();
        byte[] bootstrapNonce = SecurityUtilities.GenerateNonce(32);
        using var bootstrapPipe = Win32BootstrapPipe.Create();
        bootstrapPipe.WriteNonce(bootstrapNonce);

        byte[] sessionNonce = SecurityUtilities.GenerateNonce(8);
        string sessionId = Convert.ToHexString(sessionNonce).ToLowerInvariant();
        string pipeServerName = $"pigtree-engine-{sessionId}";

        var childProcess = EngineProcessLauncher.Spawn(
            engineBinary,
            $@"\\.\pipe\{pipeServerName}",
            sessionId,
            bootstrapPipe,
            jobObject);

        NamedPipeClientStream? pipeStream = null;
        try
        {
            pipeStream = new NamedPipeClientStream(
                ".",
                pipeServerName,
                PipeDirection.InOut,
                PipeOptions.Asynchronous);

            await pipeStream.ConnectAsync(5000, cancellationToken).ConfigureAwait(false);

            // Mutual process identity verification:
            // 1. Verify Named Pipe server PID matches spawned child PID
            if (!Win32Native.GetNamedPipeServerProcessId(pipeStream.SafePipeHandle, out uint serverPid) || serverPid != childProcess.ProcessId)
            {
                throw new InvalidOperationException($"Identity mismatch: expected engine PID {childProcess.ProcessId}, got {serverPid}");
            }

            // Perform mutual authentication handshake using RAW AuthHandshakeRequest (not command envelope)
            byte[] clientNonce = SecurityUtilities.GenerateNonce(32);
            uint clientPid = (uint)Environment.ProcessId;

            var handshakeReq = new AuthHandshakeRequest
            {
                BootstrapNonce = ByteString.CopyFrom(bootstrapNonce),
                ClientNonce = ByteString.CopyFrom(clientNonce),
                ClientPid = clientPid,
                ClientSessionId = 0
            };

            var header = new FrameHeader
            {
                ChannelTag = ChannelTag.Command,
                Flags = FrameFlags.None,
                SequenceNumber = 1
            };
            await FrameCodec.WriteFrameAsync(pipeStream, header, handshakeReq.ToByteArray(), cancellationToken).ConfigureAwait(false);

            var respFrame = await FrameCodec.ReadFrameAsync(pipeStream, cancellationToken).ConfigureAwait(false);
            if (respFrame == null)
            {
                throw new EndOfStreamException("Server closed connection during handshake");
            }

            if (respFrame.Header.ChannelTag != ChannelTag.Command)
            {
                throw new InvalidDataException($"Handshake response received on invalid channel: {respFrame.Header.ChannelTag}");
            }

            // Handshake response is raw AuthHandshakeResponse (not CommandResponse envelope)
            var handshakeResp = AuthHandshakeResponse.Parser.ParseFrom(respFrame.Payload);

            if (handshakeResp.Status != 0)
            {
                throw new UnauthorizedAccessException($"Server rejected handshake: {handshakeResp.ErrorMessage}");
            }

            if (handshakeResp.ServerPid != childProcess.ProcessId)
            {
                throw new InvalidOperationException($"Server PID mismatch in handshake: expected {childProcess.ProcessId}, got {handshakeResp.ServerPid}");
            }

            byte[] expectedChannelKey = SecurityUtilities.DeriveChannelKey(
                bootstrapNonce,
                clientNonce,
                handshakeResp.ServerNonce.ToByteArray());

            if (!SecurityUtilities.ConstantTimeEquals(handshakeResp.ChannelKeyHash.ToByteArray(), expectedChannelKey))
            {
                throw new UnauthorizedAccessException("Derived channel key hash mismatch during handshake");
            }

            return new EngineClientSession(jobObject, childProcess, pipeStream, sessionId, expectedChannelKey);
        }
        catch
        {
            pipeStream?.Dispose();
            childProcess.Terminate(1);
            childProcess.Dispose();
            jobObject.Dispose();
            throw;
        }
    }

    private async Task SendFrameAsync(ChannelTag channelTag, FrameFlags flags, byte[] payload, CancellationToken cancellationToken)
    {
        ulong seq = Interlocked.Increment(ref _sequenceNumber);
        var header = new FrameHeader
        {
            ChannelTag = channelTag,
            Flags = flags,
            SequenceNumber = seq
        };

        await _writeLock.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await FrameCodec.WriteFrameAsync(_pipeStream, header, payload, cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            _writeLock.Release();
        }
    }

    public async Task<ScanResult> StartScanAsync(
        string targetPath,
        IProgress<ScanProgressReport>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);

        string opId = $"scan-{Guid.NewGuid():N}";
        var scanReq = new CommandRequest
        {
            RequestId = opId,
            Scan = new ScanRequest
            {
                OperationId = opId,
                TargetPath = targetPath
            }
        };

        await SendFrameAsync(ChannelTag.Command, FrameFlags.None, scanReq.ToByteArray(), cancellationToken).ConfigureAwait(false);

        ulong lastProgressSeq = 0;
        bool cancelInitiated = false;
        DateTime? cancelDeadline = null;

        using var cancelReg = cancellationToken.Register(() =>
        {
            try
            {
                cancelInitiated = true;
                cancelDeadline = DateTime.UtcNow.AddSeconds(2);
                _ = CancelAsync(opId, "Scan cancelled by client", CancellationToken.None);
            }
            catch
            {
                // Best effort cancellation
            }
        });

        while (true)
        {
            if (cancelDeadline.HasValue && DateTime.UtcNow >= cancelDeadline.Value)
            {
                // Enforce bounded 2-second cancellation settlement timeout
                return new ScanResult
                {
                    OperationId = opId,
                    TargetPath = targetPath,
                    Outcome = ScanOutcome.Cancelled,
                    ErrorMessage = "Scan cancelled and settlement timed out after 2 seconds"
                };
            }

            var frame = await FrameCodec.ReadFrameAsync(_pipeStream, CancellationToken.None).ConfigureAwait(false);
            if (frame == null)
            {
                throw new EndOfStreamException("Engine closed connection prematurely during scan");
            }

            if (frame.Header.ChannelTag == ChannelTag.ProgressPulse)
            {
                var cmdResp = CommandResponse.Parser.ParseFrom(frame.Payload);
                if (cmdResp.ScanProgress == null)
                {
                    throw new InvalidDataException("ProgressPulse channel frame did not contain ScanProgress payload");
                }

                var p = cmdResp.ScanProgress;
                if (p.OperationId != opId)
                {
                    throw new InvalidDataException($"Progress pulse operation ID mismatch: expected '{opId}', got '{p.OperationId}'");
                }

                if (p.SequenceNumber <= lastProgressSeq)
                {
                    throw new InvalidDataException($"Progress pulse sequence number not strictly increasing: expected > {lastProgressSeq}, got {p.SequenceNumber}");
                }
                lastProgressSeq = p.SequenceNumber;

                progress?.Report(new ScanProgressReport(
                    p.OperationId,
                    p.SequenceNumber,
                    p.TimestampIso,
                    p.ObservedDirectories,
                    p.ObservedFiles,
                    p.ObservedLogicalBytes,
                    p.ObservedAllocatedBytes,
                    p.CoverageGaps,
                    p.CurrentPhase));
            }
            else if (frame.Header.ChannelTag == ChannelTag.Command)
            {
                var cmdResp = CommandResponse.Parser.ParseFrom(frame.Payload);
                if (!string.IsNullOrEmpty(cmdResp.RequestId) && cmdResp.RequestId != opId)
                {
                    throw new InvalidDataException($"Command response request ID mismatch: expected '{opId}', got '{cmdResp.RequestId}'");
                }

                if (cmdResp.ScanResponse != null)
                {
                    var r = cmdResp.ScanResponse;
                    if (r.OperationId != opId)
                    {
                        throw new InvalidDataException($"ScanResponse operation ID mismatch: expected '{opId}', got '{r.OperationId}'");
                    }

                    var outcome = (cancelInitiated || r.RunOutcome == ScanRunOutcome.Cancelled)
                        ? ScanOutcome.Cancelled
                        : r.RunOutcome switch
                        {
                            ScanRunOutcome.Finished => ScanOutcome.Finished,
                            ScanRunOutcome.Cancelled => ScanOutcome.Cancelled,
                            _ => ScanOutcome.Failed
                        };

                    var gaps = r.CoverageGaps.Select(g => new CoverageGapItem
                    {
                        DisplayPath = g.DisplayPath,
                        StatusCode = g.StatusCode,
                        NativeError = g.NativeError,
                        ErrorMessage = g.ErrorMessage
                    }).ToList();

                    return new ScanResult
                    {
                        OperationId = r.OperationId,
                        TargetPath = r.TargetPath,
                        Outcome = outcome,
                        ScopeCoverage = r.ScopeCoverage.ToString(),
                        DirectoryCount = r.DirectoryCount,
                        FileCount = r.FileCount,
                        SpecialCount = r.SpecialCount,
                        LogicalBytes = r.LogicalBytes,
                        AllocatedBytes = r.AllocatedBytes,
                        AllocatedBytesKnown = r.AllocatedBytesKnown,
                        DurationMs = r.DurationMs,
                        CoverageGaps = gaps
                    };
                }
                else if (cmdResp.Error != null || cmdResp.Status != 0)
                {
                    return new ScanResult
                    {
                        OperationId = opId,
                        TargetPath = targetPath,
                        Outcome = ScanOutcome.Failed,
                        ErrorCode = cmdResp.Error?.Code ?? cmdResp.ErrorCode ?? "OPERATION_FAILED",
                        ErrorMessage = cmdResp.Error?.Message ?? cmdResp.ErrorMessage ?? "Engine reported failure"
                    };
                }
                else
                {
                    throw new InvalidDataException("Command channel frame contained unexpected response variant during scan");
                }
            }
            else
            {
                throw new InvalidDataException($"Unexpected channel tag '{frame.Header.ChannelTag}' during scan");
            }
        }
    }

    public async Task<IReadOnlyList<DirectoryEntryInfo>> GetChildrenAsync(
        string operationId,
        uint parentId,
        uint offset = 0,
        uint limit = 100,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);

        string reqId = $"get-children-{Guid.NewGuid():N}";
        var req = new CommandRequest
        {
            RequestId = reqId,
            GetChildren = new GetChildrenRequest
            {
                OperationId = operationId,
                ParentId = parentId,
                Offset = offset,
                Limit = limit
            }
        };

        await SendFrameAsync(ChannelTag.Command, FrameFlags.None, req.ToByteArray(), cancellationToken).ConfigureAwait(false);

        var frame = await FrameCodec.ReadFrameAsync(_pipeStream, cancellationToken).ConfigureAwait(false);
        if (frame == null)
        {
            throw new EndOfStreamException("Engine closed connection prematurely during GetChildren query");
        }

        if (frame.Header.ChannelTag != ChannelTag.Command)
        {
            throw new InvalidDataException($"GetChildren response received on invalid channel: {frame.Header.ChannelTag}");
        }

        var cmdResp = CommandResponse.Parser.ParseFrom(frame.Payload);
        if (cmdResp.Status != 0 || cmdResp.Error != null)
        {
            string errCode = cmdResp.Error?.Code ?? cmdResp.ErrorCode ?? "GET_CHILDREN_FAILED";
            string errMsg = cmdResp.Error?.Message ?? cmdResp.ErrorMessage ?? "Failed to retrieve children";
            throw new InvalidOperationException($"GetChildren failed ({errCode}): {errMsg}");
        }

        if (cmdResp.GetChildren == null)
        {
            throw new InvalidDataException("GetChildren response variant not present in CommandResponse");
        }

        return cmdResp.GetChildren.Nodes.Select(n => new DirectoryEntryInfo
        {
            Id = n.Id,
            ParentId = n.ParentId,
            Name = n.Name,
            EntryKind = n.EntryKind,
            LogicalSize = n.LogicalSize,
            AllocatedSize = n.AllocatedSize,
            AllocatedSizeKnown = n.AllocatedSizeKnown,
            ChildCount = n.ChildCount,
            HasChildren = n.HasChildren
        }).ToList();
    }

    public async Task CancelAsync(string operationId, string reason = "User requested cancellation", CancellationToken cancellationToken = default)
    {
        if (_disposed || !_pipeStream.IsConnected)
        {
            return;
        }

        var cancelReq = new CommandRequest
        {
            RequestId = $"cancel-{Guid.NewGuid():N}",
            Cancel = new CancelRequest
            {
                TargetRequestId = operationId,
                Reason = reason
            }
        };

        try
        {
            await SendFrameAsync(ChannelTag.Command, FrameFlags.None, cancelReq.ToByteArray(), cancellationToken).ConfigureAwait(false);
        }
        catch
        {
            // Best effort cancellation
        }
    }

    public async Task ShutdownAsync(CancellationToken cancellationToken = default)
    {
        if (_disposed || !_pipeStream.IsConnected)
        {
            return;
        }

        var shutdownReq = new CommandRequest
        {
            RequestId = "shutdown",
            Shutdown = new ShutdownRequest()
        };

        try
        {
            await SendFrameAsync(ChannelTag.Command, FrameFlags.None, shutdownReq.ToByteArray(), cancellationToken).ConfigureAwait(false);
        }
        catch
        {
            // Best effort shutdown
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (!_disposed)
        {
            _disposed = true;
            try
            {
                await ShutdownAsync().ConfigureAwait(false);
            }
            catch
            {
            }
            _pipeStream.Dispose();
            _writeLock.Dispose();
            _childProcess.Dispose();
            _jobObject.Dispose();
        }
    }

    public void Dispose()
    {
        if (!_disposed)
        {
            _disposed = true;
            _pipeStream.Dispose();
            _writeLock.Dispose();
            _childProcess.Dispose();
            _jobObject.Dispose();
        }
    }
}
