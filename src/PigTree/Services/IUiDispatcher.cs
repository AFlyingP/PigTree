namespace PigTree.Services;

public interface IUiDispatcher
{
    void Post(Action action);
    Task InvokeAsync(Action action);
}
