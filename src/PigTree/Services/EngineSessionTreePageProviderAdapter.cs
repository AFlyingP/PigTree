using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using PigTree.Ipc;
using PigTree.Model;

namespace PigTree.Services;

public sealed class EngineSessionTreePageProviderAdapter : ITreePageProvider
{
    private readonly IEngineSession _session;

    public EngineSessionTreePageProviderAdapter(IEngineSession session)
    {
        _session = session ?? throw new ArgumentNullException(nameof(session));
    }

    public async Task<PagedChildrenResult> GetChildrenAsync(
        string operationId,
        uint parentId,
        uint offset,
        uint limit,
        CancellationToken cancellationToken = default)
    {
        var entries = await _session.GetChildrenAsync(operationId, parentId, offset, limit, cancellationToken);
        var nodes = entries.Select(e => new TreeNodeData(
            Id: e.Id,
            ParentId: e.ParentId,
            Name: e.Name,
            EntryKind: e.EntryKind,
            LogicalSize: e.LogicalSize,
            AllocatedSize: e.AllocatedSize,
            AllocatedSizeKnown: e.AllocatedSizeKnown,
            ChildCount: e.ChildCount,
            HasChildren: e.HasChildren,
            ScopeCoverage: "Complete",
            CoverageGaps: 0
        )).ToList();

        return new PagedChildrenResult(operationId, parentId, (uint)nodes.Count, offset, nodes);
    }

    public async Task<TreeNodeData?> GetRootNodeAsync(string operationId, CancellationToken cancellationToken = default)
    {
        var rootEntries = await _session.GetChildrenAsync(operationId, parentId: 0, offset: 0, limit: 1, cancellationToken);
        var root = rootEntries.FirstOrDefault();
        if (root == null)
        {
            return null;
        }

        return new TreeNodeData(
            Id: root.Id,
            ParentId: root.ParentId,
            Name: root.Name,
            EntryKind: root.EntryKind,
            LogicalSize: root.LogicalSize,
            AllocatedSize: root.AllocatedSize,
            AllocatedSizeKnown: root.AllocatedSizeKnown,
            ChildCount: root.ChildCount,
            HasChildren: root.HasChildren,
            ScopeCoverage: "Complete",
            CoverageGaps: 0
        );
    }
}
