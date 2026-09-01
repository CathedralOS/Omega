use omega_optimization_core::{Optimization, OptimizationCatalogDescriptor};
use omega_target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationMachineRuleKind {
    Aarch64Cbnz,
    Aarch64Movn,
    X86XorZero,
    X86MovR32Imm32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostAllocationMachineRuleCatalogPayload {
    architecture: Architecture,
    kind: PostAllocationMachineRuleKind,
}

impl PostAllocationMachineRuleCatalogPayload {
    const fn new(architecture: Architecture, kind: PostAllocationMachineRuleKind) -> Self {
        Self { architecture, kind }
    }

    pub const fn architecture(self) -> Architecture {
        self.architecture
    }

    pub const fn kind(self) -> PostAllocationMachineRuleKind {
        self.kind
    }
}

pub type PostAllocationMachineRuleCatalogEntry =
    OptimizationCatalogDescriptor<PostAllocationMachineRuleCatalogPayload>;

/// Canonical post-allocation machine-rule order.
pub const POST_ALLOCATION_MACHINE_RULE_CATALOG: [PostAllocationMachineRuleCatalogEntry; 4] = [
    PostAllocationMachineRuleCatalogEntry::new(
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        PostAllocationMachineRuleCatalogPayload::new(
            Architecture::Aarch64,
            PostAllocationMachineRuleKind::Aarch64Cbnz,
        ),
    ),
    PostAllocationMachineRuleCatalogEntry::new(
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        PostAllocationMachineRuleCatalogPayload::new(
            Architecture::Aarch64,
            PostAllocationMachineRuleKind::Aarch64Movn,
        ),
    ),
    PostAllocationMachineRuleCatalogEntry::new(
        Optimization::X86SelectXorZeroI64MaterializationV1,
        PostAllocationMachineRuleCatalogPayload::new(
            Architecture::X86_64,
            PostAllocationMachineRuleKind::X86XorZero,
        ),
    ),
    PostAllocationMachineRuleCatalogEntry::new(
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        PostAllocationMachineRuleCatalogPayload::new(
            Architecture::X86_64,
            PostAllocationMachineRuleKind::X86MovR32Imm32,
        ),
    ),
];

/// Compatibility order view derived from the owning descriptor table.
pub const ORDERED_POST_ALLOCATION_MACHINE_RULES: [Optimization; 4] = [
    POST_ALLOCATION_MACHINE_RULE_CATALOG[0].optimization(),
    POST_ALLOCATION_MACHINE_RULE_CATALOG[1].optimization(),
    POST_ALLOCATION_MACHINE_RULE_CATALOG[2].optimization(),
    POST_ALLOCATION_MACHINE_RULE_CATALOG[3].optimization(),
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
