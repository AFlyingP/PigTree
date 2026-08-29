using System;
using Microsoft.Win32;

namespace PigTree.Services;

public sealed class StandardFolderPickerService : IFolderPickerService
{
    public string? PickFolder(string? initialDirectory = null)
    {
        var dialog = new OpenFolderDialog
        {
            Title = "Select Folder to Analyze",
            Multiselect = false
        };

        if (!string.IsNullOrWhiteSpace(initialDirectory))
        {
            dialog.InitialDirectory = initialDirectory;
        }

        bool? result = dialog.ShowDialog();
        return result == true ? dialog.FolderName : null;
    }
}
