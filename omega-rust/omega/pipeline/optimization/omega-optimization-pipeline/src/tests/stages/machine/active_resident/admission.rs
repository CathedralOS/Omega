//! Rule admission, decline, budget, and vertical-custody rejection.

use crate::tests::{
    NativeTarget, OptimizationWorkBudget, OptimizedActiveResidentRematerializationError,
    PressureRematerializationError, PressureRematerializationPolicy, RecoveryClassificationPolicy,
    SpillChoicePolicy, selected_lowering_budget, stage_optimized_active_resident_rematerialization,
    stage_optimized_allocation_legality, stage_optimized_live_ranges, stage_optimized_liveness,
    staged_active_resident_exact_add_chain, staged_active_resident_two_view_legality,
    validate_optimized_active_resident_rematerialization,
};

#[test]
fn active_resident_stage_declines_default_single_use_and_exhausted_budget() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let default = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_active_resident_exact_add_chain(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_active_resident_rematerialization(
                default,
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                selected_lowering_budget(),
            ),
            Err(OptimizedActiveResidentRematerializationError::Rematerialization(
                PressureRematerializationError::NoAction
            ))
        ));
        assert!(matches!(
            stage_optimized_active_resident_rematerialization(
                staged_active_resident_two_view_legality(target),
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
                selected_lowering_budget(),
            ),
            Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy)
        ));
        assert!(matches!(
            stage_optimized_active_resident_rematerialization(
                staged_active_resident_two_view_legality(target),
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
            ),
            Err(OptimizedActiveResidentRematerializationError::SpillChoice(
                omega_regalloc::SpillChoiceError::BudgetExceeded { .. }
            ))
        ));
    }
}

#[test]
fn active_resident_stage_rejects_corrupted_vertical_custody() {
    let mut staged = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    crate::stages::machine::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
        &mut staged,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization(&staged),
        Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch)
    );
}
