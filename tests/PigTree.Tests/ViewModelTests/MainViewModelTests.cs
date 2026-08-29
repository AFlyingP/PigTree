using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Ipc;
using PigTree.Model;
using PigTree.Projection;
using PigTree.Services;
using PigTree.Tests.CoalescerTests;
using PigTree.Tests.ProjectionTests;
using PigTree.Tests.ServiceTests;
using PigTree.ViewModel;

namespace PigTree.Tests.ViewModelTests;

public class FakeEngineSession : IEngineSession
{
    public bool IsConnected => true;
    public string SessionId => "fake-session-1";
    public uint EnginePid => 9999;

    public Func<string, IProgress<ScanProgressReport>?, CancellationToken, Task<ScanResult>>? StartScanHandler { get; set; }
    public Func<string, string, CancellationToken, Task>? CancelHandler { get; set; }

    public Task<ScanResult> StartScanAsync(string targetPath, IProgress<ScanProgressReport>? progress = null, CancellationToken cancellationToken = default)
    {
        if (StartScanHandler != null)
        {
            return StartScanHandler(targetPath, progress, cancellationToken);
        }

        return Task.FromResult(new ScanResult
        {
            OperationId = "op-1",
            TargetPath = targetPath,
            Outcome = ScanOutcome.Finished,
            DirectoryCount = 10,
            FileCount = 50,
            LogicalBytes = 1024 * 1024,
            AllocatedBytes = 1024 * 1024,
            DurationMs = 150
        });
    }

    public Task<IReadOnlyList<DirectoryEntryInfo>> GetChildrenAsync(string operationId, uint parentId, uint offset = 0, uint limit = 100, CancellationToken cancellationToken = default)
    {
        return Task.FromResult<IReadOnlyList<DirectoryEntryInfo>>(Array.Empty<DirectoryEntryInfo>());
    }

    public Task CancelAsync(string operationId, string reason = "User requested cancellation", CancellationToken cancellationToken = default)
    {
        CancelHandler?.Invoke(operationId, reason, cancellationToken);
        return Task.CompletedTask;
    }

    public Task ShutdownAsync(CancellationToken cancellationToken = default) => Task.CompletedTask;
    public ValueTask DisposeAsync() => ValueTask.CompletedTask;
    public void Dispose() { }
}

public class FakeFolderPicker : IFolderPickerService
{
    public string? SelectedFolder { get; set; }
    public string? PickFolder(string? initialDirectory = null) => SelectedFolder;
}

[TestClass]
public class MainViewModelTests
{
    private FakeEngineSession _engineSession = null!;
    private FakeTreePageProvider _pageProvider = null!;
    private FakeFileSystemService _fileSystem = null!;
    private FakeFolderPicker _folderPicker = null!;
    private FakeTimerFactory _timerFactory = null!;
    private ImmediateDispatcher _dispatcher = null!;
    private MainViewModel _viewModel = null!;

    [TestInitialize]
    public void Setup()
    {
        _engineSession = new FakeEngineSession();
        _pageProvider = new FakeTreePageProvider();
        _fileSystem = new FakeFileSystemService();
        _folderPicker = new FakeFolderPicker();
        _timerFactory = new FakeTimerFactory();
        _dispatcher = new ImmediateDispatcher();

        _viewModel = new MainViewModel(
            _engineSession,
            _pageProvider,
            _fileSystem,
            _folderPicker,
            _timerFactory,
            _dispatcher);
    }

    [TestMethod]
    public void InitialState_IsIdleWithCleanProperties()
    {
        Assert.AreEqual(ScanState.Idle, _viewModel.State);
        Assert.IsFalse(_viewModel.HasError);
        Assert.IsFalse(_viewModel.IsCancelVisible);
        Assert.IsTrue(_viewModel.IsScanVisible);
        Assert.IsFalse(_viewModel.CanScan); // TargetPath is empty initially
    }

    [TestMethod]
    public async Task Scan_WithInvalidPath_EntersFailedStateAndShowsErrorBanner()
    {
        _viewModel.TargetPath = @"C:\NonExistentFolder";
        // Do not add to _fileSystem

        await _viewModel.ScanCommand.ExecuteAsync(null);

        Assert.AreEqual(ScanState.Failed, _viewModel.State);
        Assert.IsTrue(_viewModel.HasError);
        StringAssert.Contains(_viewModel.ErrorMessage, "NonExistentFolder");
        Assert.AreEqual(0, _viewModel.VisibleTreeItems.Count);
    }

    [TestMethod]
    public async Task Scan_SuccessfulRun_LoadsRootAndTransitionsToCompleted()
    {
        _fileSystem.AddDirectory(@"C:\Target");
        _viewModel.TargetPath = @"C:\Target";

        var root = new TreeNodeData(1, 0, @"C:\Target", 1, 1024, 1024, true, 2, true);
        var child1 = new TreeNodeData(2, 1, "Child1", 1, 512, 512, true, 0, false);
        _pageProvider.SetRoot(root);
        _pageProvider.AddPage(1, 0, 1, child1);

        await _viewModel.ScanCommand.ExecuteAsync(null);

        Assert.AreEqual(ScanState.Completed, _viewModel.State);
        Assert.IsFalse(_viewModel.HasError);
        Assert.AreEqual(2, _viewModel.VisibleTreeItems.Count);
        Assert.AreEqual(@"C:\Target", _viewModel.VisibleTreeItems[0].Name);
        Assert.AreEqual("Child1", _viewModel.VisibleTreeItems[1].Name);
    }

    [TestMethod]
    public async Task Cancel_DuringScan_PreservesPartialResultsAndSetsCancelledState()
    {
        _fileSystem.AddDirectory(@"C:\Target");
        _viewModel.TargetPath = @"C:\Target";

        var root = new TreeNodeData(1, 0, @"C:\Target", 1, 1024, 1024, true, 1, true);
        var child1 = new TreeNodeData(2, 1, "PartialChild", 1, 512, 512, true, 0, false);
        _pageProvider.SetRoot(root);
        _pageProvider.AddPage(1, 0, 1, child1);

        _engineSession.StartScanHandler = async (path, progress, ct) =>
        {
            // Simulate cancellation while running
            await _viewModel.CancelCommand.ExecuteAsync(null);
            return new ScanResult
            {
                OperationId = "op-cancelled",
                TargetPath = path,
                Outcome = ScanOutcome.Cancelled,
                DirectoryCount = 1,
                FileCount = 1,
                LogicalBytes = 512,
                AllocatedBytes = 512
            };
        };

        await _viewModel.ScanCommand.ExecuteAsync(null);

        Assert.AreEqual(ScanState.Cancelled, _viewModel.State);
        Assert.IsFalse(_viewModel.HasError);
        StringAssert.Contains(_viewModel.StatusText, "cancelled");
        Assert.IsTrue(_viewModel.VisibleTreeItems.Count >= 1);
        Assert.AreEqual(@"C:\Target", _viewModel.VisibleTreeItems[0].Name);
    }

    [TestMethod]
    public async Task Cancel_WhenPartialRootLoadThrows_TransitionsToFailedWithErrorBanner()
    {
        _fileSystem.AddDirectory(@"C:\Target");
        _viewModel.TargetPath = @"C:\Target";

        _pageProvider.ThrowOnGetRoot = new InvalidOperationException("Engine pipe crashed");

        _engineSession.StartScanHandler = async (path, progress, ct) =>
        {
            await _viewModel.CancelCommand.ExecuteAsync(null);
            throw new OperationCanceledException();
        };

        await _viewModel.ScanCommand.ExecuteAsync(null);

        Assert.AreEqual(ScanState.Failed, _viewModel.State);
        Assert.IsTrue(_viewModel.HasError);
        StringAssert.Contains(_viewModel.ErrorMessage, "Engine pipe crashed");
    }

    [TestMethod]
    public void ReportEngineInitializationFailure_SetsFailedStateAndErrorMessage()
    {
        _viewModel.ReportEngineInitializationFailure("Failed to spawn pigtree-engine.exe");

        Assert.AreEqual(ScanState.Failed, _viewModel.State);
        Assert.IsTrue(_viewModel.HasError);
        StringAssert.Contains(_viewModel.ErrorMessage, "Failed to spawn pigtree-engine.exe");
    }

    [TestMethod]
    public void BrowseCommand_UpdatesTargetPathWhenFolderSelected()
    {
        _folderPicker.SelectedFolder = @"D:\SelectedFolder";
        _viewModel.BrowseCommand.Execute(null);

        Assert.AreEqual(@"D:\SelectedFolder", _viewModel.TargetPath);
    }

    [TestMethod]
    public void DismissErrorCommand_ClearsErrorBanner()
    {
        _viewModel.SetErrorForTesting("Test Error");
        Assert.IsTrue(_viewModel.HasError);

        _viewModel.DismissErrorCommand.Execute(null);
        Assert.IsFalse(_viewModel.HasError);
        Assert.AreEqual(string.Empty, _viewModel.ErrorMessage);
    }

    [TestMethod]
    public async Task RepeatedScans_SafelyCancelAndDisposePreviousCancellationTokenSource()
    {
        _fileSystem.AddDirectory(@"C:\Target1");
        _fileSystem.AddDirectory(@"C:\Target2");

        var root1 = new TreeNodeData(1, 0, @"C:\Target1", 1, 100, 100, true, 0, false);
        var root2 = new TreeNodeData(2, 0, @"C:\Target2", 1, 200, 200, true, 0, false);
        _pageProvider.SetRoot(root1);

        _viewModel.TargetPath = @"C:\Target1";
        await _viewModel.ScanCommand.ExecuteAsync(null);
        Assert.AreEqual(ScanState.Completed, _viewModel.State);

        _pageProvider.SetRoot(root2);
        _viewModel.TargetPath = @"C:\Target2";
        await _viewModel.ScanCommand.ExecuteAsync(null);
        Assert.AreEqual(ScanState.Completed, _viewModel.State);
    }

    [TestMethod]
    public void MainViewModel_Dispose_IsIdempotentAndSafe()
    {
        _viewModel.Dispose();
        _viewModel.Dispose(); // second call should not throw
    }

    [TestMethod]
    public void FallbackEngineSession_Dispose_IsIdempotent()
    {
        var fallback = new FallbackEngineSession("Init test error");
        Assert.IsFalse(fallback.IsConnected);
        fallback.Dispose();
        fallback.Dispose();
    }

    [TestMethod]
    public async Task ChildExpansionFailure_DisplaysNonModalErrorBanner()
    {
        _fileSystem.AddDirectory(@"C:\Target");
        _viewModel.TargetPath = @"C:\Target";

        var root = new TreeNodeData(1, 0, @"C:\Target", 1, 1000, 1000, true, 1, true);
        _pageProvider.SetRoot(root);
        _pageProvider.ThrowOnGetChildren = new InvalidOperationException("Corrupt page data");

        await _viewModel.ScanCommand.ExecuteAsync(null);

        // Root is loaded, but expansion fails non-modally
        Assert.AreEqual(1, _viewModel.VisibleTreeItems.Count);
        Assert.IsTrue(_viewModel.HasError);
        StringAssert.Contains(_viewModel.ErrorMessage, "Corrupt page data");
    }

    [TestMethod]
    public async Task ToggleExpandCommand_OnTreeItemViewModel_ExpandsAndCollapses()
    {
        _fileSystem.AddDirectory(@"C:\Target");
        _viewModel.TargetPath = @"C:\Target";

        var root = new TreeNodeData(1, 0, @"C:\Target", 1, 1000, 1000, true, 1, true);
        var child = new TreeNodeData(2, 1, "Child", 1, 500, 500, true, 0, false);
        _pageProvider.SetRoot(root);
        _pageProvider.AddPage(1, 0, 1, child);

        await _viewModel.ScanCommand.ExecuteAsync(null);

        Assert.AreEqual(2, _viewModel.VisibleTreeItems.Count);
        var rootVm = _viewModel.VisibleTreeItems[0];
        Assert.IsTrue(rootVm.IsExpanded);
        Assert.IsNotNull(rootVm.ToggleExpandCommand);

        // Toggle to collapse
        rootVm.ToggleExpandCommand.Execute(rootVm);
        Assert.IsFalse(rootVm.IsExpanded);
        Assert.AreEqual(1, _viewModel.VisibleTreeItems.Count);

        // Toggle to expand
        rootVm.ToggleExpandCommand.Execute(rootVm);
        Assert.IsTrue(rootVm.IsExpanded);
        Assert.AreEqual(2, _viewModel.VisibleTreeItems.Count);
    }
}
