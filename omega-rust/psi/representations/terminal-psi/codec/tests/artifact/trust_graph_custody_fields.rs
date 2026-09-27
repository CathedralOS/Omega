//! Substitutable-field inventory for the canonical terminal trust graph.
//!
//! The family's declared fields are driven through the shared substitution
//! matrix by `trust_graph_custody.rs`; keeping the inventory in its own file
//! names the boundary between what is declared substitutable and the driver
//! that exercises every leg.

mutation_matrix::custody_field_inventory! {
    /// One substitutable field of the canonical terminal trust graph's
    /// admission input; each leg's verdict is checked by canonical admission
    /// and, for substitutions admission still accepts, the obligation-ledger
    /// replay bound to the honest graph.
    pub enum TrustGraphCustodyFieldForTest {
        EntryCleared,
        EntryUnknown,
        EntryNamesRegisteredRoot,
        NodeRosterReordered,
        NodeRosterDuplicated,
        NodeRosterMemberExtended,
        NonRootIdentityCleared,
        NonRootKindAlternate,
        NonRootKindRegisteredRoot,
        RootKindNonRoot,
        NonRootStatusAlternate,
        NonRootStatusRegistered,
        RootStatusDerived,
        NonRootPolicyAlternate,
        RootPolicyKernelChecked,
        NonRootSemanticSubjectSubstituted,
        NonRootSemanticSubjectCleared,
        NonRootVersionSubstituted,
        NonRootVersionCleared,
        NonRootOwnerSubstituted,
        NonRootOwnerCleared,
        NonRootScopeSubstituted,
        NonRootScopeCleared,
        NonRootRationaleSubstituted,
        NonRootRationaleCleared,
        DependentDependencyMemberAdded,
        DependentDependencyOrderReordered,
        DependentDependencySelfAdded,
        DependentDependencyUnknownAdded,
        DependentDependenciesCleared,
        RootDependencySet,
        DependencyCycleIntroduced,
        NonRootStatedSourcesSubstituted,
    }
}
