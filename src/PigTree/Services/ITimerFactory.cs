namespace PigTree.Services;

public interface IUiTimer : IDisposable
{
    void Start();
    void Stop();
    bool IsRunning { get; }
}

public interface IUiTimerFactory
{
    IUiTimer CreateTimer(TimeSpan interval, Action tickAction);
}
