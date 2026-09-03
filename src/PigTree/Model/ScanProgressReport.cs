namespace PigTree.Model;

public sealed record ScanProgressReport(
    string OperationId,
    ulong SequenceNumber,
    string TimestampIso,
    ulong ObservedDirectories,
    ulong ObservedFiles,
    ulong ObservedLogicalBytes,
    ulong ObservedReferencedAllocatedBytes,
    uint CoverageGaps,
    string CurrentPhase,
    string CurrentDirectory = "");
