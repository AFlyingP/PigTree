using System;
using System.Collections.Generic;

namespace PigTree.Services;

public sealed class DragDropValidator
{
    private readonly IFileSystemService _fileSystem;

    public DragDropValidator(IFileSystemService fileSystem)
    {
        _fileSystem = fileSystem ?? throw new ArgumentNullException(nameof(fileSystem));
    }

    public bool TryValidateDrop(IReadOnlyList<string>? paths, out string? validDirectoryPath)
    {
        validDirectoryPath = null;
        if (paths == null || paths.Count != 1)
        {
            return false;
        }

        string candidate = paths[0];
        if (string.IsNullOrWhiteSpace(candidate))
        {
            return false;
        }

        if (_fileSystem.DirectoryExists(candidate))
        {
            validDirectoryPath = candidate;
            return true;
        }

        return false;
    }
}
