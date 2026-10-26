//! Optimizer module role: stage group. Current allocation facts.
//!
//! Every route publishes the same program and view. The staged producer is
//! projected once into owned current facts and then dropped; no later stage
//! inspects or replays allocation history.

/// The `AllocationOutput` every `project_allocation` impl publishes: the
/// staged producer's current facts plus the placed program view it selected
/// and the per-route evidence variant — those three expressions are the only
/// parts that differ, so they arrive as arguments.
macro_rules! allocation_output {
    ($self:expr, $placed:expr, $selected:expr, $evidence:expr) => {
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: $placed,
                homes: $self.homes().plan(),
            },
            selected: $selected,
            liveness: $self.liveness(),
            ranges: $self.ranges(),
            legality: $self.legality(),
            homes: $self.homes(),
            manifest: $self.post_allocation_manifest(),
            environment: $self.register_environment(),
            target_input: $self.optimized_target_owner(),
            selections: $self.selections(),
            budget: $self.budget_per_pass(),
            evidence: $evidence,
        }
    };
}

mod baseline;
mod current;
mod fixed_view;
mod literal_folds;
mod pre_allocation;
mod rematerialization;
mod retained;
mod runtime_spill;

use crate::{
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

// Each staged allocation result projects its current facts into the one
// view every route publishes. Only `RetainedAllocation`'s constructors, which
// also check the exercised recovery against the declared selection, use it.
trait ProjectAllocation {
    fn project_allocation(&self) -> AllocationOutput<'_>;
}

/// A temporary borrow of the current allocated program, not another program
/// representation or a container of preceding stages. Each field is a fact
/// the allocation stage produced and checked when it produced it.
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
    receipt_accessor_rows!(ref AllocationOutput {
        program: register_homes::AllocatedProgramRef<'program>,
    });

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

    receipt_accessor_rows!(ref AllocationOutput {
    /// Exact retained build policy admitted with the allocation.
        selections: &'program OptimizationSelections,
    });
    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget
    }
    receipt_accessor_rows!(borrow AllocationOutput {
        selected: &SelectedProgramRef<'_>,
    });
    receipt_accessor_rows!(ref AllocationOutput {
        liveness: &'program ValidatedLiveness,
        ranges: &'program ValidatedLiveRanges,
        legality: &'program ValidatedAllocationLegality,
        homes: &'program ValidatedRegisterHomes,
    });
    pub const fn post_allocation_manifest(
        &self,
    ) -> &'program ValidatedPostAllocationOptimizationManifest {
        self.manifest
    }
    pub const fn register_environment(&self) -> &'program ValidatedTargetRegisterEnvironment {
        self.environment
    }
    receipt_accessor_rows!(borrow AllocationOutput {
        evidence: &AllocationEvidence,
    });
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

/// Why a staged allocation result could not become the retained allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationAdmissionError {
    /// The exercised recovery does not match the declared allocation-recovery
    /// selection: a declared rule did not run, or an undeclared one did.
    SelectionMismatch,
}

impl std::fmt::Display for AllocationAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "allocation admission failed: {self:?}")
    }
}

impl std::error::Error for AllocationAdmissionError {}
