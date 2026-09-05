use crate::{
    AllocationLegalityError, PostAllocationOptimizationManifestError,
    PressureRematerializationError, PressureRematerializationPolicy, RecoveryClassificationError,
    RecoveryClassificationPolicy, RegisterHomeError, SpillChoiceError, SpillChoicePolicy,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPostAllocationOptimizationManifest, ValidatedPressureRematerialization,
    ValidatedRecoveryClassifications, ValidatedRegisterHomes, ValidatedSpillChoices,
};
use omega_optimization_core::OptimizationWorkBudget;

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt,
};

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
    pub(super) custody: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
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
    pub const fn custody(&self) -> StagedOptimizedActiveResidentRematerializationCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationCustodyReceipt {
    pub(super) source: StagedOptimizedAllocationLegalityCustodyReceipt,
    pub(super) choices: crate::SpillChoiceIdentity,
    pub(super) choice_policy: SpillChoicePolicy,
    pub(super) choice_usage: omega_optimization_core::OptimizationWorkUsage,
    pub(super) classifications: crate::RecoveryClassificationIdentity,
    pub(super) classification_policy: RecoveryClassificationPolicy,
    pub(super) classification_usage: omega_optimization_core::OptimizationWorkUsage,
    pub(super) rematerialization: crate::PressureRematerializationIdentity,
    pub(super) rematerialization_policy: PressureRematerializationPolicy,
    pub(super) rematerialization_usage: omega_optimization_core::OptimizationWorkUsage,
    pub(super) budget: OptimizationWorkBudget,
    pub(super) transformed_selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) liveness: crate::LivenessIdentity,
    pub(super) ranges: crate::LiveRangeIdentity,
    pub(super) legality: crate::AllocationLegalityIdentity,
    pub(super) homes: crate::RegisterHomeIdentity,
    pub(super) manifest: omega_optimization_core::PostAllocationOptimizationManifestIdentity,
    pub(super) function_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) applied_count: usize,
    pub(super) rewritten_use_count: usize,
    pub(super) assignment_count: usize,
}

impl StagedOptimizedActiveResidentRematerializationCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.source
    }
    pub const fn choices(self) -> crate::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> omega_optimization_core::OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn classifications(self) -> crate::RecoveryClassificationIdentity {
        self.classifications
    }
    pub const fn classification_policy(self) -> RecoveryClassificationPolicy {
        self.classification_policy
    }
    pub const fn classification_usage(self) -> omega_optimization_core::OptimizationWorkUsage {
        self.classification_usage
    }
    pub const fn rematerialization(self) -> crate::PressureRematerializationIdentity {
        self.rematerialization
    }
    pub const fn rematerialization_policy(self) -> PressureRematerializationPolicy {
        self.rematerialization_policy
    }
    pub const fn rematerialization_usage(self) -> omega_optimization_core::OptimizationWorkUsage {
        self.rematerialization_usage
    }
    pub const fn budget(self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn transformed_selected(
        self,
    ) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn liveness(self) -> crate::LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> crate::LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> crate::AllocationLegalityIdentity {
        self.legality
    }
    pub const fn homes(self) -> crate::RegisterHomeIdentity {
        self.homes
    }
    pub const fn manifest(
        self,
    ) -> omega_optimization_core::PostAllocationOptimizationManifestIdentity {
        self.manifest
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
    pub const fn rewritten_use_count(self) -> usize {
        self.rewritten_use_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

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
