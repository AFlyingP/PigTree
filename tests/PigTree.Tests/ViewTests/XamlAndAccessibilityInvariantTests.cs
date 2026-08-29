using System;
using System.IO;
using System.Text.RegularExpressions;
using System.Windows;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Converters;
using PigTree.Model;
using PigTree.ViewModel;

namespace PigTree.Tests.ViewTests;

[TestClass]
public class XamlAndAccessibilityInvariantTests
{
    private static string FindRepoRoot()
    {
        string dir = AppContext.BaseDirectory;
        while (!string.IsNullOrEmpty(dir))
        {
            if (File.Exists(Path.Combine(dir, "PigTree.sln")) || Directory.Exists(Path.Combine(dir, "src", "PigTree")))
            {
                return dir;
            }
            dir = Path.GetDirectoryName(dir)!;
        }
        throw new InvalidOperationException("Could not find repository root.");
    }

    [TestMethod]
    public void AppManifest_SpecifiesAsInvokerAndPerMonitorV2()
    {
        string repoRoot = FindRepoRoot();
        string manifestPath = Path.Combine(repoRoot, "src", "PigTree", "app.manifest");
        Assert.IsTrue(File.Exists(manifestPath), "Manifest not found");

        string content = File.ReadAllText(manifestPath);
        Assert.IsTrue(content.Contains("level=\"asInvoker\""), "Manifest missing level=\"asInvoker\"");
        Assert.IsTrue(content.Contains("PerMonitorV2"), "Manifest missing PerMonitorV2");
    }

    [TestMethod]
    public void AppXaml_HasNoStartupUri_UsesCodeBehindOnStartup()
    {
        string repoRoot = FindRepoRoot();
        string appXamlPath = Path.Combine(repoRoot, "src", "PigTree", "App.xaml");
        Assert.IsTrue(File.Exists(appXamlPath));

        string content = File.ReadAllText(appXamlPath);
        Assert.IsFalse(content.Contains("StartupUri"), "App.xaml must not specify StartupUri because App.xaml.cs initializes DI and MainWindow.");
    }

    [TestMethod]
    public void PigTreeCsproj_StagesBothEngineAndScanWorkerBinaries()
    {
        string repoRoot = FindRepoRoot();
        string csprojPath = Path.Combine(repoRoot, "src", "PigTree", "PigTree.csproj");
        Assert.IsTrue(File.Exists(csprojPath));

        string content = File.ReadAllText(csprojPath);
        Assert.IsTrue(content.Contains("pigtree-engine.exe"), "PigTree.csproj must include staging for pigtree-engine.exe");
        Assert.IsTrue(content.Contains("pigtree-scan-worker.exe"), "PigTree.csproj must include staging for pigtree-scan-worker.exe");
        Assert.IsTrue(content.Contains("CopyToOutputDirectory"), "Staged binaries must specify CopyToOutputDirectory");
        Assert.IsTrue(content.Contains("CopyToPublishDirectory"), "Staged binaries must specify CopyToPublishDirectory");
    }

    [TestMethod]
    public void MainWindow_ContainsAllRequiredAutomationIds()
    {
        string repoRoot = FindRepoRoot();
        string xamlPath = Path.Combine(repoRoot, "src", "PigTree", "MainWindow.xaml");
        Assert.IsTrue(File.Exists(xamlPath));

        string xaml = File.ReadAllText(xamlPath);

        string[] requiredIds = new[]
        {
            "TargetPathTextBox",
            "BrowseButton",
            "ScanButton",
            "CancelButton",
            "StatusTextBlock",
            "DirectoryTreeView",
            "ErrorBanner",
            "ErrorMessageTextBlock",
            "RetryButton",
            "DismissButton",
            "ExpandCollapseButton"
        };

        foreach (var id in requiredIds)
        {
            string expectedAttr = "AutomationProperties.AutomationId=\"" + id + "\"";
            Assert.IsTrue(xaml.Contains(expectedAttr), "Missing AutomationId: " + id);
        }
    }

    [TestMethod]
    public void MainWindow_ConfiguresLiveSettingRegions()
    {
        string repoRoot = FindRepoRoot();
        string xamlPath = Path.Combine(repoRoot, "src", "PigTree", "MainWindow.xaml");
        string xaml = File.ReadAllText(xamlPath);

        Assert.IsTrue(xaml.Contains("AutomationProperties.LiveSetting=\"Polite\""), "Status text must have Polite live setting");
        Assert.IsTrue(xaml.Contains("AutomationProperties.LiveSetting=\"Assertive\""), "Error message must have Assertive live setting");
    }

    [TestMethod]
    public void AppXaml_ListViewItemStyle_ContainsKeyboardFocusIndicator()
    {
        string repoRoot = FindRepoRoot();
        string appXamlPath = Path.Combine(repoRoot, "src", "PigTree", "App.xaml");
        string appXaml = File.ReadAllText(appXamlPath);

        Assert.IsTrue(appXaml.Contains("Property=\"IsKeyboardFocused\""), "ListViewItem template must contain trigger for IsKeyboardFocused");
        Assert.IsTrue(appXaml.Contains("DynamicResource"), "Focus indicator must use DynamicResource system brush");
    }

    [TestMethod]
    public void MainWindow_ExpanderButton_BindsToggleExpandCommand()
    {
        string repoRoot = FindRepoRoot();
        string xamlPath = Path.Combine(repoRoot, "src", "PigTree", "MainWindow.xaml");
        string xaml = File.ReadAllText(xamlPath);

        Assert.IsTrue(xaml.Contains("ToggleExpandCommand"), "Expander button in MainWindow.xaml must bind ToggleExpandCommand.");
    }

    [TestMethod]
    public void LevelToIndentMarginConverter_ConvertsIntegerAndDoubleLevelsCorrectly()
    {
        var converter = new LevelToIndentMarginConverter { Step = 16.0 };

        var marginLevel0 = (Thickness)converter.Convert(0, typeof(Thickness), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.AreEqual(0.0, marginLevel0.Left);

        var marginLevel1 = (Thickness)converter.Convert(1, typeof(Thickness), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.AreEqual(16.0, marginLevel1.Left);

        var marginLevel3 = (Thickness)converter.Convert(3, typeof(Thickness), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.AreEqual(48.0, marginLevel3.Left);
    }

    [TestMethod]
    public void LevelToIndentMarginConverter_ConvertsTreeItemViewModelAndConvertible()
    {
        var converter = new LevelToIndentMarginConverter { Step = 16.0 };

        var node = new TreeNodeData(1, 0, "Test", 1, 100, 100, true, 0, false);
        var vm = new TreeItemViewModel(node, level: 2);

        var marginFromVm = (Thickness)converter.Convert(vm, typeof(Thickness), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.AreEqual(32.0, marginFromVm.Left);

        var marginFromStr = (Thickness)converter.Convert("4", typeof(Thickness), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.AreEqual(64.0, marginFromStr.Left);
    }

    [TestMethod]
    public void XamlFiles_UseDynamicResourceSystemColors_NoHardcodedHexColors()
    {
        string repoRoot = FindRepoRoot();
        string[] xamlFiles = new[]
        {
            Path.Combine(repoRoot, "src", "PigTree", "App.xaml"),
            Path.Combine(repoRoot, "src", "PigTree", "MainWindow.xaml")
        };

        var hexColorPattern = new Regex(@"#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})");

        foreach (var file in xamlFiles)
        {
            string xaml = File.ReadAllText(file);
            var matches = hexColorPattern.Matches(xaml);
            Assert.AreEqual(0, matches.Count, "Hardcoded color in " + file);
        }
    }

    [TestMethod]
    public void TreeView_HasNoFixedHeightOnRows_AllowsScaling()
    {
        string repoRoot = FindRepoRoot();
        string appXaml = File.ReadAllText(Path.Combine(repoRoot, "src", "PigTree", "App.xaml"));

        var fixedHeightPattern = new Regex(@"<Setter\s+Property=""Height""\s+Value=""\d+""");
        var matches = fixedHeightPattern.Matches(appXaml);
        Assert.AreEqual(0, matches.Count, "App.xaml has fixed row height");
    }

    [TestMethod]
    public void MainWindow_DirectoryTreeView_UsesPixelScrollUnitAndRecyclingMode()
    {
        string repoRoot = FindRepoRoot();
        string xamlPath = Path.Combine(repoRoot, "src", "PigTree", "MainWindow.xaml");
        string xaml = File.ReadAllText(xamlPath);

        Assert.IsTrue(xaml.Contains("VirtualizingStackPanel.ScrollUnit=\"Pixel\""), "DirectoryTreeView must configure VirtualizingStackPanel.ScrollUnit=\"Pixel\" per ADR 0004 section 4.1.");
        Assert.IsTrue(xaml.Contains("VirtualizingStackPanel.VirtualizationMode=\"Recycling\""), "DirectoryTreeView must configure VirtualizingStackPanel.VirtualizationMode=\"Recycling\" per ADR 0004 section 4.1.");
    }
}
