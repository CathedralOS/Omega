use crate::{
    PostAllocationOptimizationManifestError, RegisterHomeError,
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};

use crate::{OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality};
use crate::{OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis};

/// Bounded opt-in physical-home staging. This lane admits only legality plans
/// with at least one shared legal candidate per VReg and no unresolved
/// fixed-view transition or spill requirement. It grants no machine-emission
/// or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomes {
    pub(super) legality: StagedOptimizedAllocationLegality,
    pub(super) homes: ValidatedRegisterHomes,
    pub(super) manifest: ValidatedPostAllocationOptimizationManifest,
    pub(super) custody: RegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomes {
    pub const fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.legality
    }
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> RegisterHomeCustodyReceipt {
        self.custody
    }
}

pub use register_homes::RegisterHomeCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedRegisterHomeCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    Assignment(RegisterHomeError),
    Revalidation(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedRegisterHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized register-home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedRegisterHomeCustodyError {}

/// Physical homes after one exact fixed-view copy transformation and complete
/// reanalysis. This remains custody-only and cannot enter machine emission.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomesAfterFixedViewCopies {
    pub(super) reanalysis: StagedOptimizedSelectedReanalysis,
    pub(super) homes: ValidatedRegisterHomes,
    pub(super) manifest: ValidatedPostAllocationOptimizationManifest,
    pub(super) custody: PostCopyRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterFixedViewCopies {
    pub const fn reanalysis_stage(&self) -> &StagedOptimizedSelectedReanalysis {
        &self.reanalysis
    }
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> PostCopyRegisterHomeCustodyReceipt {
        self.custody
    }
}

pub use register_homes::PostCopyRegisterHomeCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostCopyRegisterHomeCustodyError {
    UpstreamReanalysis(OptimizedSelectedReanalysisError),
    Assignment(RegisterHomeError),
    Revalidation(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostCopyRegisterHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-copy register-home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostCopyRegisterHomeCustodyError {}
