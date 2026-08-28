use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::{
    AllocationLegalityError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PressureRematerializationError,
    PressureRematerializationPolicy, RecoveryClassificationError, RecoveryClassificationPolicy,
    RegisterHomeError, SpillChoiceError, SpillChoicePolicy, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedLiveness, ValidatedPostAllocationOptimizationManifest,
    ValidatedPressureRematerialization, ValidatedRecoveryClassifications, ValidatedRegisterHomes,
    ValidatedSpillChoices, analyze_allocation_legality, analyze_live_ranges, analyze_liveness,
    assign_register_homes, choose_spill_victims, classify_pressure_recovery,
    project_post_allocation_optimization_manifest, rematerialize_selected_active_resident,
    validate_allocation_legality, validate_live_ranges, validate_liveness,
    validate_post_allocation_optimization_manifest, validate_pressure_rematerialization,
    validate_recovery_classifications, validate_register_homes, validate_spill_choices,
};

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

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
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    choices: omega_regalloc::SpillChoiceIdentity,
    choice_policy: SpillChoicePolicy,
    choice_usage: omega_optimization_core::OptimizationWorkUsage,
    classifications: omega_regalloc::RecoveryClassificationIdentity,
    classification_policy: RecoveryClassificationPolicy,
    classification_usage: omega_optimization_core::OptimizationWorkUsage,
    rematerialization: omega_regalloc::PressureRematerializationIdentity,
    rematerialization_policy: PressureRematerializationPolicy,
    rematerialization_usage: omega_optimization_core::OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
    transformed_selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    liveness: omega_regalloc::LivenessIdentity,
    ranges: omega_regalloc::LiveRangeIdentity,
    legality: omega_regalloc::AllocationLegalityIdentity,
    homes: omega_regalloc::RegisterHomeIdentity,
    manifest: omega_optimization_core::PostAllocationOptimizationManifestIdentity,
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
    pub const fn choices(self) -> omega_regalloc::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> omega_optimization_core::OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn classifications(self) -> omega_regalloc::RecoveryClassificationIdentity {
        self.classifications
    }
    pub const fn classification_policy(self) -> RecoveryClassificationPolicy {
        self.classification_policy
    }
    pub const fn classification_usage(self) -> omega_optimization_core::OptimizationWorkUsage {
        self.classification_usage
    }
    pub const fn rematerialization(self) -> omega_regalloc::PressureRematerializationIdentity {
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
    pub const fn liveness(self) -> omega_regalloc::LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> omega_regalloc::LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> omega_regalloc::AllocationLegalityIdentity {
        self.legality
    }
    pub const fn homes(self) -> omega_regalloc::RegisterHomeIdentity {
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
    Liveness(omega_regalloc::LivenessError),
    Ranges(omega_regalloc::LiveRangeError),
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
    if choice_policy != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
        || classification_policy
            != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
        || rematerialization_policy
            != PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1
    {
        return Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy);
    }
    let source_receipt = validate_source(&source)?;
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let selected = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let source_ranges = source.live_range_stage().ranges();
    let choices = choose_spill_victims(
        source.legality(),
        source_ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        choice_policy,
        budget,
    )
    .map_err(OptimizedActiveResidentRematerializationError::SpillChoice)?;
    let classifications = classify_pressure_recovery(
        selected,
        source_ranges,
        source.legality(),
        &choices,
        classification_policy,
        budget,
    )
    .map_err(OptimizedActiveResidentRematerializationError::Classification)?;
    let rematerialization = rematerialize_selected_active_resident(
        selected,
        source_ranges,
        source.legality(),
        &choices,
        &classifications,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        rematerialization_policy,
        budget,
    )
    .map_err(OptimizedActiveResidentRematerializationError::Rematerialization)?;
    if rematerialization.receipt().applied_count() == 0 {
        return Err(OptimizedActiveResidentRematerializationError::NoAppliedAction);
    }

    let liveness = analyze_liveness(&rematerialization)
        .map_err(OptimizedActiveResidentRematerializationError::Liveness)?;
    let ranges = analyze_live_ranges(&rematerialization, &liveness)
        .map_err(OptimizedActiveResidentRematerializationError::Ranges)?;
    let legality = analyze_allocation_legality(
        &ranges,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Legality)?;
    if legality.receipt().entry_transition_count() != 0 {
        return Err(
            OptimizedActiveResidentRematerializationError::RemainingTransitions {
                count: legality.receipt().entry_transition_count(),
            },
        );
    }
    let homes = assign_register_homes(
        &legality,
        &ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Homes)?;
    let manifest = project_post_allocation_optimization_manifest(
        source_receipt.manifest(),
        &[
            PostAllocationSelectedTransformation::PressureRematerialization(
                rematerialization.receipt().identity(),
            ),
        ],
        &ranges,
        &legality,
        &homes,
    )
    .map_err(OptimizedActiveResidentRematerializationError::Manifest)?;
    let custody = custody_receipt(
        source_receipt,
        &choices,
        &classifications,
        &rematerialization,
        &liveness,
        &ranges,
        &legality,
        &homes,
        &manifest,
    );
    let staged = StagedOptimizedActiveResidentRematerialization {
        source,
        choices,
        classifications,
        rematerialization,
        liveness,
        ranges,
        legality,
        homes,
        manifest,
        custody,
    };
    validate_optimized_active_resident_rematerialization(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_active_resident_rematerialization(
    staged: &StagedOptimizedActiveResidentRematerialization,
) -> Result<
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    OptimizedActiveResidentRematerializationError,
> {
    let source_receipt = validate_source(&staged.source)?;
    if staged.choices.receipt().policy()
        != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
        || staged.classifications.receipt().policy()
            != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
        || staged.rematerialization.receipt().policy()
            != PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1
        || staged.choices.plan().budget != staged.classifications.plan().budget
        || staged.choices.plan().budget != staged.rematerialization.plan().budget
    {
        return Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy);
    }
    let environment = staged
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let selected = staged
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let source_ranges = staged.source.live_range_stage().ranges();
    let choices = validate_spill_choices(
        staged.source.legality(),
        source_ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.choices.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::SpillChoice)?;
    let classifications = validate_recovery_classifications(
        selected,
        source_ranges,
        staged.source.legality(),
        &choices,
        staged.classifications.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Classification)?;
    let rematerialization = validate_pressure_rematerialization(
        selected,
        source_ranges,
        staged.source.legality(),
        &choices,
        &classifications,
        staged.source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.rematerialization.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Rematerialization)?;
    if rematerialization.receipt().applied_count() == 0 {
        return Err(OptimizedActiveResidentRematerializationError::NoAppliedAction);
    }
    let liveness = validate_liveness(&rematerialization, staged.liveness.plan().clone())
        .map_err(OptimizedActiveResidentRematerializationError::Liveness)?;
    let ranges = validate_live_ranges(&rematerialization, &liveness, staged.ranges.plan().clone())
        .map_err(OptimizedActiveResidentRematerializationError::Ranges)?;
    let legality = validate_allocation_legality(
        &ranges,
        staged.source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.legality.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Legality)?;
    if legality.receipt().entry_transition_count() != 0 {
        return Err(
            OptimizedActiveResidentRematerializationError::RemainingTransitions {
                count: legality.receipt().entry_transition_count(),
            },
        );
    }
    let homes = validate_register_homes(
        &legality,
        &ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.homes.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Homes)?;
    let manifest = validate_post_allocation_optimization_manifest(
        staged.manifest.record(),
        source_receipt.manifest(),
        &[
            PostAllocationSelectedTransformation::PressureRematerialization(
                rematerialization.receipt().identity(),
            ),
        ],
        &ranges,
        &legality,
        &homes,
    )
    .map_err(OptimizedActiveResidentRematerializationError::Manifest)?;
    let custody = custody_receipt(
        source_receipt,
        &choices,
        &classifications,
        &rematerialization,
        &liveness,
        &ranges,
        &legality,
        &homes,
        &manifest,
    );
    if choices != staged.choices
        || classifications != staged.classifications
        || rematerialization != staged.rematerialization
        || liveness != staged.liveness
        || ranges != staged.ranges
        || legality != staged.legality
        || homes != staged.homes
        || manifest != staged.manifest
        || custody != staged.custody
    {
        return Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch);
    }
    Ok(custody)
}

#[allow(clippy::too_many_arguments)]
fn custody_receipt(
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    choices: &ValidatedSpillChoices,
    classifications: &ValidatedRecoveryClassifications,
    rematerialization: &ValidatedPressureRematerialization,
    liveness: &ValidatedLiveness,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> StagedOptimizedActiveResidentRematerializationCustodyReceipt {
    StagedOptimizedActiveResidentRematerializationCustodyReceipt {
        source,
        choices: choices.receipt().identity(),
        choice_policy: choices.receipt().policy(),
        choice_usage: choices.receipt().usage(),
        classifications: classifications.receipt().identity(),
        classification_policy: classifications.receipt().policy(),
        classification_usage: classifications.receipt().usage(),
        rematerialization: rematerialization.receipt().identity(),
        rematerialization_policy: rematerialization.receipt().policy(),
        rematerialization_usage: rematerialization.receipt().usage(),
        budget: rematerialization.plan().budget,
        transformed_selected: rematerialization.receipt().transformed_selected(),
        liveness: liveness.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        homes: homes.receipt().identity(),
        manifest: manifest.record().identity,
        function_count: rematerialization.receipt().function_count(),
        virtual_register_count: legality.receipt().virtual_register_count(),
        applied_count: rematerialization.receipt().applied_count(),
        rewritten_use_count: rematerialization.receipt().rewritten_use_count(),
        assignment_count: homes.receipt().assignment_count(),
    }
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_rematerialization_custody_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerialization,
) {
    staged.custody.rewritten_use_count += 1;
}

fn validate_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<
    StagedOptimizedAllocationLegalityCustodyReceipt,
    OptimizedActiveResidentRematerializationError,
> {
    validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Upstream)
}
