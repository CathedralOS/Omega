use omega_optimization_core::{Optimization, OptimizationSelectionIdentity};

use super::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedX86XorZeroMaterialization,
};

/// Rule-independent evidence retained by later physical stages.
///
/// The typed optimization result remains the authority for replay. This compact
/// view lets encoding, layout, and realization bind that authority without
/// growing one optional field or one complete route per rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostAllocationMachineOptimizationCustody {
    optimization: Optimization,
    artifact_identity: [u8; 32],
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: omega_machine_optimizer::PostAllocationMachineIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl PostAllocationMachineOptimizationCustody {
    pub(crate) const fn from_parts(
        optimization: Optimization,
        artifact_identity: [u8; 32],
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: omega_machine_optimizer::PostAllocationMachineIdentity,
        action_count: usize,
        baseline_bytes: u64,
        selected_bytes: u64,
    ) -> Self {
        Self {
            optimization,
            artifact_identity,
            selections,
            post_allocation_machine_selections,
            source,
            action_count,
            baseline_bytes,
            selected_bytes,
        }
    }

    pub const fn optimization(self) -> Optimization {
        self.optimization
    }

    pub const fn artifact_identity(self) -> [u8; 32] {
        self.artifact_identity
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn source(self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.source
    }

    pub const fn action_count(self) -> usize {
        self.action_count
    }

    pub const fn baseline_bytes(self) -> u64 {
        self.baseline_bytes
    }

    pub const fn selected_bytes(self) -> u64 {
        self.selected_bytes
    }

    pub const fn expected_byte_savings(self) -> Option<u64> {
        self.baseline_bytes.checked_sub(self.selected_bytes)
    }
}

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
        };
        Some(PostAllocationMachineOptimizationCustody {
            optimization: self.optimization(),
            artifact_identity,
            selections,
            post_allocation_machine_selections: post_selections,
            source,
            action_count,
            baseline_bytes,
            selected_bytes,
        })
    }

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
