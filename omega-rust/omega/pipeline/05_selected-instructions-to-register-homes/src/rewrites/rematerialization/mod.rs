//! Optimizer module role: executable entrance. Active-resident pressure-rematerialization stage.
//!
//! The producer rebuilds all allocation facts from the transformed selected
//! CFG. This entrance grants stage custody only after independent replay
//! validation reconstructs that complete chain.

mod compute;
mod custody;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::{
    OptimizedActiveResidentRematerializationCustodyFieldForTest,
    OptimizedActiveResidentRematerializationPressureCustodyFieldForTest,
};
pub use validation::{
    validate_optimized_active_resident_rematerialization,
    validate_optimized_active_resident_rematerialization_pressure,
};

use optimization_core::OptimizationWorkBudget;

use crate::{
    RegisterHomeError, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use register_homes::{
    AllocationLegalityIdentity, PostAllocationOptimizationManifestError,
    RecoveryClassificationIdentity, RecoveryClassificationPolicy, SpillChoiceIdentity,
    SpillChoicePolicy,
};
use selected_instructions::{
    LiveRangeIdentity, LivenessIdentity, PressureRematerializationIdentity,
};
use selected_instructions_to_selected_instructions::{
    AllocationLegalityError, LiveRangeError, LivenessError,
    OptimizedAllocationLegalityCustodyError, PressureRematerializationError,
    PressureRematerializationPolicy, RecoveryClassificationError, SpillChoiceError,
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPressureRematerialization, ValidatedRecoveryClassifications, ValidatedSpillChoices,
};

/// Stage the proven rematerialization sweep without attempting terminal
/// homes assignment: the result is custody for the route's own decision —
/// complete the sweep when assignment succeeds, or hand the same proven
/// prefix to runtime-spill recovery when residual `NoCompatibleHome`
/// pressure remains.
#[allow(clippy::too_many_arguments)]
pub fn stage_optimized_active_resident_rematerialization_pressure(
    source: StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    classification_policy: RecoveryClassificationPolicy,
    rematerialization_policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedActiveResidentRematerializationPressure,
    OptimizedActiveResidentRematerializationError,
> {
    let staged = compute::compute_active_resident_rematerialization_pressure(
        source,
        choice_policy,
        classification_policy,
        rematerialization_policy,
        budget,
    )?;
    validate_optimized_active_resident_rematerialization_pressure(&staged)?;
    Ok(staged)
}

/// Terminal completion of a proven sweep: the caller supplies the homes a
/// successful assignment over the rebuilt facts produced. Custody is granted
/// only after independent replay reconstructs the complete chain, exactly as
/// the one-shot entrance requires.
pub(crate) fn complete_optimized_active_resident_rematerialization(
    pressure: StagedOptimizedActiveResidentRematerializationPressure,
    homes: crate::ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedActiveResidentRematerialization,
    OptimizedActiveResidentRematerializationError,
> {
    let staged = compute::complete_active_resident_rematerialization(pressure, homes)?;
    validate_optimized_active_resident_rematerialization(&staged)?;
    Ok(staged)
}

#[allow(clippy::too_many_arguments)]
pub fn stage_optimized_active_resident_rematerialization(
    source: StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    classification_policy: RecoveryClassificationPolicy,
    rematerialization_policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedActiveResidentRematerialization,
    OptimizedActiveResidentRematerializationError,
> {
    let staged = compute::compute_active_resident_rematerialization(
        source,
        choice_policy,
        classification_policy,
        rematerialization_policy,
        budget,
    )?;
    validate_optimized_active_resident_rematerialization(&staged)?;
    Ok(staged)
}

/// One bounded active-resident rematerialization sweep followed by analyses,
/// homes, and a typed post-allocation manifest rebuilt from the transformed
/// selected CFG. The source analyses remain retained only as input custody.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerialization {
    source: StagedOptimizedAllocationLegality,
    choices: ValidatedSpillChoices,
    classifications: ValidatedRecoveryClassifications,
    rematerialization: ValidatedPressureRematerialization,
    liveness: ValidatedLiveness,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerialization {
    /// The retained legality stage the proven sweep consumed. Replay and
    /// custody validation inspect it; ordinary consumers read the current
    /// program and rebuilt analyses directly.
    pub const fn source(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }
    /// The register environment admitted with the retained source — the sweep
    /// preserves it, so the source's environment is the current one.
    pub const fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.source.register_environment()
    }
    /// The governing optimizer selections admitted with the retained stage.
    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.source.selections()
    }
    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.source.budget_per_pass()
    }
    /// The retained optimized-target proof input, kept as replay evidence;
    /// downstream custody checks compare the owner handle by identity.
    pub fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
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
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    choices: SpillChoiceIdentity,
    choice_policy: SpillChoicePolicy,
    choice_usage: optimization_core::OptimizationWorkUsage,
    classifications: RecoveryClassificationIdentity,
    classification_policy: RecoveryClassificationPolicy,
    classification_usage: optimization_core::OptimizationWorkUsage,
    rematerialization: PressureRematerializationIdentity,
    rematerialization_policy: PressureRematerializationPolicy,
    rematerialization_usage: optimization_core::OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
    transformed_selected: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
    ranges: LiveRangeIdentity,
    legality: AllocationLegalityIdentity,
    homes: crate::RegisterHomeIdentity,
    manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    virtual_register_count: usize,
    applied_count: usize,
    rewritten_use_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedActiveResidentRematerializationCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.source
    }
    pub const fn choices(self) -> SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn classifications(self) -> RecoveryClassificationIdentity {
        self.classifications
    }
    pub const fn classification_policy(self) -> RecoveryClassificationPolicy {
        self.classification_policy
    }
    pub const fn classification_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.classification_usage
    }
    pub const fn rematerialization(self) -> PressureRematerializationIdentity {
        self.rematerialization
    }
    pub const fn rematerialization_policy(self) -> PressureRematerializationPolicy {
        self.rematerialization_policy
    }
    pub const fn rematerialization_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.rematerialization_usage
    }
    pub const fn budget(self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn transformed_selected(
        self,
    ) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn homes(self) -> crate::RegisterHomeIdentity {
        self.homes
    }
    pub const fn manifest(self) -> optimization_core::PostAllocationOptimizationManifestIdentity {
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

/// The recorded sweep once its applied rewrite and rebuilt facts are proven
/// but before terminal homes assignment is attempted. Residual
/// `NoCompatibleHome` pressure hands this custody to runtime-spill recovery,
/// which keeps the rematerialization as the first recorded transformation
/// and independently replays this whole prefix before any spill step.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerializationPressure {
    source: StagedOptimizedAllocationLegality,
    choices: ValidatedSpillChoices,
    classifications: ValidatedRecoveryClassifications,
    rematerialization: ValidatedPressureRematerialization,
    liveness: ValidatedLiveness,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    custody: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerializationPressure {
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
    pub const fn custody(
        &self,
    ) -> StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
        self.custody
    }
}

/// Custody receipt for the proven rematerialization prefix: every identity
/// the terminal or composing completion must reproduce, without homes or
/// manifest — those belong to whichever path resolves the residual pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    choices: SpillChoiceIdentity,
    choice_policy: SpillChoicePolicy,
    choice_usage: optimization_core::OptimizationWorkUsage,
    classifications: RecoveryClassificationIdentity,
    classification_policy: RecoveryClassificationPolicy,
    classification_usage: optimization_core::OptimizationWorkUsage,
    rematerialization: PressureRematerializationIdentity,
    rematerialization_policy: PressureRematerializationPolicy,
    rematerialization_usage: optimization_core::OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
    transformed_selected: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
    ranges: LiveRangeIdentity,
    legality: AllocationLegalityIdentity,
    function_count: usize,
    virtual_register_count: usize,
    applied_count: usize,
    rewritten_use_count: usize,
}

impl StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.source
    }
    pub const fn choices(self) -> SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn classifications(self) -> RecoveryClassificationIdentity {
        self.classifications
    }
    pub const fn classification_policy(self) -> RecoveryClassificationPolicy {
        self.classification_policy
    }
    pub const fn classification_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.classification_usage
    }
    pub const fn rematerialization(self) -> PressureRematerializationIdentity {
        self.rematerialization
    }
    pub const fn rematerialization_policy(self) -> PressureRematerializationPolicy {
        self.rematerialization_policy
    }
    pub const fn rematerialization_usage(self) -> optimization_core::OptimizationWorkUsage {
        self.rematerialization_usage
    }
    pub const fn budget(self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn transformed_selected(
        self,
    ) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationError {
    Upstream(OptimizedAllocationLegalityCustodyError),
    UnsupportedPolicy,
    SpillChoice(SpillChoiceError),
    Classification(RecoveryClassificationError),
    Rematerialization(PressureRematerializationError),
    NoAppliedAction,
    Liveness(LivenessError),
    Ranges(LiveRangeError),
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

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub(crate) fn corrupt_active_resident_rematerialization_pressure_custody_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationPressure,
) {
    staged.custody.rewritten_use_count += 1;
}
