using System;
using System.Windows.Automation.Peers;
using System.Windows.Automation.Provider;
using PigTree.Controls;

namespace PigTree.Accessibility;

public sealed class DirectoryTreeViewAutomationPeer : ListViewAutomationPeer, ISelectionProvider
{
    public DirectoryTreeViewAutomationPeer(DirectoryTreeView treeView) : base(treeView)
    {
    }

    protected override AutomationControlType GetAutomationControlTypeCore()
    {
        return AutomationControlType.Tree;
    }

    protected override string GetClassNameCore()
    {
        return "DirectoryTreeView";
    }

    protected override string GetNameCore()
    {
        return "Directory Tree";
    }

    protected override ItemAutomationPeer CreateItemAutomationPeer(object item)
    {
        return new TreeItemAutomationPeer(item, this);
    }

    public override object? GetPattern(PatternInterface patternInterface)
    {
        if (patternInterface == PatternInterface.Selection)
        {
            return this;
        }

        return base.GetPattern(patternInterface);
    }
}
