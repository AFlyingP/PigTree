using PigTree.Model;

namespace PigTree.Services;

public interface ITreePageProvider
{
    Task<PagedChildrenResult> GetChildrenAsync(
        string operationId,
        uint parentId,
        uint offset,
        uint limit,
        CancellationToken cancellationToken = default);

    Task<TreeNodeData?> GetRootNodeAsync(
        string operationId,
        CancellationToken cancellationToken = default);
}
