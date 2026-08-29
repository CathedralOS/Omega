use omega_optimization_core::Optimization;

/// Canonical post-allocation machine-rule order.
pub const ORDERED_POST_ALLOCATION_MACHINE_RULES: [Optimization; 3] = [
    Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    Optimization::X86SelectXorZeroI64MaterializationV1,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostAllocationMachineRuleCatalogError {
    MissingSelection,
    UnsupportedSelection(Optimization),
    UnsupportedComposition(Optimization),
}
