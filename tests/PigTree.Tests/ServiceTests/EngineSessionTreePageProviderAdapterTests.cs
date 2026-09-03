using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Ipc;
using PigTree.Model;
using PigTree.Services;
using PigTree.Tests.ViewModelTests;

namespace PigTree.Tests.ServiceTests;

[TestClass]
public class EngineSessionTreePageProviderAdapterTests
{
    private sealed class StubEngineSession : IEngineSession
    {
        public bool IsConnected => true;
        public string SessionId => "stub-session";
        public uint EnginePid => 1234;

        public Func<string, uint, uint, uint, CancellationToken, Task<IReadOnlyList<DirectoryEntryInfo>>>? GetChildrenHandler { get; set; }

        public Task<ScanResult> StartScanAsync(string targetPath, IProgress<ScanProgressReport>? progress = null, CancellationToken cancellationToken = default)
            => Task.FromResult(new ScanResult());

        public Task<IReadOnlyList<DirectoryEntryInfo>> GetChildrenAsync(string operationId, uint parentId, uint offset = 0, uint limit = 100, CancellationToken cancellationToken = default)
        {
            if (GetChildrenHandler != null)
            {
                return GetChildrenHandler(operationId, parentId, offset, limit, cancellationToken);
            }
            return Task.FromResult<IReadOnlyList<DirectoryEntryInfo>>(Array.Empty<DirectoryEntryInfo>());
        }

        public Task CancelAsync(string operationId, string reason = "User requested cancellation", CancellationToken cancellationToken = default)
            => Task.CompletedTask;

        public Task ShutdownAsync(CancellationToken cancellationToken = default) => Task.CompletedTask;
        public ValueTask DisposeAsync() => ValueTask.CompletedTask;
        public void Dispose() { }
    }

    [TestMethod]
    public async Task GetChildrenAsync_MapsDirectoryEntryInfoToTreeNodeDataCorrectly()
    {
        var stubSession = new StubEngineSession
        {
            GetChildrenHandler = (opId, parentId, offset, limit, ct) =>
            {
                var list = new List<DirectoryEntryInfo>
                {
                    new DirectoryEntryInfo
                    {
                        Id = 10,
                        ParentId = 1,
                        Name = "ChildDir",
                        EntryKind = 1,
                        LogicalBytes = 2048,
                        ReferencedAllocatedBytes = 4096,
                        AllocatedSizeKnown = true,
                        ChildCount = 5,
                        HasChildren = true
                    },
                    new DirectoryEntryInfo
                    {
                        Id = 11,
                        ParentId = 1,
                        Name = "file.txt",
                        EntryKind = 2,
                        LogicalBytes = 1024,
                        ReferencedAllocatedBytes = 1024,
                        AllocatedSizeKnown = true,
                        ChildCount = 0,
                        HasChildren = false
                    }
                };
                return Task.FromResult<IReadOnlyList<DirectoryEntryInfo>>(list);
            }
        };

        var adapter = new EngineSessionTreePageProviderAdapter(stubSession);
        var result = await adapter.GetChildrenAsync("op-test", 1, 0, 50);

        Assert.AreEqual("op-test", result.OperationId);
        Assert.AreEqual(1u, result.ParentId);
        Assert.AreEqual(2u, result.TotalChildren);
        Assert.AreEqual(0u, result.Offset);
        Assert.AreEqual(2, result.Nodes.Count);

        var node0 = result.Nodes[0];
        Assert.AreEqual(10u, node0.Id);
        Assert.AreEqual(1u, node0.ParentId);
        Assert.AreEqual("ChildDir", node0.Name);
        Assert.AreEqual(1u, node0.EntryKind);
        Assert.AreEqual(2048UL, node0.LogicalBytes);
        Assert.AreEqual(4096UL, node0.ReferencedAllocatedBytes);
        Assert.IsTrue(node0.AllocatedSizeKnown);
        Assert.AreEqual(5u, node0.ChildCount);
        Assert.IsTrue(node0.HasChildren);
        Assert.AreEqual("Complete", node0.ScopeCoverage);
        Assert.AreEqual(0u, node0.CoverageGaps);

        var node1 = result.Nodes[1];
        Assert.AreEqual(11u, node1.Id);
        Assert.AreEqual(1u, node1.ParentId);
        Assert.AreEqual("file.txt", node1.Name);
        Assert.AreEqual(2u, node1.EntryKind);
        Assert.IsFalse(node1.HasChildren);
    }

    [TestMethod]
    public async Task GetRootNodeAsync_WhenRootExists_MapsToTreeNodeData()
    {
        var stubSession = new StubEngineSession
        {
            GetChildrenHandler = (opId, parentId, offset, limit, ct) =>
            {
                var list = new List<DirectoryEntryInfo>
                {
                    new DirectoryEntryInfo
                    {
                        Id = 1,
                        ParentId = 0,
                        Name = @"C:\Root",
                        EntryKind = 1,
                        LogicalBytes = 10000,
                        ReferencedAllocatedBytes = 12000,
                        AllocatedSizeKnown = true,
                        ChildCount = 10,
                        HasChildren = true,
                        UniqueAllocatedBytes = 11000,
                        ObservedAliasCount = 1,
                        TotalLinkCountStatus = "known",
                        TotalLinkCountValue = 1,
                        ExternalReferenceStatus = "confirmed_none",
                        KnownSubtotalAllocatedBytes = 11000
                    }
                };
                return Task.FromResult<IReadOnlyList<DirectoryEntryInfo>>(list);
            }
        };

        var adapter = new EngineSessionTreePageProviderAdapter(stubSession);
        var root = await adapter.GetRootNodeAsync("op-test");

        Assert.IsNotNull(root);
        Assert.AreEqual(1u, root.Id);
        Assert.AreEqual(0u, root.ParentId);
        Assert.AreEqual(@"C:\Root", root.Name);
        Assert.AreEqual(1u, root.EntryKind);
        Assert.AreEqual(10000UL, root.LogicalBytes);
        Assert.AreEqual(12000UL, root.ReferencedAllocatedBytes);
        Assert.IsTrue(root.AllocatedSizeKnown);
        Assert.AreEqual(10u, root.ChildCount);
        Assert.IsTrue(root.HasChildren);

        // Issue #20 filesystem object accounting fields must flow through the adapter
        Assert.AreEqual(11000UL, root.UniqueAllocatedBytes);
        Assert.AreEqual(1u, root.ObservedAliasCount);
        Assert.AreEqual("known", root.TotalLinkCountStatus);
        Assert.AreEqual(1u, root.TotalLinkCountValue);
        Assert.AreEqual("confirmed_none", root.ExternalReferenceStatus);
        Assert.AreEqual(11000UL, root.KnownSubtotalAllocatedBytes);
    }

    [TestMethod]
    public async Task GetRootNodeAsync_WhenEmpty_ReturnsNull()
    {
        var stubSession = new StubEngineSession
        {
            GetChildrenHandler = (opId, parentId, offset, limit, ct) =>
                Task.FromResult<IReadOnlyList<DirectoryEntryInfo>>(Array.Empty<DirectoryEntryInfo>())
        };

        var adapter = new EngineSessionTreePageProviderAdapter(stubSession);
        var root = await adapter.GetRootNodeAsync("op-test");

        Assert.IsNull(root);
    }
}
