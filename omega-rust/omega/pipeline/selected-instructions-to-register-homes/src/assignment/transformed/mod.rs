//! Optimizer module role: executable entrance. Register-home staging after selected-lowering transformations.
//!
//! One-step literal-fold chains and complete selected-lowering runs retain
//! distinct source custody. This entrance grants either result custody only
//! after independent home and manifest replay.

mod construction;
mod custody;
mod projection;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::{
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
};

use crate::{
    OptimizedLiteralFoldCustodyError, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedSelectedLoweringOptimizationCustodyReceipt,
};
use crate::{
    PostAllocationOptimizationManifestError, RegisterHomeError, RegisterHomeIdentity,
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use crate::{StagedOptimizedLiteralFolds, StagedSelectedLoweringOptimizationRun};
use optimization_core::PostAllocationOptimizationManifestIdentity;

pub fn stage_optimized_register_homes_after_literal_folds(
    folds: StagedOptimizedLiteralFolds,
) -> Result<StagedOptimizedRegisterHomesAfterLiteralFolds, OptimizedPostLiteralFoldHomeCustodyError>
{
    let staged = construction::construct_register_homes_after_literal_folds(folds)?;
    validate_optimized_register_home_after_literal_fold_custody(&staged)?;
    Ok(staged)
}

pub fn stage_optimized_register_homes_after_selected_lowering(
    run: StagedSelectedLoweringOptimizationRun,
) -> Result<
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let staged = construction::construct_register_homes_after_selected_lowering(run)?;
    validate_optimized_register_home_after_selected_lowering_custody(&staged)?;
    Ok(staged)
}

/// Strict homes after one or more separately requested literal folds. The
/// complete append-only fold chain remains owned and the manifest ledger is
/// derived from it rather than accepted from a caller.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomesAfterLiteralFolds {
    folds: StagedOptimizedLiteralFolds,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterLiteralFolds {
    /// The retained fold stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    pub const fn fold_stage(&self) -> &StagedOptimizedLiteralFolds {
        &self.folds
    }
    /// The final folded program this assignment describes.
    pub fn selected(
        &self,
    ) -> &selected_instructions_to_selected_instructions::ValidatedLiteralFold {
        self.folds.final_step().fold()
    }
    /// The reanalyzed facts over the final folded program.
    pub fn liveness(&self) -> &crate::ValidatedLiveness {
        self.folds.final_step().liveness()
    }
    pub fn ranges(&self) -> &crate::ValidatedLiveRanges {
        self.folds.final_step().ranges()
    }
    pub fn legality(&self) -> &crate::ValidatedAllocationLegality {
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
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> &StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
        &self.custody
    }
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
    pub const fn source(&self) -> &StagedOptimizedLiteralFoldCustodyReceipt {
        &self.source
    }
    pub const fn homes(&self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(&self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(&self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostLiteralFoldHomeCustodyError {
    UpstreamFolds(OptimizedLiteralFoldCustodyError),
    Assignment(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostLiteralFoldHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-literal-fold home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostLiteralFoldHomeCustodyError {}

/// Strict homes after a complete named selected-lowering suite. The suite's
/// completion identity is retained even when its transformation ledger is
/// empty because the source was already at the validated fixed point.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomesAfterSelectedLowering {
    run: StagedSelectedLoweringOptimizationRun,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterSelectedLowering {
    /// The retained lowering run. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    pub const fn selected_lowering_run(&self) -> &StagedSelectedLoweringOptimizationRun {
        &self.run
    }
    /// The program this assignment describes: the last step's fold when the
    /// suite ran, otherwise the unchanged selected program the run admitted.
    pub fn selected(&self) -> crate::SelectedProgramRef<'_> {
        match self.run.steps().last() {
            Some(step) => crate::SelectedProgramRef::new(step.fold()),
            None => crate::SelectedProgramRef::new(self.run.source_legality_stage().selected()),
        }
    }
    /// The facts over the current program: the last step's rebuilt analyses,
    /// or the run's admitted analyses when the suite stayed at the fixed
    /// point.
    pub fn liveness(&self) -> &crate::ValidatedLiveness {
        match self.run.steps().last() {
            Some(step) => step.liveness(),
            None => self.run.source_legality_stage().liveness(),
        }
    }
    pub fn ranges(&self) -> &crate::ValidatedLiveRanges {
        match self.run.steps().last() {
            Some(step) => step.ranges(),
            None => self.run.source_legality_stage().ranges(),
        }
    }
    pub fn legality(&self) -> &crate::ValidatedAllocationLegality {
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
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        &self.custody
    }
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
    pub const fn source(&self) -> &StagedSelectedLoweringOptimizationCustodyReceipt {
        &self.source
    }
    pub const fn homes(&self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(&self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(&self) -> usize {
        self.assignment_count
    }
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
