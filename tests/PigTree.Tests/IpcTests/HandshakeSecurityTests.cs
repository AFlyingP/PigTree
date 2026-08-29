using System;
using System.IO;
using System.Security.Cryptography;
using Google.Protobuf;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Ipc;
using PigTree.Session.V1;

namespace PigTree.Tests.IpcTests;

[TestClass]
public class HandshakeSecurityTests
{
    [TestMethod]
    public void DeriveChannelKey_GoldenVector_MatchesRustExactKey()
    {
        // Vector matching Rust test: b = [1; 32], c = [2; 32], s = [3; 32]
        byte[] b = new byte[32]; Array.Fill(b, (byte)1);
        byte[] c = new byte[32]; Array.Fill(c, (byte)2);
        byte[] s = new byte[32]; Array.Fill(s, (byte)3);

        byte[] key = SecurityUtilities.DeriveChannelKey(b, c, s);
        Assert.AreEqual(32, key.Length);

        // Compute expected SHA256 of "pigtree-v1-channel-key:" + b + ":" + c + ":" + s
        byte[] prefix = System.Text.Encoding.ASCII.GetBytes("pigtree-v1-channel-key:");
        byte[] sep = System.Text.Encoding.ASCII.GetBytes(":");
        using var ms = new MemoryStream();
        ms.Write(prefix);
        ms.Write(b);
        ms.Write(sep);
        ms.Write(c);
        ms.Write(sep);
        ms.Write(s);
        byte[] expectedHash = SHA256.HashData(ms.ToArray());

        CollectionAssert.AreEqual(expectedHash, key);
    }

    [TestMethod]
    public void DeriveChannelKey_ProducesDeterministicKey()
    {
        byte[] bootstrap = new byte[32];
        byte[] client = new byte[32];
        byte[] server = new byte[32];
        bootstrap[0] = 1;
        client[0] = 2;
        server[0] = 3;

        byte[] key1 = SecurityUtilities.DeriveChannelKey(bootstrap, client, server);
        byte[] key2 = SecurityUtilities.DeriveChannelKey(bootstrap, client, server);

        Assert.AreEqual(32, key1.Length);
        CollectionAssert.AreEqual(key1, key2);
    }

    [TestMethod]
    public void DeriveChannelKey_DifferentNonces_ProduceDifferentKeys()
    {
        byte[] bootstrap1 = new byte[32];
        byte[] bootstrap2 = new byte[32];
        byte[] client = new byte[32];
        byte[] server = new byte[32];
        bootstrap1[0] = 1;
        bootstrap2[0] = 99;

        byte[] key1 = SecurityUtilities.DeriveChannelKey(bootstrap1, client, server);
        byte[] key2 = SecurityUtilities.DeriveChannelKey(bootstrap2, client, server);

        CollectionAssert.AreNotEqual(key1, key2);
    }

    [TestMethod]
    public void GenerateNonce_ProducesRandomBytes()
    {
        byte[] n1 = SecurityUtilities.GenerateNonce(32);
        byte[] n2 = SecurityUtilities.GenerateNonce(32);

        Assert.AreEqual(32, n1.Length);
        Assert.AreEqual(32, n2.Length);
        CollectionAssert.AreNotEqual(n1, n2);
    }

    [TestMethod]
    public void ConstantTimeEquals_ValidatesIdenticalKeys()
    {
        byte[] a = new byte[32];
        byte[] b = new byte[32];
        a[15] = 42;
        b[15] = 42;

        Assert.IsTrue(SecurityUtilities.ConstantTimeEquals(a, b));
    }

    [TestMethod]
    public void ConstantTimeEquals_RejectsMismatchedKeys()
    {
        byte[] a = new byte[32];
        byte[] b = new byte[32];
        a[15] = 42;
        b[15] = 43;

        Assert.IsFalse(SecurityUtilities.ConstantTimeEquals(a, b));
    }

    [TestMethod]
    public void ConstantTimeEquals_RejectsDifferentLengths()
    {
        byte[] a = new byte[32];
        byte[] b = new byte[16];

        Assert.IsFalse(SecurityUtilities.ConstantTimeEquals(a, b));
    }

    [TestMethod]
    public void RawAuthHandshake_Framing_Roundtrip()
    {
        var req = new AuthHandshakeRequest
        {
            BootstrapNonce = ByteString.CopyFrom(new byte[] { 1, 2, 3, 4 }),
            ClientNonce = ByteString.CopyFrom(new byte[] { 5, 6, 7, 8 }),
            ClientPid = 1234,
            ClientSessionId = 5678
        };

        byte[] raw = req.ToByteArray();
        var decoded = AuthHandshakeRequest.Parser.ParseFrom(raw);

        CollectionAssert.AreEqual(new byte[] { 1, 2, 3, 4 }, decoded.BootstrapNonce.ToByteArray());
        CollectionAssert.AreEqual(new byte[] { 5, 6, 7, 8 }, decoded.ClientNonce.ToByteArray());
        Assert.AreEqual(1234u, decoded.ClientPid);
        Assert.AreEqual(5678u, decoded.ClientSessionId);

        var resp = new AuthHandshakeResponse
        {
            Status = 0,
            ServerNonce = ByteString.CopyFrom(new byte[] { 0xaa, 0xbb }),
            ServerPid = 4321,
            ChannelKeyHash = ByteString.CopyFrom(new byte[] { 0x11, 0x22 }),
            ErrorMessage = ""
        };

        byte[] respRaw = resp.ToByteArray();
        var decodedResp = AuthHandshakeResponse.Parser.ParseFrom(respRaw);

        Assert.AreEqual(0u, decodedResp.Status);
        CollectionAssert.AreEqual(new byte[] { 0xaa, 0xbb }, decodedResp.ServerNonce.ToByteArray());
        Assert.AreEqual(4321u, decodedResp.ServerPid);
        CollectionAssert.AreEqual(new byte[] { 0x11, 0x22 }, decodedResp.ChannelKeyHash.ToByteArray());
    }
}