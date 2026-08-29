using System.IO;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Ipc;

namespace PigTree.Tests.IpcTests;

[TestClass]
public class EngineProcessLauncherTests
{
    [TestMethod]
    public void FindEngineBinary_NonExistentOverride_ThrowsFileNotFoundException()
    {
        string fakePath = Path.Combine(Path.GetTempPath(), "non_existent_engine_path_12345.exe");
        var ex = Assert.ThrowsException<FileNotFoundException>(() =>
        {
            EngineProcessLauncher.FindEngineBinary(fakePath);
        });

        StringAssert.Contains(ex.Message, fakePath);
    }
}
