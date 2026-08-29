use omega_optimization_core::{Optimization, OptimizationCatalogDescriptor};
use omega_target::Architecture;

pub type PostAllocationMachineRuleCatalogEntry = OptimizationCatalogDescriptor<Architecture>;

/// Canonical post-allocation machine-rule order.
pub const POST_ALLOCATION_MACHINE_RULE_CATALOG: [PostAllocationMachineRuleCatalogEntry; 3] = [
    PostAllocationMachineRuleCatalogEntry::new(
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Architecture::Aarch64,
    ),
    PostAllocationMachineRuleCatalogEntry::new(
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        Architecture::Aarch64,
    ),
    PostAllocationMachineRuleCatalogEntry::new(
        Optimization::X86SelectXorZeroI64MaterializationV1,
        Architecture::X86_64,
    ),
];

/// Compatibility order view derived from the owning descriptor table.
pub const ORDERED_POST_ALLOCATION_MACHINE_RULES: [Optimization; 3] = [
    POST_ALLOCATION_MACHINE_RULE_CATALOG[0].optimization(),
    POST_ALLOCATION_MACHINE_RULE_CATALOG[1].optimization(),
    POST_ALLOCATION_MACHINE_RULE_CATALOG[2].optimization(),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostAllocationMachineRuleCatalogError {
    MissingSelection,
    UnsupportedSelection(Optimization),
    UnsupportedComposition(Optimization),
    UnsupportedTarget {
        optimization: Optimization,
        required: Architecture,
        actual: Architecture,
    },
}
