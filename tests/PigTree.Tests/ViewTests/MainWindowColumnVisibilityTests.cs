using System;
using System.Linq;
using System.Windows.Controls;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree;
using PigTree.Controls;
using PigTree.Tests.CoalescerTests;
using PigTree.Tests.ProjectionTests;
using PigTree.Tests.ServiceTests;
using PigTree.Tests.ViewModelTests;
using PigTree.ViewModel;

namespace PigTree.Tests.ViewTests;

[TestClass]
public class MainWindowColumnVisibilityTests
{
    private static MainViewModel CreateTestViewModel()
    {
        return new MainViewModel(
            new FakeEngineSession(),
            new FakeTreePageProvider(),
            new FakeFileSystemService(),
            new FakeFolderPicker(),
            new FakeTimerFactory(),
            new ImmediateDispatcher());
    }

    [TestMethod]
    public void UniqueAllocatedColumn_HiddenByDefault_ShowsWhenEnabled_HidesAgain()
    {
        PersistentDispatcherHost.Run(() =>
        {
            var viewModel = CreateTestViewModel();
            MainWindow? window = null;

            try
            {
                window = new MainWindow(viewModel);
                window.Show();
                PersistentDispatcherHost.Drain();

                var treeView = (DirectoryTreeView)window.FindName("DirectoryTreeView");
                Assert.IsNotNull(treeView, "DirectoryTreeView must be found in MainWindow.");

                var gridView = (GridView)treeView.View;
                var uniqueColumn = gridView.Columns.FirstOrDefault(c => (string)c.Header == "Unique Allocated");
                Assert.IsNotNull(uniqueColumn, "Unique Allocated column should exist in GridView.");

                // 1. Proving actual Unique Allocated GridViewColumn is hidden by default
                Assert.AreEqual(0.0, uniqueColumn.Width, "Unique Allocated column must be hidden (width 0) by default.");

                // 2. Becomes visible when ShowUniqueAllocatedColumn = true
                viewModel.ShowUniqueAllocatedColumn = true;
                PersistentDispatcherHost.Drain();
                Assert.AreEqual(130.0, uniqueColumn.Width, "Unique Allocated column must become visible (width 130) when ShowUniqueAllocatedColumn is true.");

                // 3. Hides again when ShowUniqueAllocatedColumn = false
                viewModel.ShowUniqueAllocatedColumn = false;
                PersistentDispatcherHost.Drain();
                Assert.AreEqual(0.0, uniqueColumn.Width, "Unique Allocated column must hide again (width 0) when ShowUniqueAllocatedColumn is set back to false.");
            }
            finally
            {
                window?.Close();
                PersistentDispatcherHost.Drain();
            }
        });
    }

    [TestMethod]
    public void UniqueAllocatedColumn_DataContextChangedAfterConstruction_StillUpdatesColumnWidth()
    {
        PersistentDispatcherHost.Run(() =>
        {
            var initialVm = CreateTestViewModel();
            MainWindow? window = null;

            try
            {
                window = new MainWindow(initialVm);
                window.Show();
                PersistentDispatcherHost.Drain();

                var treeView = (DirectoryTreeView)window.FindName("DirectoryTreeView");
                var gridView = (GridView)treeView.View;
                var uniqueColumn = gridView.Columns.FirstOrDefault(c => (string)c.Header == "Unique Allocated");
                Assert.IsNotNull(uniqueColumn);
                Assert.AreEqual(0.0, uniqueColumn.Width);

                // Reassign DataContext after construction to prove declarative XAML binding remains active
                var newVm = CreateTestViewModel();
                window.DataContext = newVm;
                PersistentDispatcherHost.Drain();

                // Initial state under new VM
                Assert.AreEqual(0.0, uniqueColumn.Width);

                // Toggle on new VM
                newVm.ShowUniqueAllocatedColumn = true;
                PersistentDispatcherHost.Drain();
                Assert.AreEqual(130.0, uniqueColumn.Width, "Column width should update when toggled on the new DataContext.");

                // Toggle off new VM
                newVm.ShowUniqueAllocatedColumn = false;
                PersistentDispatcherHost.Drain();
                Assert.AreEqual(0.0, uniqueColumn.Width, "Column width should update when toggled off on the new DataContext.");
            }
            finally
            {
                window?.Close();
                PersistentDispatcherHost.Drain();
            }
        });
    }

    [TestMethod]
    public void UniqueAllocatedColumn_UserResizeInteraction_TestedWhenVisible()
    {
        PersistentDispatcherHost.Run(() =>
        {
            var viewModel = CreateTestViewModel();
            MainWindow? window = null;

            try
            {
                window = new MainWindow(viewModel);
                window.Show();
                PersistentDispatcherHost.Drain();

                var treeView = (DirectoryTreeView)window.FindName("DirectoryTreeView");
                var gridView = (GridView)treeView.View;
                var uniqueColumn = gridView.Columns.FirstOrDefault(c => (string)c.Header == "Unique Allocated");
                Assert.IsNotNull(uniqueColumn);

                // Show column
                viewModel.ShowUniqueAllocatedColumn = true;
                PersistentDispatcherHost.Drain();
                Assert.AreEqual(130.0, uniqueColumn.Width);

                // User drag-resizes column to 180.0
                uniqueColumn.Width = 180.0;
                PersistentDispatcherHost.Drain();
                Assert.AreEqual(180.0, uniqueColumn.Width, "User resize updates column width while visible.");
            }
            finally
            {
                window?.Close();
                PersistentDispatcherHost.Drain();
            }
        });
    }
}
