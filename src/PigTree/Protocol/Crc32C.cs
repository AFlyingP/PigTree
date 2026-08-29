namespace PigTree.Protocol;

/// <summary>
/// Castagnoli CRC-32C implementation matching RFC 3720 and polynomial 0x1EDC6F41 (reversed 0x82F63B78).
/// </summary>
public static class Crc32C
{
    private static readonly uint[] Table = InitializeTable();

    private static uint[] InitializeTable()
    {
        var table = new uint[256];
        const uint poly = 0x82F63B78u;
        for (uint i = 0; i < 256; i++)
        {
            uint crc = i;
            for (int j = 0; j < 8; j++)
            {
                if ((crc & 1) != 0)
                {
                    crc = (crc >> 1) ^ poly;
                }
                else
                {
                    crc >>= 1;
                }
            }
            table[i] = crc;
        }
        return table;
    }

    public static uint Compute(ReadOnlySpan<byte> data)
    {
        return Finalize(Update(0xFFFFFFFFu, data));
    }

    public static uint Update(uint currentCrc, ReadOnlySpan<byte> data)
    {
        uint crc = currentCrc;
        foreach (byte b in data)
        {
            byte index = (byte)((crc ^ b) & 0xFF);
            crc = (crc >> 8) ^ Table[index];
        }
        return crc;
    }

    public static uint Finalize(uint crc)
    {
        return ~crc;
    }
}
