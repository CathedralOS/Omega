use optimization_core::{Optimization, OptimizationSelectionIdentity};

use super::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedAarch64SameViewCopyElision, StagedOptimizedX86MovR32Imm32Materialization,
    StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
    StagedOptimizedX86XorZeroMaterialization,
};

pub use physical_instructions::PostAllocationMachineOptimizationCustody;

/// One independently validated result from the ordered post-allocation stage.
/// Complete compiler routes carry this value rather than adding a new route
/// type for every symbolic machine rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedOptimizedPostAllocationMachineOptimization {
    Aarch64Cbnz(StagedOptimizedAarch64CbnzFusion),
    Aarch64Movn(StagedOptimizedAarch64MovnMaterialization),
    Aarch64SameViewCopyElision(StagedOptimizedAarch64SameViewCopyElision),
    X86MovR32Imm32(StagedOptimizedX86MovR32Imm32Materialization),
    X86MovR64Imm32SignExtended(StagedOptimizedX86MovR64Imm32SignExtendedMaterialization),
    X86XorZero(StagedOptimizedX86XorZeroMaterialization),
}

impl StagedOptimizedPostAllocationMachineOptimization {
    pub fn custody(&self) -> Option<PostAllocationMachineOptimizationCustody> {
        let (
            artifact_identity,
            selections,
            post_selections,
            source,
            action_count,
            baseline_bytes,
            selected_bytes,
        ) = match self {
            Self::Aarch64Cbnz(staged) => {
                let receipt = staged.custody();
                let actions = u64::try_from(receipt.action_count()).ok()?;
                (
                    receipt.fusion().bytes(),
                    receipt.selections(),
                    receipt.post_allocation_machine_selections(),
                    receipt.source(),
                    receipt.action_count(),
                    actions.checked_mul(8)?,
                    actions.checked_mul(4)?,
                )
            }
            Self::Aarch64Movn(staged) => {
                let receipt = staged.custody();
                (
                    receipt.materialization().bytes(),
                    receipt.selections(),
                    receipt.post_allocation_machine_selections(),
                    receipt.source(),
                    receipt.action_count(),
                    receipt.baseline_words().checked_mul(4)?,
                    receipt.selected_words().checked_mul(4)?,
                )
            }
            Self::Aarch64SameViewCopyElision(staged) => {
                let receipt = staged.custody();
                let actions = u64::try_from(receipt.action_count()).ok()?;
                (
                    receipt.elision().bytes(),
                    receipt.selections(),
                    receipt.post_allocation_machine_selections(),
                    receipt.source(),
                    receipt.action_count(),
                    actions.checked_mul(4)?,
                    0,
                )
            }
            Self::X86XorZero(staged) => {
                let receipt = staged.custody();
                (
                    receipt.materialization().bytes(),
                    receipt.selections(),
                    receipt.post_allocation_machine_selections(),
                    receipt.source(),
                    receipt.action_count(),
                    receipt.baseline_bytes(),
                    receipt.selected_bytes(),
                )
            }
            Self::X86MovR32Imm32(staged) => {
                let receipt = staged.custody();
                (
                    receipt.materialization().bytes(),
                    receipt.selections(),
                    receipt.post_allocation_machine_selections(),
                    receipt.source(),
                    receipt.action_count(),
                    receipt.baseline_bytes(),
                    receipt.selected_bytes(),
                )
            }
            Self::X86MovR64Imm32SignExtended(staged) => {
                let receipt = staged.custody();
                (
                    receipt.materialization().bytes(),
                    receipt.selections(),
                    receipt.post_allocation_machine_selections(),
                    receipt.source(),
                    receipt.action_count(),
                    receipt.baseline_bytes(),
                    receipt.selected_bytes(),
                )
            }
        };
        Some(PostAllocationMachineOptimizationCustody::from_parts(
            self.optimization(),
            artifact_identity,
            selections,
            post_selections,
            source,
            action_count,
            baseline_bytes,
            selected_bytes,
        ))
    }

    pub const fn optimization(&self) -> Optimization {
        match self {
            Self::Aarch64Cbnz(_) => Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Self::Aarch64Movn(_) => {
                Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
            }
            Self::Aarch64SameViewCopyElision(staged) => staged.custody().optimization(),
            Self::X86XorZero(_) => Optimization::X86SelectXorZeroI64MaterializationV1,
            Self::X86MovR32Imm32(_) => {
                Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
            }
            Self::X86MovR64Imm32SignExtended(_) => {
                Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1
            }
        }
    }

    pub const fn selections(&self) -> OptimizationSelectionIdentity {
        match self {
            Self::Aarch64Cbnz(staged) => staged.custody().selections(),
            Self::Aarch64Movn(staged) => staged.custody().selections(),
            Self::Aarch64SameViewCopyElision(staged) => staged.custody().selections(),
            Self::X86XorZero(staged) => staged.custody().selections(),
            Self::X86MovR32Imm32(staged) => staged.custody().selections(),
            Self::X86MovR64Imm32SignExtended(staged) => staged.custody().selections(),
        }
    }

    pub const fn source(&self) -> physical_instructions::PostAllocationMachineIdentity {
        match self {
            Self::Aarch64Cbnz(staged) => staged.custody().source(),
            Self::Aarch64Movn(staged) => staged.custody().source(),
            Self::Aarch64SameViewCopyElision(staged) => staged.custody().source(),
            Self::X86XorZero(staged) => staged.custody().source(),
            Self::X86MovR32Imm32(staged) => staged.custody().source(),
            Self::X86MovR64Imm32SignExtended(staged) => staged.custody().source(),
        }
    }

    pub const fn action_count(&self) -> usize {
        match self {
            Self::Aarch64Cbnz(staged) => staged.custody().action_count(),
            Self::Aarch64Movn(staged) => staged.custody().action_count(),
            Self::Aarch64SameViewCopyElision(staged) => staged.custody().action_count(),
            Self::X86XorZero(staged) => staged.custody().action_count(),
            Self::X86MovR32Imm32(staged) => staged.custody().action_count(),
            Self::X86MovR64Imm32SignExtended(staged) => staged.custody().action_count(),
        }
    }
}
