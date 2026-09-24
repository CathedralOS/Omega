//! Optimizer module role: stage group. Current allocation facts and separate replay evidence.
//!
//! Current program data is owned independently of retained replay inputs.
//! Every route publishes the same program and view; only replay inspects history.

mod baseline;
mod current;
mod fixed_view;
mod literal_folds;
mod pre_allocation;
mod rematerialization;
mod retained;
mod runtime_spill;

use crate::{
    OptimizedActiveResidentRematerializationError, OptimizedPostCopyRegisterHomeCustodyError,
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostPreAllocationHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes,
};
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
pub use retained::RetainedAllocation;
use selected_instructions_to_selected_instructions::{
    SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
};

mod sealed {
    pub trait Sealed {}
}

/// Sealed allocation boundary. Fresh inputs independently reconstruct all source
/// and rewrite evidence. Privately owned immutable retained inputs may reuse that
/// admission while still checking the complete current-program join.
pub trait AllocationSource: sealed::Sealed {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError>;
}

impl sealed::Sealed for AllocationOutput<'_> {}

impl AllocationSource for AllocationOutput<'_> {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        // Construction has replayed the source. Immutable borrows keep every
        // joined input fixed while subsequent stages use this validated view.
        Ok(self.clone())
    }
}

impl<Source: AllocationSource + ?Sized> sealed::Sealed for Box<Source> {}

impl<Source: AllocationSource + ?Sized> AllocationSource for Box<Source> {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        self.as_ref().replay_allocation()
    }
}

// Projection alone grants no admission. Only replay or the immutable retained
// carrier's checked construction may expose these facts outside this owner.
trait ProjectAllocation {
    fn project_allocation(&self) -> AllocationOutput<'_>;
}

/// A temporary borrow of the current allocated program, not another program
/// representation or a container of preceding stages. All fields were joined
/// by independent replay in the allocation phase.
#[derive(Clone)]
pub struct AllocationOutput<'program> {
    program: register_homes::AllocatedProgramRef<'program>,
    selected: SelectedProgramRef<'program>,
    liveness: &'program ValidatedLiveness,
    ranges: &'program ValidatedLiveRanges,
    legality: &'program ValidatedAllocationLegality,
    homes: &'program ValidatedRegisterHomes,
    manifest: &'program ValidatedPostAllocationOptimizationManifest,
    environment: &'program ValidatedTargetRegisterEnvironment,
    evidence: AllocationEvidence,
    target_input: &'program std::sync::Arc<
        abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    >,
    selections: &'program OptimizationSelections,
    budget: OptimizationWorkBudget,
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

/// Evidence roles remain distinct; they do not choose the downstream program
/// representation or machine-plan implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationEvidence {
    RuntimeSpill(optimization_core::PostAllocationOptimizationManifestIdentity),
    RegisterHomes(StagedOptimizedRegisterHomeCustodyReceipt),
    FixedViewCopies(StagedOptimizedPostCopyRegisterHomeCustodyReceipt),
    LiteralFolds(StagedOptimizedPostLiteralFoldHomeCustodyReceipt),
    SelectedLowering(StagedOptimizedPostSelectedLoweringHomeCustodyReceipt),
    PreAllocation(StagedOptimizedPostPreAllocationHomeCustodyReceipt),
    ActiveResidentRematerialization(StagedOptimizedActiveResidentRematerializationCustodyReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationReplayError {
    RuntimeSpill(crate::RuntimeSpillAllocationError),
    CurrentProgramMismatch,
    SelectionMismatch,
    RegisterHomes(OptimizedRegisterHomeCustodyError),
    FixedViewCopies(OptimizedPostCopyRegisterHomeCustodyError),
    LiteralFolds(OptimizedPostLiteralFoldHomeCustodyError),
    SelectedLowering(OptimizedPostSelectedLoweringHomeCustodyError),
    PreAllocation(OptimizedPostPreAllocationHomeCustodyError),
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
