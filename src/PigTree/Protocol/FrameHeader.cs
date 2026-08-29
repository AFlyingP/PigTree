namespace PigTree.Protocol;

[Flags]
public enum FrameFlags : byte
{
    None = 0,
    EndOfStream = 0x01,
    ChallengeRequired = 0x02,
}

public enum ChannelTag : byte
{
    Command = 0x01,
    LosslessDomain = 0x02,
    ProgressPulse = 0x03,
    CancellationHeartbeat = 0x04,
}

public sealed class FrameHeader
{
    public ChannelTag ChannelTag { get; set; }
    public FrameFlags Flags { get; set; }
    public ulong SequenceNumber { get; set; }
    public uint PayloadLength { get; set; }
}

public sealed class Frame
{
    public FrameHeader Header { get; }
    public byte[] Payload { get; }

    public Frame(FrameHeader header, byte[] payload)
    {
        Header = header ?? throw new ArgumentNullException(nameof(header));
        Payload = payload ?? throw new ArgumentNullException(nameof(payload));
    }
}
