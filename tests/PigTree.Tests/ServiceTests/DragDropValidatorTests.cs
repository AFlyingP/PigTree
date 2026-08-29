using System;
using System.Collections.Generic;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Services;

namespace PigTree.Tests.ServiceTests;

public class FakeFileSystemService : IFileSystemService
{
    private readonly HashSet<string> _directories = new(StringComparer.OrdinalIgnoreCase);

    public void AddDirectory(string path) => _directories.Add(path);

    public bool DirectoryExists(string path) => _directories.Contains(path);
}

[TestClass]
public class DragDropValidatorTests
{
    private FakeFileSystemService _fs = null!;
    private DragDropValidator _validator = null!;

    [TestInitialize]
    public void Setup()
    {
        _fs = new FakeFileSystemService();
        _validator = new DragDropValidator(_fs);
    }

    [TestMethod]
    public void TryGetSingleDirectory_WithValidDirectory_ReturnsTrueAndPath()
    {
        _fs.AddDirectory("C:\\TestDirectory");

        bool success = _validator.TryValidateDrop(new[] { "C:\\TestDirectory" }, out string? path);

        Assert.IsTrue(success);
        Assert.AreEqual("C:\\TestDirectory", path);
    }

    [TestMethod]
    public void TryGetSingleDirectory_WithMultiplePaths_ReturnsFalse()
    {
        _fs.AddDirectory("C:\\Dir1");
        _fs.AddDirectory("C:\\Dir2");

        bool success = _validator.TryValidateDrop(new[] { "C:\\Dir1", "C:\\Dir2" }, out string? path);

        Assert.IsFalse(success);
        Assert.IsNull(path);
    }

    [TestMethod]
    public void TryGetSingleDirectory_WithNonExistentPathOrFile_ReturnsFalse()
    {
        bool success = _validator.TryValidateDrop(new[] { "C:\\NonExistent" }, out string? path);

        Assert.IsFalse(success);
        Assert.IsNull(path);
    }

    [TestMethod]
    public void TryGetSingleDirectory_WithEmptyArray_ReturnsFalse()
    {
        bool success = _validator.TryValidateDrop(Array.Empty<string>(), out string? path);

        Assert.IsFalse(success);
        Assert.IsNull(path);
    }
}
