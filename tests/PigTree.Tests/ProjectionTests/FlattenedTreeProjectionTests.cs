using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Model;
using PigTree.Projection;
using PigTree.Services;
using PigTree.ViewModel;

namespace PigTree.Tests.ProjectionTests;

public class FakeTreePageProvider : ITreePageProvider
{
    private readonly Dictionary<(uint parentId, uint offset), PagedChildrenResult> _pages = new();
    private TreeNodeData? _rootNode;
    public Exception? ThrowOnGetRoot { get; set; }
    public Exception? ThrowOnGetChildren { get; set; }

    public void SetRoot(TreeNodeData root) => _rootNode = root;

    public void AddPage(uint parentId, uint offset, uint totalChildren, params TreeNodeData[] nodes)
    {
        _pages[(parentId, offset)] = new PagedChildrenResult("op-1", parentId, totalChildren, offset, nodes);
    }

    public Task<TreeNodeData?> GetRootNodeAsync(string operationId, CancellationToken cancellationToken = default)
    {
        if (ThrowOnGetRoot != null)
        {
            throw ThrowOnGetRoot;
        }
        return Task.FromResult(_rootNode);
    }

    public Task<PagedChildrenResult> GetChildrenAsync(string operationId, uint parentId, uint offset, uint limit, CancellationToken cancellationToken = default)
    {
        if (ThrowOnGetChildren != null)
        {
            throw ThrowOnGetChildren;
        }
        if (_pages.TryGetValue((parentId, offset), out var res))
        {
            return Task.FromResult(res);
        }

        return Task.FromResult(new PagedChildrenResult(operationId, parentId, 0, offset, Array.Empty<TreeNodeData>()));
    }
}

[TestClass]
public class FlattenedTreeProjectionTests
{
    private FakeTreePageProvider _provider = null!;
    private FlattenedTreeProjection _projection = null!;

    [TestInitialize]
    public void Setup()
    {
        _provider = new FakeTreePageProvider();
        _projection = new FlattenedTreeProjection(_provider, pageSize: 500);
    }

    [TestMethod]
    public async Task InitializeRoot_PutsSingleRootAtLevelZero()
    {
        var root = new TreeNodeData(1, 0, "C:\\", 1, 1000, 1000, true, 2, true);
        await _projection.InitializeRootAsync("op-1", root);

        Assert.AreEqual(1, _projection.VisibleItems.Count);
        var rootVm = _projection.VisibleItems[0];
        Assert.AreEqual(1u, rootVm.Id);
        Assert.AreEqual("C:\\", rootVm.Name);
        Assert.AreEqual(0, rootVm.Level);
        Assert.IsFalse(rootVm.IsExpanded);
        Assert.IsTrue(rootVm.HasChildren);
    }

    [TestMethod]
    public async Task ExpandAsync_LazilyInsertsDirectChildrenOnly()
    {
        var root = new TreeNodeData(1, 0, "C:\\", 1, 1000, 1000, true, 2, true);
        var child1 = new TreeNodeData(2, 1, "Windows", 1, 600, 600, true, 5, true);
        var child2 = new TreeNodeData(3, 1, "Program Files", 1, 400, 400, true, 0, false);
        _provider.AddPage(1, 0, 2, child1, child2);

        await _projection.InitializeRootAsync("op-1", root);
        var rootVm = _projection.VisibleItems[0];

        await _projection.ExpandAsync(rootVm);

        Assert.IsTrue(rootVm.IsExpanded);
        Assert.AreEqual(3, _projection.VisibleItems.Count);
        Assert.AreEqual(1u, _projection.VisibleItems[0].Id);
        Assert.AreEqual(0, _projection.VisibleItems[0].Level);

        Assert.AreEqual(2u, _projection.VisibleItems[1].Id);
        Assert.AreEqual("Windows", _projection.VisibleItems[1].Name);
        Assert.AreEqual(1, _projection.VisibleItems[1].Level);

        Assert.AreEqual(3u, _projection.VisibleItems[2].Id);
        Assert.AreEqual("Program Files", _projection.VisibleItems[2].Name);
        Assert.AreEqual(1, _projection.VisibleItems[2].Level);
    }

    [TestMethod]
    public async Task ToggleExpandCommand_TogglesExpansionViaICommand()
    {
        var root = new TreeNodeData(1, 0, "C:\\", 1, 1000, 1000, true, 1, true);
        var child1 = new TreeNodeData(2, 1, "Windows", 1, 600, 600, true, 0, false);
        _provider.AddPage(1, 0, 1, child1);

        await _projection.InitializeRootAsync("op-1", root);
        var rootVm = _projection.VisibleItems[0];

        // Execute ToggleExpandCommand with rootVm parameter
        _projection.ToggleExpandCommand.Execute(rootVm);
        await Task.Delay(50); // allow async command to complete

        Assert.IsTrue(rootVm.IsExpanded);
        Assert.AreEqual(2, _projection.VisibleItems.Count);

        // Execute ToggleExpandCommand again to collapse
        _projection.ToggleExpandCommand.Execute(rootVm);
        Assert.IsFalse(rootVm.IsExpanded);
        Assert.AreEqual(1, _projection.VisibleItems.Count);
    }

    [TestMethod]
    public async Task Collapse_RemovesAllDescendantsAndPreservesSelection()
    {
        var root = new TreeNodeData(1, 0, "C:\\", 1, 1000, 1000, true, 1, true);
        var child1 = new TreeNodeData(2, 1, "Windows", 1, 600, 600, true, 1, true);
        var grandChild1 = new TreeNodeData(3, 2, "System32", 1, 300, 300, true, 0, false);

        _provider.AddPage(1, 0, 1, child1);
        _provider.AddPage(2, 0, 1, grandChild1);

        await _projection.InitializeRootAsync("op-1", root);
        var rootVm = _projection.VisibleItems[0];
        await _projection.ExpandAsync(rootVm);

        var childVm = _projection.VisibleItems[1];
        await _projection.ExpandAsync(childVm);

        Assert.AreEqual(3, _projection.VisibleItems.Count);

        // Select the grandChild
        _projection.SelectedItem = _projection.VisibleItems[2];
        Assert.AreEqual(3u, _projection.SelectedItem.Id);

        // Collapse root
        _projection.Collapse(rootVm);

        Assert.IsFalse(rootVm.IsExpanded);
        Assert.AreEqual(1, _projection.VisibleItems.Count);
        Assert.AreEqual(1u, _projection.VisibleItems[0].Id);

        // Selection should be preserved on root because child was removed
        Assert.AreEqual(rootVm, _projection.SelectedItem);
    }

    [TestMethod]
    public async Task KeyboardNavigation_Right_ExpandsThenSelectsFirstChild()
    {
        var root = new TreeNodeData(1, 0, "C:\\", 1, 1000, 1000, true, 2, true);
        var child1 = new TreeNodeData(2, 1, "Windows", 1, 600, 600, true, 0, false);
        var child2 = new TreeNodeData(3, 1, "Users", 1, 400, 400, true, 0, false);
        _provider.AddPage(1, 0, 2, child1, child2);

        await _projection.InitializeRootAsync("op-1", root);
        _projection.SelectedItem = _projection.VisibleItems[0];

        // 1st Right key: expands root
        await _projection.NavigateRightAsync();
        Assert.IsTrue(_projection.VisibleItems[0].IsExpanded);
        Assert.AreEqual(3, _projection.VisibleItems.Count);
        Assert.AreEqual(1u, _projection.SelectedItem.Id);

        // 2nd Right key: moves to first child
        await _projection.NavigateRightAsync();
        Assert.AreEqual(2u, _projection.SelectedItem.Id);
    }

    [TestMethod]
    public async Task KeyboardNavigation_Left_CollapsesThenSelectsParent()
    {
        var root = new TreeNodeData(1, 0, "C:\\", 1, 1000, 1000, true, 1, true);
        var child1 = new TreeNodeData(2, 1, "Windows", 1, 600, 600, true, 1, true);
        var grandChild1 = new TreeNodeData(3, 2, "System32", 1, 300, 300, true, 0, false);

        _provider.AddPage(1, 0, 1, child1);
        _provider.AddPage(2, 0, 1, grandChild1);

        await _projection.InitializeRootAsync("op-1", root);
        await _projection.ExpandAsync(_projection.VisibleItems[0]);
        await _projection.ExpandAsync(_projection.VisibleItems[1]);

        // Select grandChild
        _projection.SelectedItem = _projection.VisibleItems[2];
        Assert.AreEqual(3u, _projection.SelectedItem.Id);

        // Left key from leaf grandChild: selects its parent (child1)
        _projection.NavigateLeft();
        Assert.AreEqual(2u, _projection.SelectedItem.Id);

        // Left key from expanded child1: collapses child1
        _projection.NavigateLeft();
        Assert.IsFalse(_projection.VisibleItems[1].IsExpanded);
        Assert.AreEqual(2u, _projection.SelectedItem.Id);

        // Left key from collapsed child1: selects root
        _projection.NavigateLeft();
        Assert.AreEqual(1u, _projection.SelectedItem.Id);
    }

    [TestMethod]
    public void BoundedPageCache_EvictsLeastRecentlyUsedWhenCapacityExceeded()
    {
        var cache = new BoundedPageCache(maxPages: 2);
        var p1 = new PagedChildrenResult("op", 1, 10, 0, Array.Empty<TreeNodeData>());
        var p2 = new PagedChildrenResult("op", 2, 10, 0, Array.Empty<TreeNodeData>());
        var p3 = new PagedChildrenResult("op", 3, 10, 0, Array.Empty<TreeNodeData>());

        cache.PutPage(p1);
        cache.PutPage(p2);
        Assert.IsTrue(cache.TryGetPage("op", 1, 0, out _));
        Assert.IsTrue(cache.TryGetPage("op", 2, 0, out _));

        // Insert 3rd page -> should evict p1 (since p2 was accessed more recently)
        cache.PutPage(p3);
        Assert.IsFalse(cache.TryGetPage("op", 1, 0, out _));
        Assert.IsTrue(cache.TryGetPage("op", 2, 0, out _));
        Assert.IsTrue(cache.TryGetPage("op", 3, 0, out _));
    }
}
