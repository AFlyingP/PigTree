using System;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Automation;
using System.Windows.Automation.Peers;
using System.Windows.Automation.Provider;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Accessibility;
using PigTree.Controls;
using PigTree.Model;
using PigTree.Projection;
using PigTree.Tests.ProjectionTests;
using PigTree.ViewModel;

namespace PigTree.Tests.AccessibilityTests;

[TestClass]
public class AutomationPeerTests
{
    private static void RunInSta(Action action)
    {
        Exception? error = null;
        var thread = new Thread(() =>
        {
            try
            {
                action();
            }
            catch (Exception ex)
            {
                error = ex;
            }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();
        if (error != null)
        {
            throw new Exception("Error in STA thread", error);
        }
    }

    private static Task RunInStaAsync(Func<Task> action)
    {
        var tcs = new TaskCompletionSource();
        var thread = new Thread(async () =>
        {
            try
            {
                await action();
                tcs.SetResult();
            }
            catch (Exception ex)
            {
                tcs.SetException(ex);
            }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return tcs.Task;
    }

    [TestMethod]
    public void DirectoryTreeViewAutomationPeer_ExposesTreeControlTypeAndSelectionProvider()
    {
        RunInSta(() =>
        {
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);

            Assert.AreEqual(AutomationControlType.Tree, treeViewPeer.GetAutomationControlType());
            Assert.AreEqual("DirectoryTreeView", treeViewPeer.GetClassName());
            Assert.AreEqual("Directory Tree", treeViewPeer.GetName());

            var selectionProvider = treeViewPeer.GetPattern(PatternInterface.Selection) as ISelectionProvider;
            Assert.IsNotNull(selectionProvider, "DirectoryTreeViewAutomationPeer must support ISelectionProvider");
        });
    }

    [TestMethod]
    public void TreeItemAutomationPeer_ExposesTreeItemControlTypeAndTruthfulName()
    {
        RunInSta(() =>
        {
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);
            var node = new TreeNodeData(1, 0, "Windows", 1, 1024 * 1024, 1024 * 1024, true, 42, true);
            var vm = new TreeItemViewModel(node, level: 0);

            var peer = new TreeItemAutomationPeer(vm, treeViewPeer);

            Assert.AreEqual(AutomationControlType.TreeItem, peer.GetAutomationControlType());
            Assert.AreEqual("TreeItem", peer.GetClassName());
            Assert.AreEqual("Windows, 1 MB, 1 MB, 42 items", peer.GetName());
        });
    }

    [TestMethod]
    public void TreeItemAutomationPeer_NameMentionsAliasesAndExternalLink_WhenPresent()
    {
        RunInSta(() =>
        {
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);
            var node = new TreeNodeData(
                1, 0, "Shared", 2,
                LogicalBytes: 4096,
                ReferencedAllocatedBytes: 8192,
                AllocatedSizeKnown: true,
                ChildCount: 0,
                HasChildren: false,
                ObservedAliasCount: 3,
                ExternalReferenceStatus: ExternalReference.ConfirmedExternal);
            var vm = new TreeItemViewModel(node, level: 0);
            var peer = new TreeItemAutomationPeer(vm, treeViewPeer);

            Assert.AreEqual("Shared, 4 KB, 8 KB, file, 3 aliases, external link", peer.GetName());
        });
    }

    [TestMethod]
    public void TreeItemAutomationPeer_IndeterminateExternalReference_HasNoExternalLinkInName_AnnouncesAliases()
    {
        RunInSta(() =>
        {
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);
            var node = new TreeNodeData(
                1, 0, "Doc.docx", 2,
                LogicalBytes: 2048,
                ReferencedAllocatedBytes: 4096,
                AllocatedSizeKnown: true,
                ChildCount: 0,
                HasChildren: false,
                ObservedAliasCount: 2,
                ExternalReferenceStatus: ExternalReference.Indeterminate);
            var vm = new TreeItemViewModel(node, level: 0);
            var peer = new TreeItemAutomationPeer(vm, treeViewPeer);

            string name = peer.GetName();
            Assert.AreEqual("Doc.docx, 2 KB, 4 KB, file, 2 aliases", name);
            Assert.IsFalse(name.Contains("external link", StringComparison.Ordinal));
            Assert.IsFalse(vm.HasExternalLinkBadge);
            Assert.AreEqual(string.Empty, vm.ExternalLinkBadgeText);
        });
    }

    [TestMethod]
    public void TreeItemAutomationPeer_ExpandCollapseState_ReflectsViewModel()
    {
        RunInSta(() =>
        {
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);
            var node = new TreeNodeData(1, 0, "Windows", 1, 1024, 1024, true, 5, true);
            var vm = new TreeItemViewModel(node, level: 0);
            var peer = new TreeItemAutomationPeer(vm, treeViewPeer);

            var expandCollapseProvider = peer.GetPattern(PatternInterface.ExpandCollapse) as IExpandCollapseProvider;
            Assert.IsNotNull(expandCollapseProvider);

            // Initially collapsed
            Assert.AreEqual(ExpandCollapseState.Collapsed, expandCollapseProvider.ExpandCollapseState);

            // Set expanded in VM
            vm.IsExpanded = true;
            Assert.AreEqual(ExpandCollapseState.Expanded, expandCollapseProvider.ExpandCollapseState);
        });
    }

    [TestMethod]
    public void TreeItemAutomationPeer_LeafNode_ReturnsLeafStateOrNullPattern()
    {
        RunInSta(() =>
        {
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);
            var fileNode = new TreeNodeData(2, 1, "test.txt", 2, 500, 500, true, 0, false);
            var vm = new TreeItemViewModel(fileNode, level: 1);
            var peer = new TreeItemAutomationPeer(vm, treeViewPeer);

            var expandCollapseProvider = peer.GetPattern(PatternInterface.ExpandCollapse) as IExpandCollapseProvider;
            if (expandCollapseProvider != null)
            {
                Assert.AreEqual(ExpandCollapseState.LeafNode, expandCollapseProvider.ExpandCollapseState);
            }
        });
    }

    [TestMethod]
    public void TreeItemAutomationPeer_SelectionItem_HasValidSelectionContainer()
    {
        RunInSta(() =>
        {
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);
            var node = new TreeNodeData(1, 0, "Windows", 1, 1024, 1024, true, 5, true);
            var vm = new TreeItemViewModel(node, level: 0);
            var peer = new TreeItemAutomationPeer(vm, treeViewPeer);

            var selectionItem = peer.GetPattern(PatternInterface.SelectionItem) as ISelectionItemProvider;
            Assert.IsNotNull(selectionItem, "TreeItemAutomationPeer must support SelectionItem pattern");
            Assert.IsNotNull(selectionItem.SelectionContainer, "SelectionContainer must be valid non-null");
        });
    }

    [TestMethod]
    public async Task TreeItemAutomationPeer_ExpandAndCollapse_DispatchesWithoutBlocking()
    {
        await RunInStaAsync(async () =>
        {
            var provider = new FakeTreePageProvider();
            var projection = new FlattenedTreeProjection(provider);
            var treeView = new DirectoryTreeView();
            var treeViewPeer = new DirectoryTreeViewAutomationPeer(treeView);

            var root = new TreeNodeData(1, 0, @"C:\", 1, 1000, 1000, true, 1, true);
            var child = new TreeNodeData(2, 1, "SubDir", 1, 500, 500, true, 0, false);
            provider.AddPage(1, 0, 1, child);

            await projection.InitializeRootAsync("op-1", root);
            var rootVm = projection.VisibleItems[0];
            rootVm.ToggleExpandCommand = projection.ToggleExpandCommand;

            var peer = new TreeItemAutomationPeer(rootVm, treeViewPeer);
            var expandCollapse = (IExpandCollapseProvider)peer.GetPattern(PatternInterface.ExpandCollapse)!;

            // Call Expand via peer
            expandCollapse.Expand();

            // Give async task a brief moment
            await Task.Delay(50);
            Assert.IsTrue(rootVm.IsExpanded);
            Assert.AreEqual(2, projection.VisibleItems.Count);

            // Call Collapse via peer
            expandCollapse.Collapse();
            Assert.IsFalse(rootVm.IsExpanded);
            Assert.AreEqual(1, projection.VisibleItems.Count);
        });
    }
}
