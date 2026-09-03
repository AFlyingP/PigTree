namespace PigTree.Model;

public enum ScanOutcome
{
    Finished = 1,
    Cancelled = 2,
    Failed = 3,
}

public sealed class CoverageGapItem
{
    public string DisplayPath { get; init; } = string.Empty;
    public string StatusCode { get; init; } = string.Empty;
    public uint NativeError { get; init; }
    public string ErrorMessage { get; init; } = string.Empty;
}

public sealed class ScanResult
{
    public string OperationId { get; init; } = string.Empty;
    public string TargetPath { get; init; } = string.Empty;
    public ScanOutcome Outcome { get; init; }
    public string ScopeCoverage { get; init; } = "Complete";
    public ulong DirectoryCount { get; init; }
    public ulong FileCount { get; init; }
    public ulong SpecialCount { get; init; }
    public ulong LogicalBytes { get; init; }
    public ulong AllocatedBytes { get; init; }
    public bool AllocatedBytesKnown { get; init; }

    /// <summary>Additive physical allocation across entry paths (primary hierarchy metric).</summary>
    public ulong ReferencedAllocatedBytes { get; init; }

    /// <summary>Physical allocation counted once per distinct object.</summary>
    public ulong UniqueAllocatedBytes { get; init; }

    /// <summary>Subtotal of allocations that are known without ambiguity.</summary>
    public ulong KnownSubtotalAllocatedBytes { get; init; }

    /// <summary>
    /// Objects whose external reference evidence was indeterminate; surfaced once at the
    /// scan target summary level (issue #20 AC-7), never badged per row.
    /// </summary>
    public ulong IndeterminateExternalReferenceObjects { get; init; }

    public ulong DurationMs { get; init; }
    public IReadOnlyList<CoverageGapItem> CoverageGaps { get; init; } = Array.Empty<CoverageGapItem>();
    public string? ErrorCode { get; init; }
    public string? ErrorMessage { get; init; }

    public bool IsSuccess => Outcome == ScanOutcome.Finished && string.IsNullOrEmpty(ErrorCode);
    public bool IsCancelled => Outcome == ScanOutcome.Cancelled;
}
