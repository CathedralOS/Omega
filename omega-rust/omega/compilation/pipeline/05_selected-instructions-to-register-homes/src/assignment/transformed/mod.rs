//! Optimizer module role: executable entrance. Register-home staging after selected-lowering transformations.
//!
//! One-step literal-fold chains and complete selected-lowering runs retain
//! distinct source custody; each constructs its homes and manifest once.

mod construction;
mod custody;
mod projection;

use crate::{
    RegisterHomeError, RegisterHomeIdentity, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes,
};
use optimization_core::PostAllocationOptimizationManifestIdentity;
use register_homes::PostAllocationOptimizationManifestError;
use selected_instructions_to_selected_instructions::{
    OptimizedLiteralFoldCustodyError, OptimizedPreAllocationCustodyError, SelectedProgramRef,
    StagedOptimizedLiteralFoldCustodyReceipt, StagedOptimizedLiteralFolds,
    StagedPreAllocationOptimizationCustodyReceipt, StagedPreAllocationOptimizationRun,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
};

/// Assignment probe over a completed selected-lowering run's final facts.
/// Computing the assignment without consuming the run lets the route answer
/// residual `NoCompatibleHome` pressure with runtime spill instead of
/// post-lowering homes.
pub(crate) fn assign_optimized_register_homes_after_selected_lowering(
    run: &StagedSelectedLoweringOptimizationRun,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    construction::assign_register_homes_after_selected_lowering(run)
}

/// The staged-homes form over an already-computed assignment, for the
/// composing route that probed assignment while it still owned the run.
pub(crate) fn stage_optimized_register_homes_after_selected_lowering_with_assignment(
    run: StagedSelectedLoweringOptimizationRun,
    homes: ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let staged =
        construction::construct_register_homes_after_selected_lowering_with_assignment(run, homes)?;
    Ok(staged)
}

/// Assignment probe over a completed pre-allocation run's final facts.
/// Computing the assignment without consuming the run lets the route answer
/// residual `NoCompatibleHome` pressure with runtime spill instead of
/// post-run homes.
pub(crate) fn assign_optimized_register_homes_after_pre_allocation(
    run: &StagedPreAllocationOptimizationRun,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    construction::assign_register_homes_after_pre_allocation(run)
}

/// The staged-homes form over an already-computed assignment, for the
/// composing route that probed assignment while it still owned the run.
pub(crate) fn stage_optimized_register_homes_after_pre_allocation_with_assignment(
    run: StagedPreAllocationOptimizationRun,
    homes: ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterPreAllocation,
    OptimizedPostPreAllocationHomeCustodyError,
> {
    let staged =
        construction::construct_register_homes_after_pre_allocation_with_assignment(run, homes)?;
    Ok(staged)
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
    /// The retained optimized-target proof input, kept as replay evidence;
    /// downstream custody checks compare the owner handle by identity.
    pub fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.folds
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
    }
    assignment_context_accessors!();
    receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterLiteralFolds {
        custody: &StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
    source: StagedOptimizedLiteralFoldCustodyReceipt,
    homes: RegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
    receipt_accessor_rows!(borrow StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
        source: &StagedOptimizedLiteralFoldCustodyReceipt,
    });
    receipt_accessor_rows!(ref StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
        homes: RegisterHomeIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        function_count: usize,
        assignment_count: usize,
    });
}

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
    /// The register environment admitted with the run's source legality —
    /// the lowering suite preserves it, so the source's environment is the
    /// current one.
    pub const fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.run.source_legality_stage().register_environment()
    }
    /// The governing optimizer selections admitted with the run.
    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.run.selections()
    }
    /// The per-pass work budget admitted beside the source's evidence.
    pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.run.source_legality_stage().budget_per_pass()
    }
    /// The retained optimized-target proof input, kept as replay evidence;
    /// downstream custody checks compare the owner handle by identity.
    pub fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.run
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
    }
    assignment_context_accessors!();
    receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterSelectedLowering {
        custody: &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
    source: StagedSelectedLoweringOptimizationCustodyReceipt,
    homes: RegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
    receipt_accessor_rows!(borrow StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        source: &StagedSelectedLoweringOptimizationCustodyReceipt,
    });
    receipt_accessor_rows!(ref StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        homes: RegisterHomeIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        function_count: usize,
        assignment_count: usize,
    });
}

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
    /// The register environment admitted with the run's source legality —
    /// the pre-allocation families preserve it, so the source's environment
    /// is the current one.
    pub const fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.run.source_legality_stage().register_environment()
    }
    /// The governing optimizer selections admitted with the run.
    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.run.selections()
    }
    /// The per-pass work budget admitted beside the source's evidence.
    pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.run.source_legality_stage().budget_per_pass()
    }
    /// The retained optimized-target proof input, kept as replay evidence;
    /// downstream custody checks compare the owner handle by identity.
    pub fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.run
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
    }
    assignment_context_accessors!();
    receipt_accessor_rows!(borrow StagedOptimizedRegisterHomesAfterPreAllocation {
        custody: &StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostPreAllocationHomeCustodyReceipt {
    source: StagedPreAllocationOptimizationCustodyReceipt,
    homes: RegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedPostPreAllocationHomeCustodyReceipt {
    receipt_accessor_rows!(borrow StagedOptimizedPostPreAllocationHomeCustodyReceipt {
        source: &StagedPreAllocationOptimizationCustodyReceipt,
    });
    receipt_accessor_rows!(ref StagedOptimizedPostPreAllocationHomeCustodyReceipt {
        homes: RegisterHomeIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        function_count: usize,
        assignment_count: usize,
    });
}

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
