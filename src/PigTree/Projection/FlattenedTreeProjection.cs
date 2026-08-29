using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Input;
using PigTree.Model;
using PigTree.Services;
using PigTree.ViewModel;

namespace PigTree.Projection;

public sealed class FlattenedTreeProjection : ViewModelBase
{
    private readonly ITreePageProvider _pageProvider;
    private readonly BoundedPageCache _cache;
    private readonly uint _pageSize;
    private string _currentOperationId = string.Empty;
    private TreeItemViewModel? _selectedItem;

    public ObservableCollection<TreeItemViewModel> VisibleItems { get; } = new();

    public Action<string>? ErrorReporter { get; set; }

    public TreeItemViewModel? SelectedItem
    {
        get => _selectedItem;
        set
        {
            if (_selectedItem != null)
            {
                _selectedItem.IsSelected = false;
            }
            if (SetProperty(ref _selectedItem, value))
            {
                if (_selectedItem != null)
                {
                    _selectedItem.IsSelected = true;
                }
                OnPropertyChanged(nameof(SelectedIndex));
            }
        }
    }

    public int SelectedIndex
    {
        get => _selectedItem == null ? -1 : VisibleItems.IndexOf(_selectedItem);
        set
        {
            if (value >= 0 && value < VisibleItems.Count)
            {
                SelectedItem = VisibleItems[value];
            }
            else
            {
                SelectedItem = null;
            }
        }
    }

    public ICommand ToggleExpandCommand { get; }

    public FlattenedTreeProjection(ITreePageProvider pageProvider, uint pageSize = 500, int maxCachedPages = 500)
    {
        _pageProvider = pageProvider ?? throw new ArgumentNullException(nameof(pageProvider));
        _pageSize = pageSize;
        _cache = new BoundedPageCache(maxCachedPages);
        ToggleExpandCommand = new AsyncRelayCommand(async param =>
        {
            if (param is TreeItemViewModel item)
            {
                await ToggleExpandAsync(item);
            }
            else
            {
                await ToggleExpandAsync(SelectedItem);
            }
        });
    }

    public Task InitializeRootAsync(string operationId, TreeNodeData rootNode)
    {
        _currentOperationId = operationId;
        _cache.Clear();
        VisibleItems.Clear();

        var rootVm = new TreeItemViewModel(rootNode, level: 0)
        {
            ToggleExpandCommand = ToggleExpandCommand
        };
        VisibleItems.Add(rootVm);
        SelectedItem = rootVm;
        return Task.CompletedTask;
    }

    public async Task ExpandAsync(TreeItemViewModel item, CancellationToken cancellationToken = default)
    {
        if (!item.HasChildren || item.IsExpanded || item.IsLoading)
        {
            return;
        }

        item.IsLoading = true;
        try
        {
            PagedChildrenResult? page;
            if (!_cache.TryGetPage(_currentOperationId, item.Id, 0, out page) || page == null)
            {
                page = await _pageProvider.GetChildrenAsync(_currentOperationId, item.Id, 0, _pageSize, cancellationToken);
                _cache.PutPage(page);
            }

            int index = VisibleItems.IndexOf(item);
            if (index < 0)
            {
                return;
            }

            item.Children.Clear();
            int insertIndex = index + 1;

            foreach (var node in page.Nodes)
            {
                var childVm = new TreeItemViewModel(node, item.Level + 1, item)
                {
                    ToggleExpandCommand = ToggleExpandCommand
                };
                item.Children.Add(childVm);
                VisibleItems.Insert(insertIndex++, childVm);
            }

            item.IsExpanded = true;
        }
        catch (OperationCanceledException)
        {
            // Expansion cancelled
        }
        catch (Exception ex)
        {
            ErrorReporter?.Invoke($"Failed to expand '{item.Name}': {ex.Message}");
            throw;
        }
        finally
        {
            item.IsLoading = false;
        }
    }

    public void Collapse(TreeItemViewModel item)
    {
        if (!item.IsExpanded)
        {
            return;
        }

        int index = VisibleItems.IndexOf(item);
        if (index < 0)
        {
            return;
        }

        bool selectionRemoved = false;
        var selected = SelectedItem;

        int removeIndex = index + 1;
        while (removeIndex < VisibleItems.Count && VisibleItems[removeIndex].Level > item.Level)
        {
            var candidate = VisibleItems[removeIndex];
            if (candidate == selected)
            {
                selectionRemoved = true;
            }
            VisibleItems.RemoveAt(removeIndex);
        }

        item.IsExpanded = false;

        if (selectionRemoved)
        {
            SelectedItem = item;
        }
    }

    public async Task ToggleExpandAsync(TreeItemViewModel? item = null, CancellationToken ct = default)
    {
        var target = item ?? SelectedItem;
        if (target == null || !target.IsDirectory)
        {
            return;
        }

        try
        {
            if (target.IsExpanded)
            {
                Collapse(target);
            }
            else
            {
                await ExpandAsync(target, ct);
            }
        }
        catch (OperationCanceledException)
        {
            // Expected cancellation
        }
        catch (Exception ex)
        {
            ErrorReporter?.Invoke($"Failed to toggle '{target.Name}': {ex.Message}");
        }
    }

    public void NavigateUp()
    {
        int index = SelectedIndex;
        if (index > 0)
        {
            SelectedIndex = index - 1;
        }
    }

    public void NavigateDown()
    {
        int index = SelectedIndex;
        if (index >= 0 && index < VisibleItems.Count - 1)
        {
            SelectedIndex = index + 1;
        }
    }

    public async Task NavigateRightAsync(CancellationToken ct = default)
    {
        var item = SelectedItem;
        if (item == null) return;

        try
        {
            if (item.HasChildren && !item.IsExpanded)
            {
                await ExpandAsync(item, ct);
            }
            else if (item.IsExpanded)
            {
                int index = SelectedIndex;
                if (index >= 0 && index < VisibleItems.Count - 1 && VisibleItems[index + 1].ParentItem == item)
                {
                    SelectedIndex = index + 1;
                }
            }
        }
        catch (OperationCanceledException)
        {
            // Expected cancellation
        }
        catch (Exception ex)
        {
            ErrorReporter?.Invoke($"Failed to navigate right: {ex.Message}");
        }
    }

    public void NavigateLeft()
    {
        var item = SelectedItem;
        if (item == null) return;

        if (item.IsExpanded)
        {
            Collapse(item);
        }
        else if (item.ParentItem != null)
        {
            SelectedItem = item.ParentItem;
        }
    }

    public void NavigateHome()
    {
        if (VisibleItems.Count > 0)
        {
            SelectedIndex = 0;
        }
    }

    public void NavigateEnd()
    {
        if (VisibleItems.Count > 0)
        {
            SelectedIndex = VisibleItems.Count - 1;
        }
    }

    public void NavigatePageUp(int count = 10)
    {
        int index = Math.Max(0, SelectedIndex - count);
        SelectedIndex = index;
    }

    public void NavigatePageDown(int count = 10)
    {
        int index = Math.Min(VisibleItems.Count - 1, SelectedIndex + count);
        SelectedIndex = index;
    }
}