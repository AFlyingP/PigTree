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
    public ulong LogicalSize { get; }
    public ulong AllocatedSize { get; }
    public bool AllocatedSizeKnown { get; }
    public uint ChildCount { get; }
    public bool HasChildren { get; }
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

    public string FormattedLogicalSize => CoalescedProgressState.FormatBytes(LogicalSize);
    public string FormattedAllocatedSize => AllocatedSizeKnown ? CoalescedProgressState.FormatBytes(AllocatedSize) : "-";
    public string FormattedItemCount => IsDirectory ? ChildCount.ToString("N0") : "-";
    public double IndentMargin => Level * 16.0;

    public string AutomationName => $"{Name}, {FormattedLogicalSize}, {(IsDirectory ? $"{ChildCount} items" : "file")}";

    public TreeItemViewModel(TreeNodeData data, int level, TreeItemViewModel? parentItem = null)
    {
        Id = data.Id;
        ParentId = data.ParentId;
        Name = data.Name;
        EntryKind = data.EntryKind;
        Level = level;
        LogicalSize = data.LogicalSize;
        AllocatedSize = data.AllocatedSize;
        AllocatedSizeKnown = data.AllocatedSizeKnown;
        ChildCount = data.ChildCount;
        HasChildren = data.HasChildren;
        ScopeCoverage = data.ScopeCoverage;
        CoverageGaps = data.CoverageGaps;
        ParentItem = parentItem;
    }
}
