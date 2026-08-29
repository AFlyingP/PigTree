namespace PigTree.Model;

public sealed record PagedChildrenResult(
    string OperationId,
    uint ParentId,
    uint TotalChildren,
    uint Offset,
    IReadOnlyList<TreeNodeData> Nodes);
