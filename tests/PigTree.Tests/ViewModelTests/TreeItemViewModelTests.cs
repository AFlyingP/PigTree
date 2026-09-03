using System;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using PigTree.Model;
using PigTree.ViewModel;

namespace PigTree.Tests.ViewModelTests;

[TestClass]
public class TreeItemViewModelTests
{
    [TestMethod]
    public void AliasBadge_Empty_WhenObservedAliasCountBelowTwo()
    {
        var node = new TreeNodeData(1, 0, "file.txt", 2, 100, 100, true, 0, false, ObservedAliasCount: 1);
        var vm = new TreeItemViewModel(node, level: 0);

        Assert.IsFalse(vm.HasAliasBadge);
        Assert.AreEqual(string.Empty, vm.AliasBadgeText);
    }

    [TestMethod]
    public void AliasBadge_ShowsText_WhenObservedAliasCountAtLeastTwo()
    {
        var node = new TreeNodeData(1, 0, "file.txt", 2, 100, 100, true, 0, false, ObservedAliasCount: 2);
        var vm = new TreeItemViewModel(node, level: 0);

        Assert.IsTrue(vm.HasAliasBadge);
        Assert.AreEqual("×2 aliases", vm.AliasBadgeText);

        var manyAliases = new TreeItemViewModel(
            new TreeNodeData(2, 0, "hard.log", 2, 100, 100, true, 0, false, ObservedAliasCount: 5), level: 0);
        Assert.AreEqual("×5 aliases", manyAliases.AliasBadgeText);
    }

    [TestMethod]
    public void ExternalLinkBadge_OnlyForConfirmedExternal()
    {
        var confirmed = new TreeItemViewModel(
            new TreeNodeData(1, 0, "link.dat", 2, 100, 100, true, 0, false,
                ExternalReferenceStatus: ExternalReference.ConfirmedExternal), level: 0);
        Assert.IsTrue(confirmed.HasExternalLinkBadge);
        Assert.AreEqual("external link", confirmed.ExternalLinkBadgeText);

        // Confirmed external is surfaced even when only one alias was observed
        Assert.AreEqual(1u, confirmed.ObservedAliasCount);
        Assert.IsFalse(confirmed.HasAliasBadge);
    }

    [TestMethod]
    public void ExternalLinkBadge_NeverShownForIndeterminateOrOtherStatuses()
    {
        foreach (var status in new[]
                 {
                     ExternalReference.ConfirmedNone,
                     ExternalReference.Indeterminate,
                     ExternalReference.InconsistentEvidence,
                     ExternalReference.NotApplicable
                 })
        {
            var vm = new TreeItemViewModel(
                new TreeNodeData(1, 0, "mystery.bin", 2, 100, 100, true, 0, false,
                    ExternalReferenceStatus: status), level: 0);

            Assert.IsFalse(vm.HasExternalLinkBadge, $"Status '{status}' must not badge the row.");
            Assert.AreEqual(string.Empty, vm.ExternalLinkBadgeText);
        }
    }

    [TestMethod]
    public void IndeterminateExternalReferences_ProduceNoBadgeText()
    {
        var vm = new TreeItemViewModel(
            new TreeNodeData(1, 0, "mystery.bin", 2, 100, 100, true, 0, false,
                ObservedAliasCount: 3,
                ExternalReferenceStatus: ExternalReference.Indeterminate), level: 0);

        Assert.AreEqual(string.Empty, vm.ExternalLinkBadgeText);
        Assert.IsFalse(vm.HasExternalLinkBadge);
        // Alias knowledge itself is still reported for the row
        Assert.IsTrue(vm.HasAliasBadge);
        Assert.AreEqual("×3 aliases", vm.AliasBadgeText);
    }

    [TestMethod]
    public void FormattedReferencedAllocated_DashWhenNotKnown_NeverZero()
    {
        var vm = new TreeItemViewModel(
            new TreeNodeData(1, 0, "unknown.bin", 2, 512, 0, AllocatedSizeKnown: false, 0, false), level: 0);

        Assert.AreEqual("-", vm.FormattedReferencedAllocated);
        Assert.AreEqual("-", vm.FormattedUniqueAllocated);
        Assert.AreNotEqual("0 B", vm.FormattedReferencedAllocated);
    }

    [TestMethod]
    public void FormattedSizes_MatchCanonicalMetrics_WhenKnown()
    {
        var vm = new TreeItemViewModel(
            new TreeNodeData(
                1, 0, "dir", 1,
                LogicalBytes: 1024,
                ReferencedAllocatedBytes: 4096,
                AllocatedSizeKnown: true,
                ChildCount: 2,
                HasChildren: true,
                UniqueAllocatedBytes: 2048), level: 0);

        Assert.AreEqual("1 KB", vm.FormattedLogicalSize);
        Assert.AreEqual("4 KB", vm.FormattedReferencedAllocated);
        Assert.AreEqual("2 KB", vm.FormattedUniqueAllocated);
    }

    [TestMethod]
    public void CanonicalProperties_FlowFromTreeNodeData()
    {
        var vm = new TreeItemViewModel(
            new TreeNodeData(
                7, 2, "shared.dll", 2,
                LogicalBytes: 100,
                ReferencedAllocatedBytes: 200,
                AllocatedSizeKnown: true,
                ChildCount: 0,
                HasChildren: false,
                UniqueAllocatedBytes: 150,
                ObservedAliasCount: 4,
                TotalLinkCountStatus: LinkCountKnowledge.Known,
                TotalLinkCountValue: 4,
                ExternalReferenceStatus: ExternalReference.ConfirmedNone,
                KnownSubtotalAllocatedBytes: 150), level: 1);

        Assert.AreEqual(7u, vm.Id);
        Assert.AreEqual(2u, vm.ParentId);
        Assert.AreEqual(100UL, vm.LogicalBytes);
        Assert.AreEqual(200UL, vm.ReferencedAllocatedBytes);
        Assert.AreEqual(150UL, vm.UniqueAllocatedBytes);
        Assert.AreEqual(4u, vm.ObservedAliasCount);
        Assert.AreEqual("known", vm.TotalLinkCountStatus);
        Assert.AreEqual((uint?)4, vm.TotalLinkCountValue);
        Assert.AreEqual("confirmed_none", vm.ExternalReferenceStatus);
        Assert.AreEqual(150UL, vm.KnownSubtotalAllocatedBytes);
    }

    [TestMethod]
    public void AutomationName_IncludesReferencedAllocated_AndMentionsAliasesAndExternalLink()
    {
        var plain = new TreeItemViewModel(
            new TreeNodeData(1, 0, "dir", 1, 1024, 4096, true, 2, true), level: 0);
        Assert.AreEqual("dir, 1 KB, 4 KB, 2 items", plain.AutomationName);

        var decorated = new TreeItemViewModel(
            new TreeNodeData(
                2, 0, "file.txt", 2,
                LogicalBytes: 1024,
                ReferencedAllocatedBytes: 4096,
                AllocatedSizeKnown: true,
                ChildCount: 0,
                HasChildren: false,
                ObservedAliasCount: 2,
                ExternalReferenceStatus: ExternalReference.ConfirmedExternal), level: 0);

        Assert.AreEqual("file.txt, 1 KB, 4 KB, file, 2 aliases, external link", decorated.AutomationName);
    }

    [TestMethod]
    public void AutomationName_OmitsAliasAndExternalClauses_WhenNotApplicable()
    {
        var vm = new TreeItemViewModel(
            new TreeNodeData(1, 0, "plain.txt", 2, 512, 512, true, 0, false), level: 0);

        StringAssert.Contains(vm.AutomationName, "plain.txt");
        Assert.IsFalse(vm.AutomationName.Contains("aliases", StringComparison.Ordinal));
        Assert.IsFalse(vm.AutomationName.Contains("external link", StringComparison.Ordinal));
    }

    [TestMethod]
    public void TreeItemViewModel_TotalLinkCountValue_NullableUint_PreservesNullAndExplicitZero()
    {
        var omitted = new TreeItemViewModel(
            new TreeNodeData(1, 0, "file.txt", 2, 100, 100, true, 0, false, TotalLinkCountStatus: LinkCountKnowledge.NotObserved, TotalLinkCountValue: null), level: 0);
        Assert.IsNull(omitted.TotalLinkCountValue);

        var knownZero = new TreeItemViewModel(
            new TreeNodeData(2, 0, "corrupt.txt", 2, 100, 100, true, 0, false, TotalLinkCountStatus: LinkCountKnowledge.Known, TotalLinkCountValue: 0u), level: 0);
        Assert.AreEqual((uint?)0, knownZero.TotalLinkCountValue);
    }
}
