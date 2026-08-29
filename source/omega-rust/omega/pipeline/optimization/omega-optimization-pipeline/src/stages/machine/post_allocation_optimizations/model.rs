use omega_optimization_core::{Optimization, OptimizationSelectionIdentity};

use super::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedX86XorZeroMaterialization,
};

/// One independently validated result from the ordered post-allocation stage.
/// Complete compiler routes carry this value rather than adding a new route
/// type for every symbolic machine rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedOptimizedPostAllocationMachineOptimization {
    Aarch64Cbnz(StagedOptimizedAarch64CbnzFusion),
    Aarch64Movn(StagedOptimizedAarch64MovnMaterialization),
    X86XorZero(StagedOptimizedX86XorZeroMaterialization),
}

impl StagedOptimizedPostAllocationMachineOptimization {
    pub const fn optimization(&self) -> Optimization {
        match self {
            Self::Aarch64Cbnz(_) => Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Self::Aarch64Movn(_) => {
                Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
            }
            Self::X86XorZero(_) => Optimization::X86SelectXorZeroI64MaterializationV1,
        }
    }

    pub const fn selections(&self) -> OptimizationSelectionIdentity {
        match self {
            Self::Aarch64Cbnz(staged) => staged.custody().selections(),
            Self::Aarch64Movn(staged) => staged.custody().selections(),
            Self::X86XorZero(staged) => staged.custody().selections(),
        }
    }

    pub const fn source(&self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        match self {
            Self::Aarch64Cbnz(staged) => staged.custody().source(),
            Self::Aarch64Movn(staged) => staged.custody().source(),
            Self::X86XorZero(staged) => staged.custody().source(),
        }
    }

    pub const fn action_count(&self) -> usize {
        match self {
            Self::Aarch64Cbnz(staged) => staged.custody().action_count(),
            Self::Aarch64Movn(staged) => staged.custody().action_count(),
            Self::X86XorZero(staged) => staged.custody().action_count(),
        }
    }
}
