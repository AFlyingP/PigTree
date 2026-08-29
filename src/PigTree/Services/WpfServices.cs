using System;
using System.Threading.Tasks;
using System.Windows.Threading;

namespace PigTree.Services;

public sealed class WpfUiDispatcher : IUiDispatcher
{
    private readonly Dispatcher _dispatcher;

    public WpfUiDispatcher(Dispatcher? dispatcher = null)
    {
        _dispatcher = dispatcher ?? Dispatcher.CurrentDispatcher;
    }

    public void Post(Action action)
    {
        if (_dispatcher.CheckAccess())
        {
            action();
        }
        else
        {
            _dispatcher.BeginInvoke(action);
        }
    }

    public async Task InvokeAsync(Action action)
    {
        if (_dispatcher.CheckAccess())
        {
            action();
        }
        else
        {
            await _dispatcher.InvokeAsync(action);
        }
    }
}

public sealed class WpfUiTimer : IUiTimer
{
    private readonly DispatcherTimer _timer;
    private readonly Action _tickAction;

    public bool IsRunning => _timer.IsEnabled;

    public WpfUiTimer(TimeSpan interval, Action tickAction, Dispatcher? dispatcher = null)
    {
        _tickAction = tickAction ?? throw new ArgumentNullException(nameof(tickAction));
        _timer = new DispatcherTimer(DispatcherPriority.Normal, dispatcher ?? Dispatcher.CurrentDispatcher)
        {
            Interval = interval
        };
        _timer.Tick += (s, e) => _tickAction();
    }

    public void Start() => _timer.Start();
    public void Stop() => _timer.Stop();
    public void Dispose() => _timer.Stop();
}

public sealed class WpfUiTimerFactory : IUiTimerFactory
{
    private readonly Dispatcher? _dispatcher;

    public WpfUiTimerFactory(Dispatcher? dispatcher = null)
    {
        _dispatcher = dispatcher;
    }

    public IUiTimer CreateTimer(TimeSpan interval, Action tickAction)
    {
        return new WpfUiTimer(interval, tickAction, _dispatcher);
    }
}
