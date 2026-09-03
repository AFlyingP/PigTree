using System;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Threading;

namespace PigTree.Tests.ViewTests;

/// <summary>
/// Smallest reusable persistent STA dispatcher host for UI behavior tests.
/// Avoids creating disposable STA threads and avoids creating/disposing Application across tests.
/// Starts one persistent Application instance on the dedicated STA thread with App.xaml resources.
/// </summary>
public static class PersistentDispatcherHost
{
    private static readonly Thread _staThread;
    private static readonly Dispatcher _dispatcher;

    static PersistentDispatcherHost()
    {
        var tcs = new TaskCompletionSource<Dispatcher>();
        _staThread = new Thread(() =>
        {
            if (Application.Current == null)
            {
                var app = new PigTree.App
                {
                    ShutdownMode = ShutdownMode.OnExplicitShutdown
                };
                app.InitializeComponent();
            }

            var d = Dispatcher.CurrentDispatcher;
            tcs.SetResult(d);
            Dispatcher.Run();
        })
        {
            IsBackground = true,
            Name = "PigTreeTestSTAHost"
        };
        _staThread.SetApartmentState(ApartmentState.STA);
        _staThread.Start();
        _dispatcher = tcs.Task.GetAwaiter().GetResult();
    }

    public static Dispatcher Dispatcher => _dispatcher;

    public static void Run(Action action)
    {
        _dispatcher.Invoke(action);
    }

    public static T Run<T>(Func<T> func)
    {
        return _dispatcher.Invoke(func);
    }

    public static async Task RunAsync(Func<Task> action)
    {
        await _dispatcher.InvokeAsync(action).Task.Unwrap();
    }

    /// <summary>
    /// Drains any pending dispatcher operations down to ApplicationIdle.
    /// </summary>
    public static void Drain(DispatcherPriority priority = DispatcherPriority.ApplicationIdle)
    {
        if (Dispatcher.CheckAccess())
        {
            DrainOnCurrentDispatcher(priority);
        }
        else
        {
            _dispatcher.Invoke(() => DrainOnCurrentDispatcher(priority));
        }
    }

    private static void DrainOnCurrentDispatcher(DispatcherPriority priority)
    {
        var frame = new DispatcherFrame();
        Dispatcher.CurrentDispatcher.BeginInvoke(priority, new DispatcherOperationCallback(f =>
        {
            ((DispatcherFrame)f).Continue = false;
            return null;
        }), frame);
        Dispatcher.PushFrame(frame);
    }
}
