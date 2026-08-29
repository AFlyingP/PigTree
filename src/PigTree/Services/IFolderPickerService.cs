namespace PigTree.Services;

public interface IFolderPickerService
{
    string? PickFolder(string? initialDirectory = null);
}
