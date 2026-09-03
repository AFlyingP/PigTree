namespace PigTree.Model;

/// <summary>
/// Canonical lowercase knowledge states for the total link count of a filesystem object
/// (issue #20). Missing knowledge is never reported as zero.
/// </summary>
public static class LinkCountKnowledge
{
    public const string Known = "known";
    public const string NotObserved = "not_observed";
    public const string Unavailable = "unavailable";
    public const string NotApplicable = "not_applicable";
}

/// <summary>
/// Canonical lowercase external reference statuses for a filesystem object (issue #20).
/// Indeterminate evidence is surfaced once at the scan target summary level (AC-7),
/// never badged per row.
/// </summary>
public static class ExternalReference
{
    public const string ConfirmedNone = "confirmed_none";
    public const string ConfirmedExternal = "confirmed_external";
    public const string Indeterminate = "indeterminate";
    public const string InconsistentEvidence = "inconsistent_evidence";
    public const string NotApplicable = "not_applicable";
}

public sealed class DirectoryEntryInfo
{
    public uint Id { get; init; }
    public uint ParentId { get; init; }
    public string Name { get; init; } = string.Empty;
    public uint EntryKind { get; init; } // 1 = Directory, 2 = File, 3 = Special

    /// <summary>Addressable content bytes; scope aggregate for directories, self for files.</summary>
    public ulong LogicalBytes { get; init; }

    /// <summary>Additive physical allocation across entry paths (primary hierarchy metric).</summary>
    public ulong ReferencedAllocatedBytes { get; init; }

    public bool AllocatedSizeKnown { get; init; }
    public uint ChildCount { get; init; }
    public bool HasChildren { get; init; }

    /// <summary>Physical allocation counted once per distinct object.</summary>
    public ulong UniqueAllocatedBytes { get; init; }

    /// <summary>Observed directory entries referring to the same underlying object (aliases).</summary>
    public uint ObservedAliasCount { get; init; } = 1;

    /// <summary>One of <see cref="LinkCountKnowledge"/> canonical strings.</summary>
    public string TotalLinkCountStatus { get; init; } = LinkCountKnowledge.NotObserved;
    public uint? TotalLinkCountValue { get; init; }

    /// <summary>One of <see cref="ExternalReference"/> canonical strings.</summary>
    public string ExternalReferenceStatus { get; init; } = ExternalReference.ConfirmedNone;

    /// <summary>Subtotal of allocations that are known without ambiguity.</summary>
    public ulong KnownSubtotalAllocatedBytes { get; init; }

    public bool IsDirectory => EntryKind == 1;
    public bool IsFile => EntryKind == 2;
    public bool IsSpecial => EntryKind == 3;
}
