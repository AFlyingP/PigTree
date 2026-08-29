using System.Windows;
using PigTree.Services;
using PigTree.ViewModel;

namespace PigTree;

public partial class MainWindow : Window
{
    private readonly DragDropValidator _dragDropValidator;

    public MainWindow(MainViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;
        _dragDropValidator = new DragDropValidator(new DefaultFileSystemService());
    }

    private void OnWindowDragOver(object sender, DragEventArgs e)
    {
        if (e.Data.GetDataPresent(DataFormats.FileDrop))
        {
            if (e.Data.GetData(DataFormats.FileDrop) is string[] files &&
                _dragDropValidator.TryValidateDrop(files, out _))
            {
                e.Effects = DragDropEffects.Copy;
                e.Handled = true;
                return;
            }
        }

        e.Effects = DragDropEffects.None;
        e.Handled = true;
    }

    private void OnWindowDrop(object sender, DragEventArgs e)
    {
        if (e.Data.GetDataPresent(DataFormats.FileDrop))
        {
            if (e.Data.GetData(DataFormats.FileDrop) is string[] files &&
                _dragDropValidator.TryValidateDrop(files, out string? validDir) &&
                validDir != null)
            {
                if (DataContext is MainViewModel vm)
                {
                    vm.TargetPath = validDir;
                }
                e.Handled = true;
            }
        }
    }
}
