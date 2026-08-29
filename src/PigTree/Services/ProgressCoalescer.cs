using System;
using System.Diagnostics;
using PigTree.Model;

namespace PigTree.Services;

public sealed class ProgressCoalescer : IProgress<ScanProgressReport>, IDisposable
{
    private readonly IUiTimer _timer;
    private readonly IUiDispatcher _dispatcher;
    private readonly Action<CoalescedProgressState> _onUpdate;
    private readonly Stopwatch _stopwatch = new();
    private readonly object _lock = new();

    private ScanProgressReport? _latestReport;
    private bool _hasNewReport;

    public ProgressCoalescer(
        IUiTimerFactory timerFactory,
        IUiDispatcher dispatcher,
        Action<CoalescedProgressState> onUpdate,
        TimeSpan? interval = null)
    {
        _dispatcher = dispatcher ?? throw new ArgumentNullException(nameof(dispatcher));
        _onUpdate = onUpdate ?? throw new ArgumentNullException(nameof(onUpdate));
        var timerInterval = interval ?? TimeSpan.FromMilliseconds(33); // ~30Hz default
        _timer = timerFactory.CreateTimer(timerInterval, OnTick);
    }

    public void Start()
    {
        lock (_lock)
        {
            _latestReport = null;
            _hasNewReport = false;
            _stopwatch.Restart();
            _timer.Start();
        }
    }

    public void Stop()
    {
        lock (_lock)
        {
            _timer.Stop();
            _stopwatch.Stop();
        }
    }

    public void Report(ScanProgressReport value)
    {
        lock (_lock)
        {
            _latestReport = value;
            _hasNewReport = true;
        }
    }

    private void OnTick()
    {
        ScanProgressReport? reportToDispatch = null;
        TimeSpan elapsed;

        lock (_lock)
        {
            if (!_hasNewReport || _latestReport == null)
            {
                return;
            }

            reportToDispatch = _latestReport;
            _hasNewReport = false;
            elapsed = _stopwatch.Elapsed;
        }

        DispatchState(reportToDispatch, elapsed);
    }

    public void Flush()
    {
        ScanProgressReport? reportToDispatch;
        TimeSpan elapsed;

        lock (_lock)
        {
            reportToDispatch = _latestReport;
            _hasNewReport = false;
            elapsed = _stopwatch.Elapsed;
        }

        if (reportToDispatch != null)
        {
            DispatchState(reportToDispatch, elapsed);
        }
    }

    private void DispatchState(ScanProgressReport report, TimeSpan elapsed)
    {
        var state = new CoalescedProgressState
        {
            OperationId = report.OperationId,
            SequenceNumber = report.SequenceNumber,
            Directories = report.ObservedDirectories,
            Files = report.ObservedFiles,
            LogicalBytes = report.ObservedLogicalBytes,
            AllocatedBytes = report.ObservedAllocatedBytes,
            CoverageGaps = report.CoverageGaps,
            CurrentPhase = report.CurrentPhase,
            CurrentDirectory = report.CurrentDirectory,
            Elapsed = elapsed
        };

        _dispatcher.Post(() => _onUpdate(state));
    }

    public void Dispose()
    {
        Stop();
        _timer.Dispose();
    }
}
