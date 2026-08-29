using System.IO;

namespace PigTree.Services;

public interface IFileSystemService
{
    bool DirectoryExists(string path);
}

public sealed class DefaultFileSystemService : IFileSystemService
{
    public bool DirectoryExists(string path)
    {
        return !string.IsNullOrWhiteSpace(path) && Directory.Exists(path);
    }
}
