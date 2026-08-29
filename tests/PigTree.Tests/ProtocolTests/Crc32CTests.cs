using System.Text;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Protocol;

namespace PigTree.Tests.ProtocolTests;

[TestClass]
public class Crc32CTests
{
    [TestMethod]
    public void Compute_KnownVector_123456789_Matches_0xe3069283()
    {
        byte[] data = Encoding.ASCII.GetBytes("123456789");
        uint crc = Crc32C.Compute(data);
        Assert.AreEqual(0xe3069283u, crc);
    }

    [TestMethod]
    public void Compute_32ZeroBytes_Matches_0x8a9136aa()
    {
        byte[] data = new byte[32];
        uint crc = Crc32C.Compute(data);
        Assert.AreEqual(0x8a9136aau, crc);
    }

    [TestMethod]
    public void Compute_32FFBytes_Matches_0x62a8ab43()
    {
        byte[] data = new byte[32];
        Array.Fill(data, (byte)0xff);
        uint crc = Crc32C.Compute(data);
        Assert.AreEqual(0x62a8ab43u, crc);
    }

    [TestMethod]
    public void UpdateAndFinalize_IncrementalCalculation_MatchesSingleCompute()
    {
        byte[] data = Encoding.UTF8.GetBytes("The quick brown fox jumps over the lazy dog 1234567890! PigTree framed protocol CRC32C test.");
        uint expected = Crc32C.Compute(data);

        // Split into chunks and compute incrementally
        uint crc = 0xFFFFFFFFu;
        crc = Crc32C.Update(crc, data.AsSpan(0, 15));
        crc = Crc32C.Update(crc, data.AsSpan(15, 30));
        crc = Crc32C.Update(crc, data.AsSpan(45));
        uint actual = Crc32C.Finalize(crc);

        Assert.AreEqual(expected, actual);
    }
}
