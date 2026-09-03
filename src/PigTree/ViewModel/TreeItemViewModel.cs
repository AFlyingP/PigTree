using System;
using System.Collections.Generic;
using System.Windows.Input;
using PigTree.Model;

namespace PigTree.ViewModel;

public sealed class TreeItemViewModel : ViewModelBase
{
    private bool _isExpanded;
    private bool _isLoading;
    private bool _isSelected;
    private bool _isFocused;

    public uint Id { get; }
    public uint ParentId { get; }
    public string Name { get; }
    public uint EntryKind { get; }
    public int Level { get; }
    public ulong LogicalBytes { get; }
    public ulong ReferencedAllocatedBytes { get; }
    public bool AllocatedSizeKnown { get; }
    public uint ChildCount { get; }
    public bool HasChildren { get; }
    public ulong UniqueAllocatedBytes { get; }
    public uint ObservedAliasCount { get; }
    public string TotalLinkCountStatus { get; }
    public uint? TotalLinkCountValue { get; }
    public string ExternalReferenceStatus { get; }
    public ulong KnownSubtotalAllocatedBytes { get; }
    public string ScopeCoverage { get; }
    public uint CoverageGaps { get; }

    public TreeItemViewModel? ParentItem { get; set; }
    public List<TreeItemViewModel> Children { get; } = new();

    public ICommand? ToggleExpandCommand { get; set; }

    public bool IsDirectory => EntryKind == 1;
    public bool IsFile => EntryKind == 2;
    public bool IsSpecial => EntryKind == 3;

    public bool IsExpanded
    {
        get => _isExpanded;
        set => SetProperty(ref _isExpanded, value);
    }

    public bool IsLoading
    {
        get => _isLoading;
        set => SetProperty(ref _isLoading, value);
    }

    public bool IsSelected
    {
        get => _isSelected;
        set => SetProperty(ref _isSelected, value);
    }

    public bool IsFocused
    {
        get => _isFocused;
        set => SetProperty(ref _isFocused, value);
    }

    public string FormattedLogicalSize => CoalescedProgressState.FormatBytes(LogicalBytes);

    public string FormattedReferencedAllocated => AllocatedSizeKnown
        ? CoalescedProgressState.FormatBytes(ReferencedAllocatedBytes)
        : "-";

    public string FormattedUniqueAllocated => AllocatedSizeKnown
        ? CoalescedProgressState.FormatBytes(UniqueAllocatedBytes)
        : "-";

    public string FormattedItemCount => IsDirectory ? ChildCount.ToString("N0") : "-";
    public double IndentMargin => Level * 16.0;

    /// <summary>
    /// True when at least two directory entries refer to the same underlying object
    /// (issue #20 hard link aliases).
    /// </summary>
    public bool HasAliasBadge => ObservedAliasCount >= 2;

    /// <summary>Empty when no alias badge should be shown; otherwise e.g. "×2 aliases".</summary>
    public string AliasBadgeText => HasAliasBadge ? $"×{ObservedAliasCount} aliases" : string.Empty;

    /// <summary>
    /// True only when external reference evidence is confirmed external (issue #20);
    /// indeterminate evidence is never badged per row.
    /// </summary>
    public bool HasExternalLinkBadge => string.Equals(ExternalReferenceStatus, ExternalReference.ConfirmedExternal, StringComparison.Ordinal);

    /// <summary>Empty when no external link indicator should be shown.</summary>
    public string ExternalLinkBadgeText => HasExternalLinkBadge ? "external link" : string.Empty;

    public string AutomationName
    {
        get
        {
            string automationName =
                $"{Name}, {FormattedLogicalSize}, {FormattedReferencedAllocated}, {(IsDirectory ? $"{ChildCount} items" : "file")}";
            if (HasAliasBadge)
            {
                automationName += $", {ObservedAliasCount} aliases";
            }
            if (HasExternalLinkBadge)
            {
                automationName += ", external link";
            }
            return automationName;
        }
    }

    public TreeItemViewModel(TreeNodeData data, int level, TreeItemViewModel? parentItem = null)
    {
        Id = data.Id;
        ParentId = data.ParentId;
        Name = data.Name;
        EntryKind = data.EntryKind;
        Level = level;
        LogicalBytes = data.LogicalBytes;
        ReferencedAllocatedBytes = data.ReferencedAllocatedBytes;
        AllocatedSizeKnown = data.AllocatedSizeKnown;
        ChildCount = data.ChildCount;
        HasChildren = data.HasChildren;
        UniqueAllocatedBytes = data.UniqueAllocatedBytes;
        ObservedAliasCount = data.ObservedAliasCount;
        TotalLinkCountStatus = data.TotalLinkCountStatus;
        TotalLinkCountValue = data.TotalLinkCountValue;
        ExternalReferenceStatus = data.ExternalReferenceStatus;
        KnownSubtotalAllocatedBytes = data.KnownSubtotalAllocatedBytes;
        ScopeCoverage = data.ScopeCoverage;
        CoverageGaps = data.CoverageGaps;
        ParentItem = parentItem;
    }
}
