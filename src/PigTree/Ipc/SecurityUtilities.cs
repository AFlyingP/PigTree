using System.Security.Cryptography;
using System.Text;

namespace PigTree.Ipc;

public static class SecurityUtilities
{
    public static byte[] GenerateNonce(int length = 32)
    {
        byte[] nonce = new byte[length];
        RandomNumberGenerator.Fill(nonce);
        return nonce;
    }

    public static bool ConstantTimeEquals(ReadOnlySpan<byte> a, ReadOnlySpan<byte> b)
    {
        return CryptographicOperations.FixedTimeEquals(a, b);
    }

    public static byte[] DeriveChannelKey(ReadOnlySpan<byte> bootstrapNonce, ReadOnlySpan<byte> clientNonce, ReadOnlySpan<byte> serverNonce)
    {
        byte[] prefix = Encoding.ASCII.GetBytes("pigtree-v1-channel-key:");
        byte[] sep = Encoding.ASCII.GetBytes(":");

        int totalLen = prefix.Length + bootstrapNonce.Length + sep.Length + clientNonce.Length + sep.Length + serverNonce.Length;
        byte[] buffer = new byte[totalLen];

        int offset = 0;
        prefix.CopyTo(buffer.AsSpan(offset));
        offset += prefix.Length;

        bootstrapNonce.CopyTo(buffer.AsSpan(offset));
        offset += bootstrapNonce.Length;

        sep.CopyTo(buffer.AsSpan(offset));
        offset += sep.Length;

        clientNonce.CopyTo(buffer.AsSpan(offset));
        offset += clientNonce.Length;

        sep.CopyTo(buffer.AsSpan(offset));
        offset += sep.Length;

        serverNonce.CopyTo(buffer.AsSpan(offset));

        return SHA256.HashData(buffer);
    }
}
