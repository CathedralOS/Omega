use omega_optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use omega_regalloc::{
    SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

use crate::{
    OptimizedActiveResidentRematerializationError, OptimizedPostCopyRegisterHomeCustodyError,
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt,
};

/// A temporary borrow of the current allocated program, not another program
/// representation or a container of preceding stages. All fields were joined
/// by independent replay in the allocation phase.
#[derive(Clone)]
pub struct AllocationOutput<'program> {
    pub(super) selected: SelectedProgramRef<'program>,
    pub(super) liveness: &'program ValidatedLiveness,
    pub(super) ranges: &'program ValidatedLiveRanges,
    pub(super) legality: &'program ValidatedAllocationLegality,
    pub(super) homes: &'program ValidatedRegisterHomes,
    pub(super) manifest: &'program ValidatedPostAllocationOptimizationManifest,
    pub(super) environment: &'program ValidatedTargetRegisterEnvironment,
    pub(super) evidence: AllocationEvidence,
    pub(super) target_input: &'program omega_abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    pub(super) selections: &'program OptimizationSelections,
    pub(super) budget: OptimizationWorkBudget,
}

impl<'program> AllocationOutput<'program> {
    /// The selected program's borrow remains tied to the retained input, not this view.
    pub fn selected_plan(&self) -> &'program omega_selected_instructions::SelectedInstructionPlan {
        self.selected.plan()
    }

    /// Earlier target/proof input retained for independent downstream joins.
    pub const fn target_input(
        &self,
    ) -> &'program omega_abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations
    {
        self.target_input
    }

    /// Exact retained build policy, independently joined during allocation replay.
    pub const fn selections(&self) -> &'program OptimizationSelections {
        self.selections
    }
    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn selected(&self) -> &SelectedProgramRef<'_> {
        &self.selected
    }
    pub const fn liveness(&self) -> &'program ValidatedLiveness {
        self.liveness
    }
    pub const fn ranges(&self) -> &'program ValidatedLiveRanges {
        self.ranges
    }
    pub const fn legality(&self) -> &'program ValidatedAllocationLegality {
        self.legality
    }
    pub const fn homes(&self) -> &'program ValidatedRegisterHomes {
        self.homes
    }
    pub const fn post_allocation_manifest(
        &self,
    ) -> &'program ValidatedPostAllocationOptimizationManifest {
        self.manifest
    }
    pub const fn register_environment(&self) -> &'program ValidatedTargetRegisterEnvironment {
        self.environment
    }
    pub const fn evidence(&self) -> &AllocationEvidence {
        &self.evidence
    }
}

/// Evidence roles remain distinct; they do not choose the downstream program
/// representation or machine-plan implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationEvidence {
    RegisterHomes(StagedOptimizedRegisterHomeCustodyReceipt),
    FixedViewCopies(StagedOptimizedPostCopyRegisterHomeCustodyReceipt),
    LiteralFolds(StagedOptimizedPostLiteralFoldHomeCustodyReceipt),
    SelectedLowering(StagedOptimizedPostSelectedLoweringHomeCustodyReceipt),
    ActiveResidentRematerialization(StagedOptimizedActiveResidentRematerializationCustodyReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationReplayError {
    SelectionMismatch,
    RegisterHomes(OptimizedRegisterHomeCustodyError),
    FixedViewCopies(OptimizedPostCopyRegisterHomeCustodyError),
    LiteralFolds(OptimizedPostLiteralFoldHomeCustodyError),
    SelectedLowering(OptimizedPostSelectedLoweringHomeCustodyError),
    ActiveResidentRematerialization(OptimizedActiveResidentRematerializationError),
    ReceiptMismatch,
}

impl std::fmt::Display for AllocationReplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "allocation replay failed: {self:?}")
    }
}

impl std::error::Error for AllocationReplayError {}
