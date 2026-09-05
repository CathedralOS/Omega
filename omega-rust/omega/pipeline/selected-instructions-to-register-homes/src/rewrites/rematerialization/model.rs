use crate::{
    AllocationLegalityError, PostAllocationOptimizationManifestError,
    PressureRematerializationError, RecoveryClassificationError, RegisterHomeError,
    SpillChoiceError, ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPostAllocationOptimizationManifest, ValidatedPressureRematerialization,
    ValidatedRecoveryClassifications, ValidatedRegisterHomes, ValidatedSpillChoices,
};

use crate::{OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality};

/// One bounded active-resident rematerialization sweep followed by analyses,
/// homes, and a typed post-allocation manifest rebuilt from the transformed
/// selected CFG. The source analyses remain retained only as input custody.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerialization {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) choices: ValidatedSpillChoices,
    pub(super) classifications: ValidatedRecoveryClassifications,
    pub(super) rematerialization: ValidatedPressureRematerialization,
    pub(super) liveness: ValidatedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) legality: ValidatedAllocationLegality,
    pub(super) homes: ValidatedRegisterHomes,
    pub(super) manifest: ValidatedPostAllocationOptimizationManifest,
    pub(super) custody: ActiveResidentRematerializationCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerialization {
    pub const fn source(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }
    pub const fn choices(&self) -> &ValidatedSpillChoices {
        &self.choices
    }
    pub const fn classifications(&self) -> &ValidatedRecoveryClassifications {
        &self.classifications
    }
    pub const fn rematerialization(&self) -> &ValidatedPressureRematerialization {
        &self.rematerialization
    }
    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        &self.legality
    }
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> ActiveResidentRematerializationCustodyReceipt {
        self.custody
    }
}

pub use register_homes::ActiveResidentRematerializationCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationError {
    Upstream(OptimizedAllocationLegalityCustodyError),
    UnsupportedPolicy,
    SpillChoice(SpillChoiceError),
    Classification(RecoveryClassificationError),
    Rematerialization(PressureRematerializationError),
    NoAppliedAction,
    Liveness(crate::LivenessError),
    Ranges(crate::LiveRangeError),
    Legality(AllocationLegalityError),
    RemainingTransitions { count: usize },
    Homes(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedActiveResidentRematerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized active-resident rematerialization failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedActiveResidentRematerializationError {}

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub fn corrupt_active_resident_rematerialization_custody_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerialization,
) {
    staged.custody.rewritten_use_count += 1;
}
