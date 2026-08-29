using PigTree.Model;

namespace PigTree.Ipc;

public interface IEngineSession : IAsyncDisposable, IDisposable
{
    bool IsConnected { get; }
    string SessionId { get; }
    uint EnginePid { get; }

    Task<ScanResult> StartScanAsync(string targetPath, IProgress<ScanProgressReport>? progress = null, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<DirectoryEntryInfo>> GetChildrenAsync(string operationId, uint parentId, uint offset = 0, uint limit = 100, CancellationToken cancellationToken = default);
    Task CancelAsync(string operationId, string reason = "User requested cancellation", CancellationToken cancellationToken = default);
    Task ShutdownAsync(CancellationToken cancellationToken = default);
}
