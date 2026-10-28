//! Optimizer module role: executable entrance. Register-home staging after selected-lowering transformations.
//!
//! One-step literal-fold chains and complete selected-lowering runs retain
//! distinct source custody; each constructs its homes and manifest once.

mod construction;
mod projection;

use crate::{
    RegisterHomeError, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use register_homes::PostAllocationOptimizationManifestError;
use selected_instructions_to_selected_instructions::{
    OptimizedLiteralFoldCustodyError, OptimizedPreAllocationCustodyError, SelectedProgramRef,
    StagedOptimizedLiteralFoldCustodyReceipt, StagedOptimizedLiteralFolds,
    StagedPreAllocationOptimizationCustodyReceipt, StagedPreAllocationOptimizationRun,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
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
            self.$field.selections()
        }
        /// The per-pass work budget admitted beside the source's evidence.
        pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
            self.$field.source_legality_stage().budget_per_pass()
        }
        assignment_context_accessors!(
            $field
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
        );
    };
}

/// Strict homes after one or more separately requested literal folds. The
/// complete append-only fold chain remains owned and the manifest ledger is
/// derived from it rather than accepted from a caller.
#[derive(Debug)]
pub(crate) struct StagedOptimizedRegisterHomesAfterLiteralFolds {
    folds: StagedOptimizedLiteralFolds,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterLiteralFolds {
    /// The retained fold stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    /// The final folded program this assignment describes.
    pub fn selected(
        &self,
    ) -> &selected_instructions_to_selected_instructions::ValidatedLiteralFold {
        self.folds.final_step().fold()
    }
    /// The reanalyzed facts over the final folded program.
    pub fn liveness(&self) -> &ValidatedLiveness {
        self.folds.final_step().liveness()
    }
    pub fn ranges(&self) -> &ValidatedLiveRanges {
        self.folds.final_step().ranges()
    }
    pub fn legality(&self) -> &ValidatedAllocationLegality {
        self.folds.final_step().legality()
    }
    /// The register environment admitted with the fold's source legality —
    /// folds preserve it, so the source's environment is the current one.
    pub const fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.folds.source_legality_stage().register_environment()
    }
    /// The governing optimizer selections admitted with the source stage.
    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.folds.source_legality_stage().selections()
    }
    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.folds.source_legality_stage().budget_per_pass()
    }
    assignment_context_accessors!(
        folds
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
    );
    receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterLiteralFolds {
        custody: &StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    });
}

/// The literal-fold route's receipt: the fold chain's custody, over the one
/// post-home shape every staging route publishes.
pub type StagedOptimizedPostLiteralFoldHomeCustodyReceipt =
    PostHomeCustodyReceipt<StagedOptimizedLiteralFoldCustodyReceipt>;

/// Strict homes after a complete named selected-lowering suite. The suite's
/// completion identity is retained even when its transformation ledger is
/// empty because the source was already at the validated fixed point.
#[derive(Debug)]
pub(crate) struct StagedOptimizedRegisterHomesAfterSelectedLowering {
    run: StagedSelectedLoweringOptimizationRun,
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
        match self.run.steps().last() {
            Some(step) => SelectedProgramRef::new(step.fold()),
            None => SelectedProgramRef::new(self.run.source_legality_stage().selected()),
        }
    }
    /// The facts over the current program: the last step's rebuilt analyses,
    /// or the run's admitted analyses when the suite stayed at the fixed
    /// point.
    pub fn liveness(&self) -> &ValidatedLiveness {
        match self.run.steps().last() {
            Some(step) => step.liveness(),
            None => self.run.source_legality_stage().liveness(),
        }
    }
    pub fn ranges(&self) -> &ValidatedLiveRanges {
        match self.run.steps().last() {
            Some(step) => step.ranges(),
            None => self.run.source_legality_stage().ranges(),
        }
    }
    pub fn legality(&self) -> &ValidatedAllocationLegality {
        match self.run.steps().last() {
            Some(step) => step.legality(),
            None => self.run.source_legality_stage().legality(),
        }
    }
    source_evidence_accessors!(run);
    receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterSelectedLowering {
        custody: &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    });
}

/// The selected-lowering route's receipt: the lowering run's custody, over the
/// one post-home shape every staging route publishes.
pub type StagedOptimizedPostSelectedLoweringHomeCustodyReceipt =
    PostHomeCustodyReceipt<StagedSelectedLoweringOptimizationCustodyReceipt>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostSelectedLoweringHomeCustodyError {
    UpstreamSelectedLowering(OptimizedLiteralFoldCustodyError),
    Assignment(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostSelectedLoweringHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-selected-lowering home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostSelectedLoweringHomeCustodyError {}

/// Strict homes after a completed pre-allocation suite. The suite's
/// completion identity is retained even when its transformation ledger is
/// empty because every candidate declined on the admitted source program.
#[derive(Debug)]
pub(crate) struct StagedOptimizedRegisterHomesAfterPreAllocation {
    run: StagedPreAllocationOptimizationRun,
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
    PostHomeCustodyReceipt<StagedPreAllocationOptimizationCustodyReceipt>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostPreAllocationHomeCustodyError {
    UpstreamPreAllocation(OptimizedPreAllocationCustodyError),
    Assignment(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostPreAllocationHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-pre-allocation home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostPreAllocationHomeCustodyError {}
