use crate::tests::{
    NativeTarget, OptimizationWorkBudget, OptimizedActiveResidentRematerializationError,
    PressureRematerializationPolicy, RecoveryClassificationPolicy, SpillChoicePolicy,
    StagedOptimizedActiveResidentRematerialization,
    stage_optimized_active_resident_rematerialization, staged_active_resident_two_view_legality,
};

pub(super) const CHOICE_POLICY: SpillChoicePolicy =
    SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1;
pub(super) const CLASSIFICATION_POLICY: RecoveryClassificationPolicy =
    RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1;
pub(super) const REMATERIALIZATION_POLICY: PressureRematerializationPolicy =
    PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1;

pub(super) fn targets() -> [NativeTarget; 2] {
    [NativeTarget::linux_x64(), NativeTarget::linux_arm64()]
}

pub(super) fn run(
    target: NativeTarget,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedActiveResidentRematerialization,
    OptimizedActiveResidentRematerializationError,
> {
    stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(target),
        CHOICE_POLICY,
        CLASSIFICATION_POLICY,
        REMATERIALIZATION_POLICY,
        budget,
    )
}
