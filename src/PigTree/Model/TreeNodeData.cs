namespace PigTree.Model;

public sealed record TreeNodeData(
    uint Id,
    uint ParentId,
    string Name,
    uint EntryKind,
    ulong LogicalSize,
    ulong AllocatedSize,
    bool AllocatedSizeKnown,
    uint ChildCount,
    bool HasChildren,
    string ScopeCoverage = "Complete",
    uint CoverageGaps = 0)
{
    public bool IsDirectory => EntryKind == 1;
    public bool IsFile => EntryKind == 2;
    public bool IsSpecial => EntryKind == 3;
}
