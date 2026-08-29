using System;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Automation.Peers;
using System.Windows.Controls;
using System.Windows.Input;
using PigTree.Accessibility;
using PigTree.ViewModel;

namespace PigTree.Controls;

public class DirectoryTreeView : ListView
{
    static DirectoryTreeView()
    {
        DefaultStyleKeyProperty.OverrideMetadata(typeof(DirectoryTreeView), new FrameworkPropertyMetadata(typeof(DirectoryTreeView)));
    }

    protected override AutomationPeer OnCreateAutomationPeer()
    {
        return new DirectoryTreeViewAutomationPeer(this);
    }

    protected override void PrepareContainerForItemOverride(DependencyObject element, object item)
    {
        base.PrepareContainerForItemOverride(element, item);
        if (element is ListViewItem lvi && item is TreeItemViewModel vm)
        {
            AutomationProperties.SetName(lvi, vm.AutomationName);
            AutomationProperties.SetAutomationId(lvi, $"TreeItem_{vm.Id}");
            AutomationProperties.SetItemType(lvi, vm.IsDirectory ? "Directory" : "File");
            AutomationProperties.SetItemStatus(lvi, vm.ScopeCoverage);
            AutomationProperties.SetPositionInSet(lvi, vm.ParentItem != null && vm.ParentItem.Children.Count > 0 ? vm.ParentItem.Children.IndexOf(vm) + 1 : 1);
            AutomationProperties.SetSizeOfSet(lvi, vm.ParentItem != null && vm.ParentItem.Children.Count > 0 ? vm.ParentItem.Children.Count : 1);
        }
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (DataContext is MainViewModel mainVm)
        {
            var projection = mainVm.Projection;
            switch (e.Key)
            {
                case Key.Up:
                    projection.NavigateUp();
                    ScrollSelectedItemIntoView();
                    e.Handled = true;
                    break;
                case Key.Down:
                    projection.NavigateDown();
                    ScrollSelectedItemIntoView();
                    e.Handled = true;
                    break;
                case Key.Right:
                    ExecuteObservedAsync(async () =>
                    {
                        await projection.NavigateRightAsync();
                        ScrollSelectedItemIntoView();
                    });
                    e.Handled = true;
                    break;
                case Key.Left:
                    projection.NavigateLeft();
                    ScrollSelectedItemIntoView();
                    e.Handled = true;
                    break;
                case Key.Home:
                    projection.NavigateHome();
                    ScrollSelectedItemIntoView();
                    e.Handled = true;
                    break;
                case Key.End:
                    projection.NavigateEnd();
                    ScrollSelectedItemIntoView();
                    e.Handled = true;
                    break;
                case Key.PageUp:
                    projection.NavigatePageUp(GetEstimatedPageSize());
                    ScrollSelectedItemIntoView();
                    e.Handled = true;
                    break;
                case Key.PageDown:
                    projection.NavigatePageDown(GetEstimatedPageSize());
                    ScrollSelectedItemIntoView();
                    e.Handled = true;
                    break;
                case Key.Enter:
                case Key.Space:
                    ExecuteObservedAsync(async () =>
                    {
                        await projection.ToggleExpandAsync();
                        ScrollSelectedItemIntoView();
                    });
                    e.Handled = true;
                    break;
            }
        }

        if (!e.Handled)
        {
            base.OnKeyDown(e);
        }
    }

    private void ScrollSelectedItemIntoView()
    {
        if (SelectedItem != null)
        {
            ScrollIntoView(SelectedItem);
        }
    }

    public int GetEstimatedPageSize()
    {
        double height = ActualHeight;
        if (height > 0)
        {
            return Math.Max(1, (int)Math.Floor(height / 22.0));
        }
        return 10;
    }

    private async void ExecuteObservedAsync(Func<Task> action)
    {
        try
        {
            await action();
        }
        catch (Exception ex)
        {
            if (DataContext is MainViewModel mainVm)
            {
                mainVm.SetErrorForTesting($"Keyboard navigation error: {ex.Message}");
            }
        }
    }
}
