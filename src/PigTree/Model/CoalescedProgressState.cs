using System;

namespace PigTree.Model;

public sealed class CoalescedProgressState
{
    public string OperationId { get; init; } = string.Empty;
    public ulong SequenceNumber { get; init; }
    public ulong Directories { get; init; }
    public ulong Files { get; init; }
    public ulong LogicalBytes { get; init; }
    public ulong ReferencedAllocatedBytes { get; init; }
    public uint CoverageGaps { get; init; }
    public string CurrentPhase { get; init; } = string.Empty;
    public string CurrentDirectory { get; init; } = string.Empty;
    public TimeSpan Elapsed { get; init; }

    public string FormattedDirectories => Directories.ToString("N0");
    public string FormattedFiles => Files.ToString("N0");
    public string FormattedLogicalBytes => FormatBytes(LogicalBytes);
    public string FormattedReferencedAllocatedBytes => FormatBytes(ReferencedAllocatedBytes);
    public string FormattedElapsed => Elapsed.TotalHours >= 1
        ? $"{(int)Elapsed.TotalHours:00}:{Elapsed.Minutes:00}:{Elapsed.Seconds:00}"
        : $"{Elapsed.Minutes:00}:{Elapsed.Seconds:00}.{Elapsed.Milliseconds / 100}";

    public static string FormatBytes(ulong bytes)
    {
        string[] suffixes = { "B", "KB", "MB", "GB", "TB", "PB" };
        int counter = 0;
        decimal number = bytes;
        while (Math.Round(number / 1024m, 1) >= 1 && counter < suffixes.Length - 1)
        {
            number /= 1024m;
            counter++;
        }
        return $"{number:0.##} {suffixes[counter]}";
    }
}
