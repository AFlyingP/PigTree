namespace PigTree.Model;

public sealed record TreeNodeData(
    uint Id,
    uint ParentId,
    string Name,
    uint EntryKind,
    ulong LogicalBytes,
    ulong ReferencedAllocatedBytes,
    bool AllocatedSizeKnown,
    uint ChildCount,
    bool HasChildren,
    string ScopeCoverage = "Complete",
    uint CoverageGaps = 0,
    ulong UniqueAllocatedBytes = 0,
    uint ObservedAliasCount = 1,
    string TotalLinkCountStatus = LinkCountKnowledge.NotObserved,
    uint? TotalLinkCountValue = null,
    string ExternalReferenceStatus = ExternalReference.ConfirmedNone,
    ulong KnownSubtotalAllocatedBytes = 0)
{
    public bool IsDirectory => EntryKind == 1;
    public bool IsFile => EntryKind == 2;
    public bool IsSpecial => EntryKind == 3;
}
