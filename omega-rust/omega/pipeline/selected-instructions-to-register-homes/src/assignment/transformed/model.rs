use crate::{
    PostAllocationOptimizationManifestError, RegisterHomeError,
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};

use crate::{
    OptimizedLiteralFoldCustodyError, StagedOptimizedLiteralFolds,
    StagedSelectedLoweringOptimizationRun,
};

/// Strict homes after one or more separately requested literal folds. The
/// complete append-only fold chain remains owned and the manifest ledger is
/// derived from it rather than accepted from a caller.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomesAfterLiteralFolds {
    pub(super) folds: StagedOptimizedLiteralFolds,
    pub(super) homes: ValidatedRegisterHomes,
    pub(super) manifest: ValidatedPostAllocationOptimizationManifest,
    pub(super) custody: PostLiteralFoldHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterLiteralFolds {
    pub const fn fold_stage(&self) -> &StagedOptimizedLiteralFolds {
        &self.folds
    }
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> &PostLiteralFoldHomeCustodyReceipt {
        &self.custody
    }
}

pub use register_homes::PostLiteralFoldHomeCustodyReceipt;

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
    pub(super) run: StagedSelectedLoweringOptimizationRun,
    pub(super) homes: ValidatedRegisterHomes,
    pub(super) manifest: ValidatedPostAllocationOptimizationManifest,
    pub(super) custody: PostSelectedLoweringHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterSelectedLowering {
    pub const fn selected_lowering_run(&self) -> &StagedSelectedLoweringOptimizationRun {
        &self.run
    }
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> &PostSelectedLoweringHomeCustodyReceipt {
        &self.custody
    }
}

pub use register_homes::PostSelectedLoweringHomeCustodyReceipt;

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
