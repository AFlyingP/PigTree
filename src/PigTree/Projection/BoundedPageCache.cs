using System;
using System.Collections.Generic;
using PigTree.Model;

namespace PigTree.Projection;

public sealed class BoundedPageCache
{
    private readonly int _maxPages;
    private readonly Dictionary<(string opId, uint parentId, uint offset), LinkedListNode<CacheEntry>> _map = new();
    private readonly LinkedList<CacheEntry> _lruList = new();
    private readonly object _lock = new();

    private sealed record CacheEntry(
        (string opId, uint parentId, uint offset) Key,
        PagedChildrenResult Value);

    public BoundedPageCache(int maxPages = 500)
    {
        _maxPages = Math.Max(1, maxPages);
    }

    public bool TryGetPage(string opId, uint parentId, uint offset, out PagedChildrenResult? result)
    {
        lock (_lock)
        {
            var key = (opId, parentId, offset);
            if (_map.TryGetValue(key, out var node))
            {
                _lruList.Remove(node);
                _lruList.AddLast(node);
                result = node.Value.Value;
                return true;
            }

            result = null;
            return false;
        }
    }

    public void PutPage(PagedChildrenResult result)
    {
        lock (_lock)
        {
            var key = (result.OperationId, result.ParentId, result.Offset);
            if (_map.TryGetValue(key, out var existingNode))
            {
                _lruList.Remove(existingNode);
                _map.Remove(key);
            }

            while (_map.Count >= _maxPages && _lruList.First != null)
            {
                var oldest = _lruList.First;
                _lruList.RemoveFirst();
                _map.Remove(oldest.Value.Key);
            }

            var entry = new CacheEntry(key, result);
            var newNode = _lruList.AddLast(entry);
            _map[key] = newNode;
        }
    }

    public void Clear()
    {
        lock (_lock)
        {
            _map.Clear();
            _lruList.Clear();
        }
    }
}
