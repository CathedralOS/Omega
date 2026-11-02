//! Optimizer module role: executable entrance. Register-home staging after selected-lowering transformations.
//!
//! Completed selected-lowering and pre-allocation suites arrive as their
//! final program, rebuilt facts and ledger; each route constructs its homes
//! and manifest once over them.

mod construction;
mod projection;

use crate::{
    PreAllocationLedger, PreAllocationResult, SelectedLoweringLedger, SelectedLoweringResult,
};
use crate::{
    RegisterHomeError, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use register_homes::PostAllocationOptimizationManifestError;
use selected_instruction_analyses::{
    SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
};

use super::PostHomeCustodyReceipt;

pub(crate) use construction::{
    assign_register_homes_after_pre_allocation as assign_optimized_register_homes_after_pre_allocation,
    assign_register_homes_after_selected_lowering as assign_optimized_register_homes_after_selected_lowering,
    stage_optimized_register_homes_after_pre_allocation_with_assignment,
    stage_optimized_register_homes_after_selected_lowering_with_assignment,
};

/// The source-evidence accessors every run-based route publishes: the
/// environment, selections and budget the admitted source legality settled,
/// plus the shared assignment-context rows.
macro_rules! source_evidence_accessors {
    ($field:ident) => {
        pub const fn register_environment(
            &self,
        ) -> &register_environment::ValidatedTargetRegisterEnvironment {
            self.$field.source_legality_stage().register_environment()
        }
        /// The governing optimizer selections admitted with the run.
        pub fn selections(&self) -> &optimization_core::OptimizationSelections {
            self.$field.source_legality_stage().selections()
        }
        /// The per-pass work budget admitted beside the source's evidence.
        pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
            self.$field.source_legality_stage().budget_per_pass()
        }
        assignment_context_accessors!();
    };
}

/// Strict homes after a complete named selected-lowering suite. The suite's
/// completion identity is retained even when its transformation ledger is
/// empty because the source was already at the validated fixed point.
#[derive(Debug)]
pub(crate) struct StagedOptimizedRegisterHomesAfterSelectedLowering {
    run: SelectedLoweringResult,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterSelectedLowering {
    /// The retained lowering run. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    /// The program this assignment describes: the last step's fold when the
    /// suite ran, otherwise the unchanged selected program the run admitted.
    pub fn selected(&self) -> SelectedProgramRef<'_> {
        self.run.current()
    }
    /// The facts over the current program: the last step's rebuilt analyses,
    /// or the run's admitted analyses when the suite stayed at the fixed
    /// point.
    pub fn liveness(&self) -> &ValidatedLiveness {
        self.run.liveness()
    }
    pub fn ranges(&self) -> &ValidatedLiveRanges {
        self.run.ranges()
    }
    pub fn legality(&self) -> &ValidatedAllocationLegality {
        self.run.legality()
    }
    source_evidence_accessors!(run);
    receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterSelectedLowering {
        custody: &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    });
}

/// The selected-lowering route's receipt: the lowering run's custody, over the
/// one post-home shape every staging route publishes.
pub type StagedOptimizedPostSelectedLoweringHomeCustodyReceipt =
    PostHomeCustodyReceipt<SelectedLoweringLedger>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostSelectedLoweringHomeCustodyError {
    Assignment(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

stage_error_display!(
    OptimizedPostSelectedLoweringHomeCustodyError,
    "optimized post-selected-lowering home staging failed"
);

/// Strict homes after a completed pre-allocation suite. The suite's
/// completion identity is retained even when its transformation ledger is
/// empty because every candidate declined on the admitted source program.
#[derive(Debug)]
pub(crate) struct StagedOptimizedRegisterHomesAfterPreAllocation {
    run: PreAllocationResult,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostPreAllocationHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterPreAllocation {
    /// The retained pre-allocation run. Replay and custody validation inspect
    /// it; ordinary consumers read the current program and analyses directly.
    /// The program this assignment describes: the run's current program —
    /// the last committed step's transformed plan, or the unchanged admitted
    /// source plan when the terminal pass was already clean.
    pub fn selected(&self) -> SelectedProgramRef<'_> {
        self.run.current()
    }
    /// The facts over the current program.
    pub fn liveness(&self) -> &ValidatedLiveness {
        self.run.liveness()
    }
    pub fn ranges(&self) -> &ValidatedLiveRanges {
        self.run.ranges()
    }
    pub fn legality(&self) -> &ValidatedAllocationLegality {
        self.run.legality()
    }
    source_evidence_accessors!(run);
    receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterPreAllocation {
        custody: &StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    });
}

/// The pre-allocation route's receipt: the run's custody, over the one
/// post-home shape every staging route publishes.
pub type StagedOptimizedPostPreAllocationHomeCustodyReceipt =
    PostHomeCustodyReceipt<PreAllocationLedger>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostPreAllocationHomeCustodyError {
    Assignment(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

stage_error_display!(
    OptimizedPostPreAllocationHomeCustodyError,
    "optimized post-pre-allocation home staging failed"
);
