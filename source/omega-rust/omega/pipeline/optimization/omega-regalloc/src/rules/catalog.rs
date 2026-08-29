use omega_optimization_core::Optimization;

/// Canonical allocation-recovery rule order.
pub const ORDERED_ALLOCATION_RECOVERY_RULES: [Optimization; 2] = [
    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
    Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
];

/// Canonical selected-lowering rule order. Their combined execution is an
/// explicit composition of these exact names, not another source-visible name.
pub const ORDERED_SELECTED_LOWERING_RULES: [Optimization; 2] = [
    Optimization::SelectedIncomingU12ExactAddImmediate,
    Optimization::SelectedIncomingU12ExactSubtractImmediate,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationRecoveryRuleCatalogError {
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedLoweringRuleCatalogError {
    MissingSelection,
    UnsupportedSelection(Optimization),
}
