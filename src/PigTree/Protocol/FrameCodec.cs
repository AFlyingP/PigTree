using System.IO;
using System.Buffers.Binary;

namespace PigTree.Protocol;

public static class FrameCodec
{
    public static readonly byte[] Magic = { 0x50, 0x54 }; // "PT"
    public const ushort SchemaVersion = 1;
    public const int HeaderSize = 20;
    public const int CrcSize = 4;
    public const int MaxPayloadSize = 4 * 1024 * 1024; // 4 MiB

    public static byte[] EncodeFrame(FrameHeader header, ReadOnlySpan<byte> payload)
    {
        ArgumentNullException.ThrowIfNull(header);
        if (payload.Length > MaxPayloadSize)
        {
            throw new ArgumentException($"Payload size {payload.Length} exceeds maximum {MaxPayloadSize}", nameof(payload));
        }

        int totalSize = HeaderSize + payload.Length + CrcSize;
        byte[] buffer = new byte[totalSize];

        // 0..2: Magic
        buffer[0] = Magic[0];
        buffer[1] = Magic[1];
        // 2..4: Version
        BinaryPrimitives.WriteUInt16LittleEndian(buffer.AsSpan(2, 2), SchemaVersion);
        // 4: ChannelTag
        buffer[4] = (byte)header.ChannelTag;
        // 5: Flags
        buffer[5] = (byte)header.Flags;
        // 6..8: Reserved (0)
        BinaryPrimitives.WriteUInt16LittleEndian(buffer.AsSpan(6, 2), 0);
        // 8..16: SequenceNumber
        BinaryPrimitives.WriteUInt64LittleEndian(buffer.AsSpan(8, 8), header.SequenceNumber);
        // 16..20: PayloadLength
        BinaryPrimitives.WriteUInt32LittleEndian(buffer.AsSpan(16, 4), (uint)payload.Length);

        // 20..20+N: Payload
        if (payload.Length > 0)
        {
            payload.CopyTo(buffer.AsSpan(HeaderSize, payload.Length));
        }

        // CRC32C over header + payload
        uint crc = Crc32C.Compute(buffer.AsSpan(0, HeaderSize + payload.Length));
        BinaryPrimitives.WriteUInt32LittleEndian(buffer.AsSpan(HeaderSize + payload.Length, CrcSize), crc);

        return buffer;
    }

    public static void WriteFrame(Stream stream, FrameHeader header, byte[] payload)
    {
        ArgumentNullException.ThrowIfNull(stream);
        ArgumentNullException.ThrowIfNull(header);
        ArgumentNullException.ThrowIfNull(payload);

        byte[] encoded = EncodeFrame(header, payload);
        stream.Write(encoded, 0, encoded.Length);
        stream.Flush();
    }

    public static async Task WriteFrameAsync(Stream stream, FrameHeader header, byte[] payload, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(stream);
        ArgumentNullException.ThrowIfNull(header);
        ArgumentNullException.ThrowIfNull(payload);

        byte[] encoded = EncodeFrame(header, payload);
        await stream.WriteAsync(encoded.AsMemory(), cancellationToken).ConfigureAwait(false);
        await stream.FlushAsync(cancellationToken).ConfigureAwait(false);
    }

    public static Frame DecodeFrame(ReadOnlySpan<byte> buffer)
    {
        if (buffer.Length < HeaderSize)
        {
            throw new EndOfStreamException($"Premature EOF: buffer length {buffer.Length} is less than header size {HeaderSize}");
        }

        if (buffer[0] != Magic[0] || buffer[1] != Magic[1])
        {
            throw new InvalidDataException($"Invalid frame magic: [0x{buffer[0]:X2}, 0x{buffer[1]:X2}]");
        }

        ushort version = BinaryPrimitives.ReadUInt16LittleEndian(buffer.Slice(2, 2));
        if (version != SchemaVersion)
        {
            throw new InvalidDataException($"Unsupported frame version: {version}");
        }

        byte tagByte = buffer[4];
        if (!Enum.IsDefined(typeof(ChannelTag), tagByte))
        {
            throw new InvalidDataException($"Invalid channel tag: {tagByte}");
        }
        var channelTag = (ChannelTag)tagByte;
        var flags = (FrameFlags)buffer[5];

        ushort reserved = BinaryPrimitives.ReadUInt16LittleEndian(buffer.Slice(6, 2));
        if (reserved != 0)
        {
            throw new InvalidDataException($"Non-zero reserved field: {reserved}");
        }

        ulong seq = BinaryPrimitives.ReadUInt64LittleEndian(buffer.Slice(8, 8));
        uint payloadLen = BinaryPrimitives.ReadUInt32LittleEndian(buffer.Slice(16, 4));

        if (payloadLen > MaxPayloadSize)
        {
            throw new InvalidDataException($"Payload length {payloadLen} exceeds max {MaxPayloadSize}");
        }

        int requiredLen = HeaderSize + (int)payloadLen + CrcSize;
        if (buffer.Length < requiredLen)
        {
            throw new EndOfStreamException($"Premature EOF: buffer length {buffer.Length} is less than required frame length {requiredLen}");
        }

        uint expectedCrc = BinaryPrimitives.ReadUInt32LittleEndian(buffer.Slice(HeaderSize + (int)payloadLen, CrcSize));
        uint computedCrc = Crc32C.Compute(buffer.Slice(0, HeaderSize + (int)payloadLen));

        if (expectedCrc != computedCrc)
        {
            throw new InvalidDataException($"CRC32C checksum mismatch: expected 0x{expectedCrc:X8}, computed 0x{computedCrc:X8}");
        }

        byte[] payload = payloadLen == 0 ? Array.Empty<byte>() : buffer.Slice(HeaderSize, (int)payloadLen).ToArray();
        var header = new FrameHeader
        {
            ChannelTag = channelTag,
            Flags = flags,
            SequenceNumber = seq,
            PayloadLength = payloadLen
        };

        return new Frame(header, payload);
    }

    public static Frame? ReadFrame(Stream stream)
    {
        ArgumentNullException.ThrowIfNull(stream);

        byte[] headerBuf = new byte[HeaderSize];
        int firstByte = stream.ReadByte();
        if (firstByte == -1)
        {
            return null; // Clean EOF
        }
        headerBuf[0] = (byte)firstByte;

        // Premature EOF if stream ends before 20-byte header completes
        ReadExact(stream, headerBuf, 1, HeaderSize - 1);

        if (headerBuf[0] != Magic[0] || headerBuf[1] != Magic[1])
        {
            throw new InvalidDataException($"Invalid frame magic: [0x{headerBuf[0]:X2}, 0x{headerBuf[1]:X2}]");
        }

        ushort version = BinaryPrimitives.ReadUInt16LittleEndian(headerBuf.AsSpan(2, 2));
        if (version != SchemaVersion)
        {
            throw new InvalidDataException($"Unsupported frame version: {version}");
        }

        byte tagByte = headerBuf[4];
        if (!Enum.IsDefined(typeof(ChannelTag), tagByte))
        {
            throw new InvalidDataException($"Invalid channel tag: {tagByte}");
        }
        var channelTag = (ChannelTag)tagByte;
        var flags = (FrameFlags)headerBuf[5];

        ushort reserved = BinaryPrimitives.ReadUInt16LittleEndian(headerBuf.AsSpan(6, 2));
        if (reserved != 0)
        {
            throw new InvalidDataException($"Non-zero reserved field: {reserved}");
        }

        ulong seq = BinaryPrimitives.ReadUInt64LittleEndian(headerBuf.AsSpan(8, 8));
        uint payloadLen = BinaryPrimitives.ReadUInt32LittleEndian(headerBuf.AsSpan(16, 4));

        if (payloadLen > MaxPayloadSize)
        {
            throw new InvalidDataException($"Payload length {payloadLen} exceeds max {MaxPayloadSize}");
        }

        byte[] payload = payloadLen == 0 ? Array.Empty<byte>() : new byte[payloadLen];
        if (payloadLen > 0)
        {
            ReadExact(stream, payload, 0, (int)payloadLen);
        }

        byte[] crcBuf = new byte[CrcSize];
        ReadExact(stream, crcBuf, 0, CrcSize);
        uint expectedCrc = BinaryPrimitives.ReadUInt32LittleEndian(crcBuf);

        uint crc = Crc32C.Update(0xFFFFFFFFu, headerBuf);
        crc = Crc32C.Update(crc, payload);
        uint computedCrc = Crc32C.Finalize(crc);

        if (expectedCrc != computedCrc)
        {
            throw new InvalidDataException($"CRC32C checksum mismatch: expected 0x{expectedCrc:X8}, computed 0x{computedCrc:X8}");
        }

        var header = new FrameHeader
        {
            ChannelTag = channelTag,
            Flags = flags,
            SequenceNumber = seq,
            PayloadLength = payloadLen
        };

        return new Frame(header, payload);
    }

    public static async Task<Frame?> ReadFrameAsync(Stream stream, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(stream);

        byte[] headerBuf = new byte[HeaderSize];
        byte[] firstByte = new byte[1];
        int read = await stream.ReadAsync(firstByte.AsMemory(0, 1), cancellationToken).ConfigureAwait(false);
        if (read == 0)
        {
            return null; // Clean EOF
        }
        headerBuf[0] = firstByte[0];

        // Premature EOF if stream ends before 20-byte header completes
        await ReadExactAsync(stream, headerBuf, 1, HeaderSize - 1, cancellationToken).ConfigureAwait(false);

        if (headerBuf[0] != Magic[0] || headerBuf[1] != Magic[1])
        {
            throw new InvalidDataException($"Invalid frame magic: [0x{headerBuf[0]:X2}, 0x{headerBuf[1]:X2}]");
        }

        ushort version = BinaryPrimitives.ReadUInt16LittleEndian(headerBuf.AsSpan(2, 2));
        if (version != SchemaVersion)
        {
            throw new InvalidDataException($"Unsupported frame version: {version}");
        }

        byte tagByte = headerBuf[4];
        if (!Enum.IsDefined(typeof(ChannelTag), tagByte))
        {
            throw new InvalidDataException($"Invalid channel tag: {tagByte}");
        }
        var channelTag = (ChannelTag)tagByte;
        var flags = (FrameFlags)headerBuf[5];

        ushort reserved = BinaryPrimitives.ReadUInt16LittleEndian(headerBuf.AsSpan(6, 2));
        if (reserved != 0)
        {
            throw new InvalidDataException($"Non-zero reserved field: {reserved}");
        }

        ulong seq = BinaryPrimitives.ReadUInt64LittleEndian(headerBuf.AsSpan(8, 8));
        uint payloadLen = BinaryPrimitives.ReadUInt32LittleEndian(headerBuf.AsSpan(16, 4));

        if (payloadLen > MaxPayloadSize)
        {
            throw new InvalidDataException($"Payload length {payloadLen} exceeds max {MaxPayloadSize}");
        }

        byte[] payload = payloadLen == 0 ? Array.Empty<byte>() : new byte[payloadLen];
        if (payloadLen > 0)
        {
            await ReadExactAsync(stream, payload, 0, (int)payloadLen, cancellationToken).ConfigureAwait(false);
        }

        byte[] crcBuf = new byte[CrcSize];
        await ReadExactAsync(stream, crcBuf, 0, CrcSize, cancellationToken).ConfigureAwait(false);
        uint expectedCrc = BinaryPrimitives.ReadUInt32LittleEndian(crcBuf);

        uint crc = Crc32C.Update(0xFFFFFFFFu, headerBuf);
        crc = Crc32C.Update(crc, payload);
        uint computedCrc = Crc32C.Finalize(crc);

        if (expectedCrc != computedCrc)
        {
            throw new InvalidDataException($"CRC32C checksum mismatch: expected 0x{expectedCrc:X8}, computed 0x{computedCrc:X8}");
        }

        var header = new FrameHeader
        {
            ChannelTag = channelTag,
            Flags = flags,
            SequenceNumber = seq,
            PayloadLength = payloadLen
        };

        return new Frame(header, payload);
    }

    private static void ReadExact(Stream stream, byte[] buffer, int offset, int count)
    {
        int totalRead = 0;
        while (totalRead < count)
        {
            int read = stream.Read(buffer, offset + totalRead, count - totalRead);
            if (read == 0)
            {
                throw new EndOfStreamException($"Premature EOF while reading {count} bytes (read {totalRead})");
            }
            totalRead += read;
        }
    }

    private static async Task ReadExactAsync(Stream stream, byte[] buffer, int offset, int count, CancellationToken cancellationToken)
    {
        int totalRead = 0;
        while (totalRead < count)
        {
            int read = await stream.ReadAsync(buffer.AsMemory(offset + totalRead, count - totalRead), cancellationToken).ConfigureAwait(false);
            if (read == 0)
            {
                throw new EndOfStreamException($"Premature EOF while reading {count} bytes (read {totalRead})");
            }
            totalRead += read;
        }
    }
}