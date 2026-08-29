namespace PigTree.Model;

public enum ScanState
{
    Idle,
    Starting,
    Scanning,
    Cancelling,
    Completed,
    Cancelled,
    Failed
}
