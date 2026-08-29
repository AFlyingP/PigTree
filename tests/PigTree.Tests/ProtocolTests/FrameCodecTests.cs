using System;
using System.Buffers.Binary;
using System.IO;
using System.Text;
using System.Threading.Tasks;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Protocol;

namespace PigTree.Tests.ProtocolTests;

[TestClass]
public class FrameCodecTests
{
    [TestMethod]
    public void EncodeAndDecode_ValidFrame_PreservesHeaderAndPayload()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.Command,
            Flags = FrameFlags.None,
            SequenceNumber = 42,
        };
        byte[] payload = Encoding.UTF8.GetBytes("Hello, framed PigTree!");

        using var ms = new MemoryStream();
        FrameCodec.WriteFrame(ms, header, payload);

        ms.Position = 0;
        var decoded = FrameCodec.ReadFrame(ms);

        Assert.IsNotNull(decoded);
        Assert.AreEqual(ChannelTag.Command, decoded.Header.ChannelTag);
        Assert.AreEqual(FrameFlags.None, decoded.Header.Flags);
        Assert.AreEqual(42u, decoded.Header.SequenceNumber);
        Assert.AreEqual(payload.Length, decoded.Payload.Length);
        CollectionAssert.AreEqual(payload, decoded.Payload);
    }

    [TestMethod]
    public async Task EncodeAndDecodeAsync_ValidFrame_PreservesHeaderAndPayload()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.ProgressPulse,
            Flags = FrameFlags.EndOfStream,
            SequenceNumber = 100,
        };
        byte[] payload = Encoding.UTF8.GetBytes("Async payload test");

        using var ms = new MemoryStream();
        await FrameCodec.WriteFrameAsync(ms, header, payload);

        ms.Position = 0;
        var decoded = await FrameCodec.ReadFrameAsync(ms);

        Assert.IsNotNull(decoded);
        Assert.AreEqual(ChannelTag.ProgressPulse, decoded.Header.ChannelTag);
        Assert.AreEqual(FrameFlags.EndOfStream, decoded.Header.Flags);
        Assert.AreEqual(100u, decoded.Header.SequenceNumber);
        CollectionAssert.AreEqual(payload, decoded.Payload);
    }

    [TestMethod]
    public void EncodeAndDecode_EmptyPayload_Succeeds()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.Command,
            Flags = FrameFlags.None,
            SequenceNumber = 1,
        };
        byte[] payload = Array.Empty<byte>();

        byte[] encoded = FrameCodec.EncodeFrame(header, payload);
        Assert.AreEqual(FrameCodec.HeaderSize + FrameCodec.CrcSize, encoded.Length);

        var decoded = FrameCodec.DecodeFrame(encoded);
        Assert.IsNotNull(decoded);
        Assert.AreEqual(ChannelTag.Command, decoded.Header.ChannelTag);
        Assert.AreEqual(0, decoded.Payload.Length);
    }

    [TestMethod]
    public void ReadFrame_CleanEofOnEmptyStream_ReturnsNull()
    {
        using var emptyMs = new MemoryStream(Array.Empty<byte>());
        var frame = FrameCodec.ReadFrame(emptyMs);
        Assert.IsNull(frame);
    }

    [TestMethod]
    public async Task ReadFrameAsync_CleanEofOnEmptyStream_ReturnsNull()
    {
        using var emptyMs = new MemoryStream(Array.Empty<byte>());
        var frame = await FrameCodec.ReadFrameAsync(emptyMs);
        Assert.IsNull(frame);
    }

    [TestMethod]
    public void Decode_InvalidMagic_ThrowsInvalidDataException()
    {
        byte[] raw = new byte[FrameCodec.HeaderSize + FrameCodec.CrcSize];
        raw[0] = 0x58; // 'X' instead of 'P'
        raw[1] = 0x54;

        using var ms = new MemoryStream(raw);
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.ReadFrame(ms));
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.DecodeFrame(raw));
    }

    [TestMethod]
    public void Decode_UnsupportedVersion_ThrowsInvalidDataException()
    {
        byte[] raw = new byte[FrameCodec.HeaderSize + FrameCodec.CrcSize];
        raw[0] = FrameCodec.Magic[0];
        raw[1] = FrameCodec.Magic[1];
        BinaryPrimitives.WriteUInt16LittleEndian(raw.AsSpan(2, 2), 2); // Version 2

        using var ms = new MemoryStream(raw);
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.ReadFrame(ms));
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.DecodeFrame(raw));
    }

    [TestMethod]
    public void Decode_InvalidChannelTag_ThrowsInvalidDataException()
    {
        byte[] raw = new byte[FrameCodec.HeaderSize + FrameCodec.CrcSize];
        raw[0] = FrameCodec.Magic[0];
        raw[1] = FrameCodec.Magic[1];
        BinaryPrimitives.WriteUInt16LittleEndian(raw.AsSpan(2, 2), FrameCodec.SchemaVersion);
        raw[4] = 0x99; // Invalid channel tag

        using var ms = new MemoryStream(raw);
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.ReadFrame(ms));
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.DecodeFrame(raw));
    }

    [TestMethod]
    public void Decode_NonZeroReservedField_ThrowsInvalidDataException()
    {
        byte[] raw = new byte[FrameCodec.HeaderSize + FrameCodec.CrcSize];
        raw[0] = FrameCodec.Magic[0];
        raw[1] = FrameCodec.Magic[1];
        BinaryPrimitives.WriteUInt16LittleEndian(raw.AsSpan(2, 2), FrameCodec.SchemaVersion);
        raw[4] = (byte)ChannelTag.Command;
        BinaryPrimitives.WriteUInt16LittleEndian(raw.AsSpan(6, 2), 0x0001); // Non-zero reserved

        using var ms = new MemoryStream(raw);
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.ReadFrame(ms));
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.DecodeFrame(raw));
    }

    [TestMethod]
    public void Decode_CorruptedPayload_ChecksumMismatchThrows()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.Command,
            Flags = FrameFlags.None,
            SequenceNumber = 1,
        };
        byte[] payload = Encoding.UTF8.GetBytes("Important data");

        byte[] encoded = FrameCodec.EncodeFrame(header, payload);
        encoded[21] ^= 0xFF; // Corrupt a payload byte

        using var corruptMs = new MemoryStream(encoded);
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.ReadFrame(corruptMs));
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.DecodeFrame(encoded));
    }

    [TestMethod]
    public void Decode_PrematureEofInHeader_ThrowsEndOfStreamException()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.Command,
            Flags = FrameFlags.None,
            SequenceNumber = 1,
        };
        byte[] payload = Encoding.UTF8.GetBytes("Data needing full length");
        byte[] encoded = FrameCodec.EncodeFrame(header, payload);

        // Truncate stream inside header
        byte[] truncated = encoded[..10];
        using var truncMs = new MemoryStream(truncated);
        Assert.ThrowsException<EndOfStreamException>(() => FrameCodec.ReadFrame(truncMs));
        Assert.ThrowsException<EndOfStreamException>(() => FrameCodec.DecodeFrame(truncated));
    }

    [TestMethod]
    public void Decode_PrematureEofInPayload_ThrowsEndOfStreamException()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.Command,
            Flags = FrameFlags.None,
            SequenceNumber = 1,
        };
        byte[] payload = Encoding.UTF8.GetBytes("Data needing full length");
        byte[] encoded = FrameCodec.EncodeFrame(header, payload);

        // Truncate stream inside payload
        byte[] truncated = encoded[..(FrameCodec.HeaderSize + 5)];
        using var truncMs = new MemoryStream(truncated);
        Assert.ThrowsException<EndOfStreamException>(() => FrameCodec.ReadFrame(truncMs));
        Assert.ThrowsException<EndOfStreamException>(() => FrameCodec.DecodeFrame(truncated));
    }

    [TestMethod]
    public void Decode_PrematureEofInCrc_ThrowsEndOfStreamException()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.Command,
            Flags = FrameFlags.None,
            SequenceNumber = 1,
        };
        byte[] payload = Encoding.UTF8.GetBytes("Data needing full length");
        byte[] encoded = FrameCodec.EncodeFrame(header, payload);

        // Truncate stream inside CRC
        byte[] truncated = encoded[..(encoded.Length - 2)];
        using var truncMs = new MemoryStream(truncated);
        Assert.ThrowsException<EndOfStreamException>(() => FrameCodec.ReadFrame(truncMs));
        Assert.ThrowsException<EndOfStreamException>(() => FrameCodec.DecodeFrame(truncated));
    }

    [TestMethod]
    public void Encode_PayloadExceedingMax_ThrowsArgumentException()
    {
        var header = new FrameHeader
        {
            ChannelTag = ChannelTag.Command,
            Flags = FrameFlags.None,
            SequenceNumber = 1,
        };
        byte[] oversized = new byte[FrameCodec.MaxPayloadSize + 1];

        using var ms = new MemoryStream();
        Assert.ThrowsException<ArgumentException>(() => FrameCodec.WriteFrame(ms, header, oversized));
    }

    [TestMethod]
    public void Decode_PayloadLengthExceedingMaxInHeader_ThrowsInvalidDataException()
    {
        byte[] raw = new byte[FrameCodec.HeaderSize + FrameCodec.CrcSize];
        raw[0] = FrameCodec.Magic[0];
        raw[1] = FrameCodec.Magic[1];
        BinaryPrimitives.WriteUInt16LittleEndian(raw.AsSpan(2, 2), FrameCodec.SchemaVersion);
        raw[4] = (byte)ChannelTag.Command;
        BinaryPrimitives.WriteUInt32LittleEndian(raw.AsSpan(16, 4), (uint)FrameCodec.MaxPayloadSize + 1);

        using var ms = new MemoryStream(raw);
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.ReadFrame(ms));
        Assert.ThrowsException<InvalidDataException>(() => FrameCodec.DecodeFrame(raw));
    }
}
