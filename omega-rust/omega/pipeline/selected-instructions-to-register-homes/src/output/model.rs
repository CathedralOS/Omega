use crate::{
    SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;

use crate::{
    OptimizedActiveResidentRematerializationError, OptimizedPostCopyRegisterHomeCustodyError,
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError,
};

/// A temporary borrow of the current allocated program, not another program
/// representation or a container of preceding stages. All fields were joined
/// by independent replay in the allocation phase.
#[derive(Clone)]
pub struct AllocationOutput<'program> {
    pub(super) program: register_homes::AllocatedProgramRef<'program>,
    pub(super) selected: SelectedProgramRef<'program>,
    pub(super) liveness: &'program ValidatedLiveness,
    pub(super) ranges: &'program ValidatedLiveRanges,
    pub(super) legality: &'program ValidatedAllocationLegality,
    pub(super) homes: &'program ValidatedRegisterHomes,
    pub(super) manifest: &'program ValidatedPostAllocationOptimizationManifest,
    pub(super) environment: &'program ValidatedTargetRegisterEnvironment,
    pub(super) evidence: AllocationEvidence,
    pub(super) target_input: &'program std::sync::Arc<
        abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    >,
    pub(super) selections: &'program OptimizationSelections,
    pub(super) budget: OptimizationWorkBudget,
}

impl<'program> AllocationOutput<'program> {
    pub const fn program(&self) -> register_homes::AllocatedProgramRef<'program> {
        self.program
    }

    /// The selected program's borrow remains tied to the retained input, not this view.
    pub fn selected_plan(&self) -> &'program selected_instructions::SelectedInstructionPlan {
        self.program.selected
    }

    /// Earlier target/proof input retained for independent downstream joins.
    pub fn target_input(
        &self,
    ) -> &'program abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations
    {
        self.target_input
    }

    /// Share the exact proof input without retaining allocation history.
    pub fn target_input_owner(
        &self,
    ) -> &'program std::sync::Arc<
        abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    > {
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

pub use register_homes::AllocationEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationReplayError {
    CurrentProgramMismatch,
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

impl From<std::convert::Infallible> for AllocationReplayError {
    fn from(error: std::convert::Infallible) -> Self {
        match error {}
    }
}
