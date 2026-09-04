//! Active-resident rematerialization retained through machine-plan custody.

use crate::tests::{
    NativeTarget, OptimizedActiveResidentRematerializationError,
    OptimizedMachineEffectPipelineError, OptimizedPostAllocationMachinePipelineError,
    PressureRematerializationPolicy, RecoveryClassificationPolicy, SpillChoicePolicy,
    StagedOptimizedMachineEffectSourceCustodyReceipt,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, selected_lowering_budget,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_machine_effects_after_active_resident_rematerialization,
    stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization,
    staged_active_resident_two_view_legality,
    validate_optimized_machine_effect_custody_after_active_resident_rematerialization,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
};

#[test]
fn active_resident_rematerialization_reaches_machine_custody_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(target),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let source_selected = source
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let transformed_selected = source.rematerialization().receipt().transformed_selected();

        let effects =
            stage_optimized_machine_effects_after_active_resident_rematerialization(&source)
                .unwrap();
        assert_eq!(effects.effects().receipt().selected(), transformed_selected);
        assert_eq!(
            effects.effects().plan().optimization_unit,
            source.custody().source().optimization_unit()
        );
        assert_eq!(
            effects.effects().plan().fuel_schedule,
            source.custody().source().fuel_schedule()
        );
        assert_eq!(effects.effects().plan().target, target);
        assert_eq!(
            effects.effects().receipt().register_environment(),
            source_selected.register_environment().identity()
        );
        assert_eq!(
            effects.custody().source(),
            &StagedOptimizedMachineEffectSourceCustodyReceipt::ActiveResidentRematerialization(
                source.custody()
            )
        );
        assert_eq!(
            &validate_optimized_machine_effect_custody_after_active_resident_rematerialization(
                &source,
                effects.effects(),
            )
            .unwrap(),
            effects.custody()
        );

        let post =
            stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
                &source,
            )
            .unwrap();
        assert_eq!(post.machine().receipt().selected(), transformed_selected);
        assert_eq!(
            post.machine().receipt().effects(),
            post.effects().effects().receipt().identity()
        );
        assert_eq!(
            post.machine().receipt().homes(),
            source.homes().receipt().identity()
        );
        assert_eq!(
            post.machine().receipt().post_allocation_manifest(),
            source.post_allocation_manifest().record().identity
        );
        assert_eq!(
            post.machine().receipt().register_environment(),
            source_selected.register_environment().identity()
        );
        assert_eq!(
            post.custody().source(),
            &StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
                source.custody()
            )
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
                &source,
                &post,
            )
            .unwrap(),
            post.custody()
        );

        assert_eq!(
            omega_machine_optimizer::validate_post_allocation_machine_plan(
                source_selected.selected(),
                post.effects().effects(),
                source.ranges(),
                source.legality(),
                source.homes(),
                source.post_allocation_manifest(),
                source_selected.register_environment().identity(),
                source_selected.register_environment().physical(),
                source_selected.register_environment().constraints(),
                post.machine().plan().clone(),
            ),
            Err(omega_machine_optimizer::PostAllocationMachineError::SelectedRootMismatch)
        );
    }

    let mut corrupted = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    crate::stages::machine::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
        &mut corrupted,
    );
    assert!(matches!(
        stage_optimized_machine_effects_after_active_resident_rematerialization(&corrupted),
        Err(
            OptimizedMachineEffectPipelineError::ActiveResidentRematerialization(
                OptimizedActiveResidentRematerializationError::ReceiptMismatch
            )
        )
    ));
    assert!(matches!(
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
            &corrupted,
        ),
        Err(
            OptimizedPostAllocationMachinePipelineError::ActiveResidentRematerialization(
                OptimizedActiveResidentRematerializationError::ReceiptMismatch
            )
        )
    ));

    let x86 = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let arm = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(NativeTarget::linux_arm64()),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let x86_post =
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(&x86)
            .unwrap();
    assert!(
        validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
            &arm,
            &x86_post,
        )
        .is_err()
    );
}
