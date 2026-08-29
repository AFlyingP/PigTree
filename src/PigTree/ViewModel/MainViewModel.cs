using System;
using System.Collections.ObjectModel;
using System.Threading;
using System.Threading.Tasks;
using PigTree.Ipc;
using PigTree.Model;
using PigTree.Projection;
using PigTree.Services;

namespace PigTree.ViewModel;

public sealed class MainViewModel : ViewModelBase, IDisposable
{
    private readonly IEngineSession _engineSession;
    private readonly ITreePageProvider _pageProvider;
    private readonly IFileSystemService _fileSystem;
    private readonly IFolderPickerService _folderPicker;
    private readonly IUiTimerFactory _timerFactory;
    private readonly IUiDispatcher _dispatcher;
    private readonly ProgressCoalescer _coalescer;

    private string _targetPath = string.Empty;
    private ScanState _state = ScanState.Idle;
    private string _statusText = "Ready";
    private string _currentDirectory = string.Empty;
    private string _elapsedText = "00:00.0";
    private string _observedFilesText = "0";
    private string _observedDirectoriesText = "0";
    private string _observedLogicalSizeText = "0 B";
    private string _observedAllocatedSizeText = "0 B";
    private uint _coverageGapsCount;
    private bool _hasError;
    private string _errorMessage = string.Empty;
    private string _errorDetails = string.Empty;
    private string _currentOperationId = string.Empty;

    private CancellationTokenSource? _scanCts;
    private int _disposed;

    public FlattenedTreeProjection Projection { get; }
    public ObservableCollection<TreeItemViewModel> VisibleTreeItems => Projection.VisibleItems;

    public TreeItemViewModel? SelectedTreeItem
    {
        get => Projection.SelectedItem;
        set
        {
            Projection.SelectedItem = value;
            OnPropertyChanged();
        }
    }

    public string TargetPath
    {
        get => _targetPath;
        set
        {
            if (SetProperty(ref _targetPath, value))
            {
                OnPropertyChanged(nameof(CanScan));
                ScanCommand.RaiseCanExecuteChanged();
            }
        }
    }

    public ScanState State
    {
        get => _state;
        private set
        {
            if (SetProperty(ref _state, value))
            {
                OnPropertyChanged(nameof(IsScanning));
                OnPropertyChanged(nameof(CanScan));
                OnPropertyChanged(nameof(CanCancel));
                OnPropertyChanged(nameof(CanBrowse));
                OnPropertyChanged(nameof(IsCancelVisible));
                OnPropertyChanged(nameof(IsScanVisible));
                ScanCommand.RaiseCanExecuteChanged();
                CancelCommand.RaiseCanExecuteChanged();
                BrowseCommand.RaiseCanExecuteChanged();
            }
        }
    }

    public string StatusText
    {
        get => _statusText;
        set => SetProperty(ref _statusText, value);
    }

    public string CurrentDirectory
    {
        get => _currentDirectory;
        set => SetProperty(ref _currentDirectory, value);
    }

    public string ElapsedText
    {
        get => _elapsedText;
        set => SetProperty(ref _elapsedText, value);
    }

    public string ObservedFilesText
    {
        get => _observedFilesText;
        set => SetProperty(ref _observedFilesText, value);
    }

    public string ObservedDirectoriesText
    {
        get => _observedDirectoriesText;
        set => SetProperty(ref _observedDirectoriesText, value);
    }

    public string ObservedLogicalSizeText
    {
        get => _observedLogicalSizeText;
        set => SetProperty(ref _observedLogicalSizeText, value);
    }

    public string ObservedAllocatedSizeText
    {
        get => _observedAllocatedSizeText;
        set => SetProperty(ref _observedAllocatedSizeText, value);
    }

    public uint CoverageGapsCount
    {
        get => _coverageGapsCount;
        set => SetProperty(ref _coverageGapsCount, value);
    }

    public bool HasError
    {
        get => _hasError;
        set => SetProperty(ref _hasError, value);
    }

    public string ErrorMessage
    {
        get => _errorMessage;
        set => SetProperty(ref _errorMessage, value);
    }

    public string ErrorDetails
    {
        get => _errorDetails;
        set => SetProperty(ref _errorDetails, value);
    }

    public bool IsScanning => State == ScanState.Starting || State == ScanState.Scanning || State == ScanState.Cancelling;
    public bool CanScan => !IsScanning && !string.IsNullOrWhiteSpace(TargetPath);
    public bool CanCancel => State == ScanState.Starting || State == ScanState.Scanning;
    public bool CanBrowse => !IsScanning;
    public bool IsCancelVisible => State == ScanState.Starting || State == ScanState.Scanning || State == ScanState.Cancelling;
    public bool IsScanVisible => !IsCancelVisible;

    public AsyncRelayCommand ScanCommand { get; }
    public AsyncRelayCommand CancelCommand { get; }
    public RelayCommand BrowseCommand { get; }
    public RelayCommand DismissErrorCommand { get; }
    public RelayCommand RetryCommand { get; }

    public MainViewModel(
        IEngineSession engineSession,
        ITreePageProvider pageProvider,
        IFileSystemService fileSystem,
        IFolderPickerService folderPicker,
        IUiTimerFactory timerFactory,
        IUiDispatcher dispatcher)
    {
        _engineSession = engineSession ?? throw new ArgumentNullException(nameof(engineSession));
        _pageProvider = pageProvider ?? throw new ArgumentNullException(nameof(pageProvider));
        _fileSystem = fileSystem ?? throw new ArgumentNullException(nameof(fileSystem));
        _folderPicker = folderPicker ?? throw new ArgumentNullException(nameof(folderPicker));
        _timerFactory = timerFactory ?? throw new ArgumentNullException(nameof(timerFactory));
        _dispatcher = dispatcher ?? throw new ArgumentNullException(nameof(dispatcher));

        Projection = new FlattenedTreeProjection(_pageProvider);
        Projection.ErrorReporter = msg =>
        {
            HasError = true;
            ErrorMessage = msg;
            StatusText = "Expansion failed";
        };

        _coalescer = new ProgressCoalescer(
            _timerFactory,
            _dispatcher,
            OnProgressUpdate,
            TimeSpan.FromMilliseconds(33));

        ScanCommand = new AsyncRelayCommand(ExecuteScanAsync, () => CanScan);
        CancelCommand = new AsyncRelayCommand(ExecuteCancelAsync, () => CanCancel);
        BrowseCommand = new RelayCommand(ExecuteBrowse, () => CanBrowse);
        DismissErrorCommand = new RelayCommand(DismissError);
        RetryCommand = new RelayCommand(() =>
        {
            DismissError();
            if (CanScan)
            {
                ScanCommand.Execute(null);
            }
        });
    }

    private void ExecuteBrowse()
    {
        string? picked = _folderPicker.PickFolder(string.IsNullOrWhiteSpace(TargetPath) ? null : TargetPath);
        if (!string.IsNullOrWhiteSpace(picked))
        {
            TargetPath = picked;
        }
    }

    private async Task ExecuteScanAsync()
    {
        string path = TargetPath?.Trim() ?? string.Empty;
        if (!_fileSystem.DirectoryExists(path))
        {
            State = ScanState.Failed;
            HasError = true;
            ErrorMessage = $"The path '{path}' is invalid, does not exist, or is not accessible.";
            StatusText = "Scan failed";
            return;
        }

        HasError = false;
        ErrorMessage = string.Empty;
        ErrorDetails = string.Empty;
        _currentOperationId = Guid.NewGuid().ToString("N");

        State = ScanState.Starting;
        StatusText = "Starting scan...";

        // Dispose previous CancellationTokenSource safely
        var newCts = new CancellationTokenSource();
        var oldCts = Interlocked.Exchange(ref _scanCts, newCts);
        if (oldCts != null)
        {
            try { oldCts.Cancel(); } catch { }
            try { oldCts.Dispose(); } catch { }
        }
        var ct = newCts.Token;

        _coalescer.Start();
        State = ScanState.Scanning;
        StatusText = "Scanning...";

        try
        {
            var result = await _engineSession.StartScanAsync(path, _coalescer, ct);
            _coalescer.Flush();

            if (result.Outcome == ScanOutcome.Cancelled || ct.IsCancellationRequested)
            {
                await LoadRootAndFinishAsync(result.OperationId, ScanState.Cancelled, "Scan cancelled - partial results displayed");
            }
            else if (result.Outcome == ScanOutcome.Finished && result.IsSuccess)
            {
                await LoadRootAndFinishAsync(result.OperationId, ScanState.Completed, $"Scan completed in {result.DurationMs}ms");
            }
            else
            {
                State = ScanState.Failed;
                HasError = true;
                ErrorMessage = result.ErrorMessage ?? "Scan failed with an unknown error.";
                StatusText = "Scan failed";
            }
        }
        catch (OperationCanceledException)
        {
            _coalescer.Flush();
            await LoadRootAndFinishAsync(_currentOperationId, ScanState.Cancelled, "Scan cancelled - partial results displayed");
        }
        catch (Exception ex)
        {
            _coalescer.Flush();
            State = ScanState.Failed;
            HasError = true;
            ErrorMessage = ex.Message;
            StatusText = "Scan failed";
        }
        finally
        {
            _coalescer.Stop();
        }
    }

    private async Task LoadRootAndFinishAsync(string opId, ScanState finalState, string finalStatus)
    {
        try
        {
            var root = await _pageProvider.GetRootNodeAsync(opId);
            if (root != null)
            {
                await Projection.InitializeRootAsync(opId, root);
                if (Projection.VisibleItems.Count > 0)
                {
                    try
                    {
                        await Projection.ExpandAsync(Projection.VisibleItems[0]);
                    }
                    catch (Exception ex)
                    {
                        HasError = true;
                        ErrorMessage = $"Failed to expand root children: {ex.Message}";
                    }
                }
            }

            State = finalState;
            StatusText = finalStatus;
        }
        catch (OperationCanceledException)
        {
            State = ScanState.Cancelled;
            StatusText = "Scan cancelled";
        }
        catch (Exception ex)
        {
            State = ScanState.Failed;
            HasError = true;
            ErrorMessage = $"Failed to load partial results: {ex.Message}";
            StatusText = "Scan failed";
        }
    }

    private async Task ExecuteCancelAsync()
    {
        if (State != ScanState.Starting && State != ScanState.Scanning)
        {
            return;
        }

        State = ScanState.Cancelling;
        StatusText = "Cancelling scan...";

        try
        {
            _scanCts?.Cancel();
            await _engineSession.CancelAsync(_currentOperationId, "User requested cancellation");
        }
        catch
        {
            // cancellation best effort
        }
    }

    private void DismissError()
    {
        HasError = false;
        ErrorMessage = string.Empty;
        ErrorDetails = string.Empty;
        if (State == ScanState.Failed)
        {
            State = ScanState.Idle;
            StatusText = "Ready";
        }
    }

    public void SetErrorForTesting(string message)
    {
        HasError = true;
        ErrorMessage = message;
        State = ScanState.Failed;
        StatusText = "Scan failed";
    }

    public void ReportEngineInitializationFailure(string error)
    {
        HasError = true;
        ErrorMessage = $"Failed to initialize engine backend: {error}";
        State = ScanState.Failed;
        StatusText = "Engine initialization failed";
    }

    private void OnProgressUpdate(CoalescedProgressState state)
    {
        ElapsedText = state.FormattedElapsed;
        ObservedFilesText = state.FormattedFiles;
        ObservedDirectoriesText = state.FormattedDirectories;
        ObservedLogicalSizeText = state.FormattedLogicalBytes;
        ObservedAllocatedSizeText = state.FormattedAllocatedBytes;
        CoverageGapsCount = state.CoverageGaps;
        CurrentDirectory = state.CurrentDirectory;
        StatusText = $"Scanning ({state.FormattedFiles} files, {state.FormattedLogicalBytes})...";
    }

    public void Dispose()
    {
        if (Interlocked.Exchange(ref _disposed, 1) == 0)
        {
            _coalescer.Dispose();
            var cts = Interlocked.Exchange(ref _scanCts, null);
            if (cts != null)
            {
                try { cts.Cancel(); } catch { }
                try { cts.Dispose(); } catch { }
            }
        }
    }
}
