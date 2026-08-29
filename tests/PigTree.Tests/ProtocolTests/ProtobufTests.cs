using System.IO;
using Google.Protobuf;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Session.V1;

namespace PigTree.Tests.ProtocolTests;

[TestClass]
public class ProtobufTests
{
    [TestMethod]
    public void AuthHandshakeRequestResponse_Roundtrip()
    {
        var req = new CommandRequest
        {
            RequestId = "req-1",
            AuthHandshake = new AuthHandshakeRequest
            {
                BootstrapNonce = ByteString.CopyFrom(new byte[] { 1, 2, 3, 4 }),
                ClientNonce = ByteString.CopyFrom(new byte[] { 5, 6, 7, 8 }),
                ClientPid = 1234,
                ClientSessionId = 0
            }
        };

        byte[] bytes = req.ToByteArray();
        var decoded = CommandRequest.Parser.ParseFrom(bytes);

        Assert.AreEqual("req-1", decoded.RequestId);
        Assert.IsNotNull(decoded.AuthHandshake);
        Assert.AreEqual(1234u, decoded.AuthHandshake.ClientPid);
        CollectionAssert.AreEqual(new byte[] { 1, 2, 3, 4 }, decoded.AuthHandshake.BootstrapNonce.ToByteArray());
    }

    [TestMethod]
    public void PingRequestResponse_Roundtrip()
    {
        var resp = new CommandResponse
        {
            RequestId = "ping-1",
            Status = 0,
            Ping = new PingResponse
            {
                TimestampUtcMs = 1700000000000,
                EchoTimestampUtcMs = 1700000000500
            }
        };

        byte[] bytes = resp.ToByteArray();
        var decoded = CommandResponse.Parser.ParseFrom(bytes);

        Assert.AreEqual("ping-1", decoded.RequestId);
        Assert.AreEqual(0u, decoded.Status);
        Assert.AreEqual(1700000000500u, decoded.Ping.EchoTimestampUtcMs);
    }

    [TestMethod]
    public void ScanProgress_Roundtrip()
    {
        var progress = new ScanProgress
        {
            OperationId = "op-scan",
            SequenceNumber = 5,
            TimestampIso = "2026-08-28T12:00:00.000Z",
            ObservedDirectories = 10,
            ObservedFiles = 100,
            ObservedLogicalBytes = 1048576,
            ObservedAllocatedBytes = 2097152,
            CoverageGaps = 1,
            CurrentPhase = "traversing"
        };

        byte[] bytes = progress.ToByteArray();
        var decoded = ScanProgress.Parser.ParseFrom(bytes);

        Assert.AreEqual("op-scan", decoded.OperationId);
        Assert.AreEqual(5u, decoded.SequenceNumber);
        Assert.AreEqual(10u, decoded.ObservedDirectories);
        Assert.AreEqual(100u, decoded.ObservedFiles);
        Assert.AreEqual(1048576u, decoded.ObservedLogicalBytes);
        Assert.AreEqual("traversing", decoded.CurrentPhase);
    }

    [TestMethod]
    public void ScanResponse_Roundtrip()
    {
        var scanResp = new ScanResponse
        {
            OperationId = "op-scan-1",
            TargetPath = @"C:ScanTarget",
            RunOutcome = ScanRunOutcome.Finished,
            ObservationStartedIso = "2026-08-28T12:00:00.000Z",
            ObservationCompletedIso = "2026-08-28T12:01:00.000Z",
            ScopeCoverage = ScopeCoverage.Complete,
            DirectoryCount = 10,
            FileCount = 200,
            SpecialCount = 0,
            LogicalBytes = 10485760,
            AllocatedBytes = 12582912,
            AllocatedBytesKnown = true,
            DurationMs = 60000,
            CoverageGaps =
            {
                new CoverageGapReport
                {
                    DisplayPath = @"C:ScanTargetLocked",
                    StatusCode = "FS_ACCESS_DENIED",
                    NativeError = 5,
                    ErrorMessage = "Access is denied"
                }
            }
        };

        byte[] bytes = scanResp.ToByteArray();
        var decoded = ScanResponse.Parser.ParseFrom(bytes);

        Assert.AreEqual(@"C:ScanTarget", decoded.TargetPath);
        Assert.AreEqual(ScanRunOutcome.Finished, decoded.RunOutcome);
        Assert.AreEqual(10u, decoded.DirectoryCount);
        Assert.AreEqual(200u, decoded.FileCount);
        Assert.AreEqual(1, decoded.CoverageGaps.Count);
        Assert.AreEqual(@"C:ScanTargetLocked", decoded.CoverageGaps[0].DisplayPath);
    }

    [TestMethod]
    public void GetChildrenRequestAndResponse_Roundtrip()
    {
        var req = new CommandRequest
        {
            RequestId = "req-gc-1",
            GetChildren = new GetChildrenRequest
            {
                OperationId = "op-scan-1",
                ParentId = 1,
                Offset = 0,
                Limit = 50
            }
        };

        byte[] reqBytes = req.ToByteArray();
        var decodedReq = CommandRequest.Parser.ParseFrom(reqBytes);
        Assert.AreEqual("req-gc-1", decodedReq.RequestId);
        Assert.IsNotNull(decodedReq.GetChildren);
        Assert.AreEqual("op-scan-1", decodedReq.GetChildren.OperationId);
        Assert.AreEqual(1u, decodedReq.GetChildren.ParentId);

        var resp = new CommandResponse
        {
            RequestId = "req-gc-1",
            Status = 0,
            GetChildren = new GetChildrenResponse
            {
                OperationId = "op-scan-1",
                ParentId = 1,
                TotalChildren = 2,
                Offset = 0,
                Nodes =
                {
                    new DirectoryEntryNode
                    {
                        Id = 2,
                        ParentId = 1,
                        Name = "SubDir",
                        EntryKind = 1,
                        LogicalSize = 0,
                        AllocatedSize = 0,
                        AllocatedSizeKnown = true,
                        ChildCount = 3,
                        HasChildren = true
                    },
                    new DirectoryEntryNode
                    {
                        Id = 3,
                        ParentId = 1,
                        Name = "file.txt",
                        EntryKind = 2,
                        LogicalSize = 1024,
                        AllocatedSize = 4096,
                        AllocatedSizeKnown = true,
                        ChildCount = 0,
                        HasChildren = false
                    }
                }
            }
        };

        byte[] respBytes = resp.ToByteArray();
        var decodedResp = CommandResponse.Parser.ParseFrom(respBytes);
        Assert.AreEqual(0u, decodedResp.Status);
        Assert.IsNotNull(decodedResp.GetChildren);
        Assert.AreEqual(2, decodedResp.GetChildren.Nodes.Count);
        Assert.AreEqual("SubDir", decodedResp.GetChildren.Nodes[0].Name);
        Assert.AreEqual(1u, decodedResp.GetChildren.Nodes[0].EntryKind);
        Assert.IsTrue(decodedResp.GetChildren.Nodes[0].HasChildren);
        Assert.AreEqual("file.txt", decodedResp.GetChildren.Nodes[1].Name);
        Assert.AreEqual(2u, decodedResp.GetChildren.Nodes[1].EntryKind);
        Assert.IsFalse(decodedResp.GetChildren.Nodes[1].HasChildren);
    }
}
