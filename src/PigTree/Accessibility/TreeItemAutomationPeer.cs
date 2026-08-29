using System;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Automation.Peers;
using System.Windows.Automation.Provider;
using PigTree.ViewModel;

namespace PigTree.Accessibility;

public sealed class TreeItemAutomationPeer : ListBoxItemAutomationPeer, IExpandCollapseProvider, ISelectionItemProvider
{
    private readonly SelectorAutomationPeer _selectorPeer;

    public TreeItemAutomationPeer(object owner, SelectorAutomationPeer selectorAutomationPeer)
        : base(owner, selectorAutomationPeer)
    {
        _selectorPeer = selectorAutomationPeer ?? throw new ArgumentNullException(nameof(selectorAutomationPeer));
    }

    protected override AutomationControlType GetAutomationControlTypeCore()
    {
        return AutomationControlType.TreeItem;
    }

    protected override string GetClassNameCore()
    {
        return "TreeItem";
    }

    protected override string GetNameCore()
    {
        if (Item is TreeItemViewModel vm)
        {
            return vm.AutomationName;
        }
        return base.GetNameCore();
    }

    protected override string GetAutomationIdCore()
    {
        if (Item is TreeItemViewModel vm)
        {
            return $"TreeItem_{vm.Id}";
        }
        return base.GetAutomationIdCore();
    }

    public override object? GetPattern(PatternInterface patternInterface)
    {
        if (patternInterface == PatternInterface.ExpandCollapse)
        {
            if (Item is TreeItemViewModel vm && vm.IsDirectory)
            {
                return this;
            }
            return null;
        }

        if (patternInterface == PatternInterface.SelectionItem)
        {
            return this;
        }

        return base.GetPattern(patternInterface);
    }

    // IExpandCollapseProvider
    public ExpandCollapseState ExpandCollapseState
    {
        get
        {
            if (Item is TreeItemViewModel vm)
            {
                if (!vm.IsDirectory || !vm.HasChildren)
                {
                    return ExpandCollapseState.LeafNode;
                }
                return vm.IsExpanded ? ExpandCollapseState.Expanded : ExpandCollapseState.Collapsed;
            }
            return ExpandCollapseState.LeafNode;
        }
    }

    public void Expand()
    {
        if (Item is TreeItemViewModel vm && vm.IsDirectory && vm.HasChildren && !vm.IsExpanded)
        {
            var oldState = ExpandCollapseState;
            if (vm.ToggleExpandCommand != null && vm.ToggleExpandCommand.CanExecute(vm))
            {
                vm.ToggleExpandCommand.Execute(vm);
            }
            else if (_selectorPeer.Owner is FrameworkElement fe && fe.DataContext is MainViewModel mainVm)
            {
                _ = mainVm.Projection.ExpandAsync(vm);
            }
            RaisePropertyChangedEvent(
                ExpandCollapsePatternIdentifiers.ExpandCollapseStateProperty,
                oldState,
                ExpandCollapseState.Expanded);
        }
    }

    public void Collapse()
    {
        if (Item is TreeItemViewModel vm && vm.IsDirectory && vm.IsExpanded)
        {
            var oldState = ExpandCollapseState;
            if (vm.ToggleExpandCommand != null && vm.ToggleExpandCommand.CanExecute(vm))
            {
                vm.ToggleExpandCommand.Execute(vm);
            }
            else if (_selectorPeer.Owner is FrameworkElement fe && fe.DataContext is MainViewModel mainVm)
            {
                mainVm.Projection.Collapse(vm);
            }
            RaisePropertyChangedEvent(
                ExpandCollapsePatternIdentifiers.ExpandCollapseStateProperty,
                oldState,
                ExpandCollapseState.Collapsed);
        }
    }

    // ISelectionItemProvider
    public bool IsSelected => (Item as TreeItemViewModel)?.IsSelected ?? false;

    public IRawElementProviderSimple? SelectionContainer
    {
        get
        {
            var provider = ProviderFromPeer(_selectorPeer);
            if (provider != null)
            {
                return provider;
            }
            if (_selectorPeer is IRawElementProviderSimple simple)
            {
                return simple;
            }
            return new RawElementProviderSimpleWrapper(_selectorPeer);
        }
    }

    public void AddToSelection() => Select();

    public void RemoveFromSelection()
    {
        if (Item is TreeItemViewModel vm)
        {
            vm.IsSelected = false;
        }
    }

    public void Select()
    {
        if (Item is TreeItemViewModel vm)
        {
            if (_selectorPeer.Owner is FrameworkElement fe && fe.DataContext is MainViewModel mainVm)
            {
                mainVm.SelectedTreeItem = vm;
            }
            else
            {
                vm.IsSelected = true;
            }
        }
    }
}

internal sealed class RawElementProviderSimpleWrapper : IRawElementProviderSimple
{
    private readonly AutomationPeer _peer;

    public RawElementProviderSimpleWrapper(AutomationPeer peer)
    {
        _peer = peer ?? throw new ArgumentNullException(nameof(peer));
    }

    public ProviderOptions ProviderOptions => ProviderOptions.ServerSideProvider;
    public IRawElementProviderSimple? HostRawElementProvider => null;
    public object? GetPatternProvider(int patternId) => _peer.GetPattern((PatternInterface)patternId);
    public object? GetPropertyValue(int propertyId) => null;
}
