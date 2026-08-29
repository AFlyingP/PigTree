using System;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using PigTree.Ipc;
using PigTree.Services;
using PigTree.ViewModel;

namespace PigTree;

public partial class App : Application
{
    private IEngineSession? _engineSession;
    private MainViewModel? _mainViewModel;
    private int _disposed;

    public IEngineSession? EngineSession => _engineSession;
    public MainViewModel? MainViewModel => _mainViewModel;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        var fileSystem = new DefaultFileSystemService();
        var folderPicker = new StandardFolderPickerService();
        var timerFactory = new WpfUiTimerFactory();
        var dispatcher = new WpfUiDispatcher();

        string? initError = null;
        try
        {
            _engineSession = await EngineClientSession.LaunchAsync();
        }
        catch (Exception ex)
        {
            initError = ex.Message;
            _engineSession = new FallbackEngineSession(ex.Message);
        }

        var pageProvider = new EngineSessionTreePageProviderAdapter(_engineSession);

        _mainViewModel = new MainViewModel(
            _engineSession,
            pageProvider,
            fileSystem,
            folderPicker,
            timerFactory,
            dispatcher);

        if (!string.IsNullOrEmpty(initError))
        {
            _mainViewModel.ReportEngineInitializationFailure(initError);
        }

        var mainWindow = new MainWindow(_mainViewModel);
        mainWindow.Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        DisposeResources();
        base.OnExit(e);
    }

    public void DisposeResources()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0)
        {
            return;
        }

        try
        {
            _mainViewModel?.Dispose();
        }
        catch
        {
            // Best-effort MainViewModel disposal
        }

        if (_engineSession != null)
        {
            if (_engineSession.IsConnected)
            {
                try
                {
                    using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(500));
                    _engineSession.ShutdownAsync(cts.Token).GetAwaiter().GetResult();
                }
                catch
                {
                    // Best-effort shutdown
                }
            }

            try
            {
                _engineSession.Dispose();
            }
            catch
            {
                // Best-effort session disposal
            }
        }
    }
}

public sealed class FallbackEngineSession : IEngineSession
{
    private readonly string _initError;
    private int _disposed;

    public bool IsConnected => false;
    public string SessionId => string.Empty;
    public uint EnginePid => 0;

    public FallbackEngineSession(string initError)
    {
        _initError = initError ?? "Engine initialization failed";
    }

    public Task<Model.ScanResult> StartScanAsync(string targetPath, IProgress<Model.ScanProgressReport>? progress = null, CancellationToken cancellationToken = default)
    {
        return Task.FromResult(new Model.ScanResult
        {
            TargetPath = targetPath,
            Outcome = Model.ScanOutcome.Failed,
            ErrorMessage = $"Engine process failed to initialize: {_initError}"
        });
    }

    public Task<System.Collections.Generic.IReadOnlyList<Model.DirectoryEntryInfo>> GetChildrenAsync(string operationId, uint parentId, uint offset = 0, uint limit = 100, CancellationToken cancellationToken = default)
    {
        return Task.FromResult<System.Collections.Generic.IReadOnlyList<Model.DirectoryEntryInfo>>(Array.Empty<Model.DirectoryEntryInfo>());
    }

    public Task CancelAsync(string operationId, string reason = "User requested cancellation", CancellationToken cancellationToken = default) => Task.CompletedTask;
    public Task ShutdownAsync(CancellationToken cancellationToken = default) => Task.CompletedTask;
    public ValueTask DisposeAsync()
    {
        Dispose();
        return ValueTask.CompletedTask;
    }
    public void Dispose()
    {
        Interlocked.Exchange(ref _disposed, 1);
    }
}
