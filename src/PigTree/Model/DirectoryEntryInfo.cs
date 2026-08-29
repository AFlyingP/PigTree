namespace PigTree.Model;

public sealed class DirectoryEntryInfo
{
    public uint Id { get; init; }
    public uint ParentId { get; init; }
    public string Name { get; init; } = string.Empty;
    public uint EntryKind { get; init; } // 1 = Directory, 2 = File, 3 = Special
    public ulong LogicalSize { get; init; }
    public ulong AllocatedSize { get; init; }
    public bool AllocatedSizeKnown { get; init; }
    public uint ChildCount { get; init; }
    public bool HasChildren { get; init; }

    public bool IsDirectory => EntryKind == 1;
    public bool IsFile => EntryKind == 2;
    public bool IsSpecial => EntryKind == 3;
}
