using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Model;
using PigTree.Services;

namespace PigTree.Tests.CoalescerTests;

public class FakeTimer : IUiTimer
{
    public TimeSpan Interval { get; set; }
    public Action TickAction { get; set; }
    public bool IsRunning { get; private set; }

    public FakeTimer(TimeSpan interval, Action tickAction)
    {
        Interval = interval;
        TickAction = tickAction;
    }

    public void Start() => IsRunning = true;
    public void Stop() => IsRunning = false;
    public void TriggerTick() => TickAction?.Invoke();
    public void Dispose() => IsRunning = false;
}

public class FakeTimerFactory : IUiTimerFactory
{
    public List<FakeTimer> Timers { get; } = new();

    public IUiTimer CreateTimer(TimeSpan interval, Action tickAction)
    {
        var timer = new FakeTimer(interval, tickAction);
        Timers.Add(timer);
        return timer;
    }
}

public class ImmediateDispatcher : IUiDispatcher
{
    public List<Action> ExecutedActions { get; } = new();

    public void Post(Action action)
    {
        ExecutedActions.Add(action);
        action();
    }

    public Task InvokeAsync(Action action)
    {
        ExecutedActions.Add(action);
        action();
        return Task.CompletedTask;
    }
}

[TestClass]
public class ProgressCoalescerTests
{
    [TestMethod]
    public void HighFrequencyReports_AreCoalescedUntilTimerTicks()
    {
        var timerFactory = new FakeTimerFactory();
        var dispatcher = new ImmediateDispatcher();
        var updates = new List<CoalescedProgressState>();

        var coalescer = new ProgressCoalescer(timerFactory, dispatcher, state => updates.Add(state), TimeSpan.FromMilliseconds(33));

        coalescer.Start();
        Assert.AreEqual(1, timerFactory.Timers.Count);
        Assert.IsTrue(timerFactory.Timers[0].IsRunning);

        // Send 10 rapid reports
        for (ulong i = 1; i <= 10; i++)
        {
            coalescer.Report(new ScanProgressReport(
                OperationId: "op-1",
                SequenceNumber: i,
                TimestampIso: "2026-08-29T12:00:00Z",
                ObservedDirectories: i * 10,
                ObservedFiles: i * 100,
                ObservedLogicalBytes: i * 1024 * 1024,
                ObservedReferencedAllocatedBytes: i * 1024 * 1024,
                CoverageGaps: 0,
                CurrentPhase: "Scanning",
                CurrentDirectory: $"C:\\Folder\\Sub{i}"));
        }

        // Before tick, nothing should be dispatched
        Assert.AreEqual(0, updates.Count);

        // Trigger tick
        timerFactory.Timers[0].TriggerTick();

        // Exactly 1 update with the latest pulse (10)
        Assert.AreEqual(1, updates.Count);
        var latest = updates[0];
        Assert.AreEqual(100UL, latest.Directories);
        Assert.AreEqual(1000UL, latest.Files);
        Assert.AreEqual(10UL * 1024 * 1024, latest.LogicalBytes);
        Assert.AreEqual("C:\\Folder\\Sub10", latest.CurrentDirectory);
        Assert.AreEqual("Scanning", latest.CurrentPhase);
    }

    [TestMethod]
    public void TickWithoutNewReport_DoesNotDispatchDuplicate()
    {
        var timerFactory = new FakeTimerFactory();
        var dispatcher = new ImmediateDispatcher();
        var updates = new List<CoalescedProgressState>();

        var coalescer = new ProgressCoalescer(timerFactory, dispatcher, state => updates.Add(state), TimeSpan.FromMilliseconds(33));
        coalescer.Start();

        coalescer.Report(new ScanProgressReport("op-1", 1, "2026-08-29T12:00:00Z", 5, 20, 1024, 1024, 0, "Scanning", "C:\\Test"));
        timerFactory.Timers[0].TriggerTick();
        Assert.AreEqual(1, updates.Count);

        // Second tick with no new reports
        timerFactory.Timers[0].TriggerTick();
        Assert.AreEqual(1, updates.Count);
    }

    [TestMethod]
    public void Flush_ImmediatelyDispatchesPendingReport()
    {
        var timerFactory = new FakeTimerFactory();
        var dispatcher = new ImmediateDispatcher();
        var updates = new List<CoalescedProgressState>();

        var coalescer = new ProgressCoalescer(timerFactory, dispatcher, state => updates.Add(state), TimeSpan.FromMilliseconds(33));
        coalescer.Start();

        coalescer.Report(new ScanProgressReport("op-1", 1, "2026-08-29T12:00:00Z", 5, 20, 1024, 1024, 0, "Scanning", "C:\\Test"));
        Assert.AreEqual(0, updates.Count);

        coalescer.Flush();
        Assert.AreEqual(1, updates.Count);
        Assert.AreEqual("C:\\Test", updates[0].CurrentDirectory);
    }
}
