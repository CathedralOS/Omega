//! Active-resident rematerialization retained through machine-plan custody.

use crate::tests::{
    AllocationEvidence, NativeTarget, OptimizedActiveResidentRematerializationError,
    OptimizedPostAllocationMachinePipelineError, PostAllocationMachineCustodyFieldForTest,
    PostAllocationMachinePlanReceiptFieldForTest, PressureRematerializationPolicy,
    RecoveryClassificationPolicy, SpillChoicePolicy, analyze_machine_effects,
    selected_lowering_budget, stage_optimized_active_resident_rematerialization,
    stage_optimized_post_allocation_machine_plan, staged_active_resident_two_view_legality,
    validate_machine_effects, validate_optimized_post_allocation_machine_plan_custody,
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

        let effects = analyze_machine_effects(
            source.rematerialization(),
            source_selected.register_environment(),
        )
        .unwrap();
        assert_eq!(effects.receipt().selected(), transformed_selected);
        assert_eq!(
            effects.plan().optimization_unit,
            source.custody().source().optimization_unit()
        );
        assert_eq!(
            effects.plan().fuel_schedule,
            source.custody().source().fuel_schedule()
        );
        assert_eq!(effects.plan().target, target);
        assert_eq!(
            effects.receipt().register_environment(),
            source_selected.register_environment().identity()
        );
        assert_eq!(effects.receipt().selected(), transformed_selected);
        validate_machine_effects(
            source.rematerialization(),
            source_selected.register_environment(),
            &effects,
        )
        .unwrap();

        let post = stage_optimized_post_allocation_machine_plan(&source).unwrap();
        assert_eq!(post.machine().receipt().selected(), transformed_selected);
        assert_eq!(
            post.machine().receipt().effects(),
            post.effects().receipt().identity()
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
            &AllocationEvidence::ActiveResidentRematerialization(source.custody())
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_custody(&source, &post,).unwrap(),
            post.custody()
        );

        assert_eq!(
            register_homes_to_post_allocation_machine::validate_post_allocation_machine_plan(
                source_selected.selected(),
                post.effects(),
                source.ranges(),
                source.legality(),
                source.homes(),
                source.post_allocation_manifest(),
                source_selected.register_environment().identity(),
                source_selected.register_environment().physical(),
                source_selected.register_environment().constraints(),
                post.machine().plan().clone(),
            ),
            Err(register_homes_to_post_allocation_machine::PostAllocationMachineError::SelectedRootMismatch)
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
    crate::corrupt_active_resident_rematerialization_custody_for_test(&mut corrupted);
    assert!(matches!(
        crate::validate_optimized_active_resident_rematerialization(&corrupted),
        Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch)
    ));
    assert!(matches!(
        stage_optimized_post_allocation_machine_plan(&corrupted,),
        Err(OptimizedPostAllocationMachinePipelineError::Allocation(
            crate::AllocationReplayError::ActiveResidentRematerialization(
                OptimizedActiveResidentRematerializationError::ReceiptMismatch
            )
        ))
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
    let x86_post = stage_optimized_post_allocation_machine_plan(&x86).unwrap();
    assert!(validate_optimized_post_allocation_machine_plan_custody(&arm, &x86_post,).is_err());
}

#[test]
fn post_allocation_machine_custody_rejects_every_one_field_substitution() {
    use PostAllocationMachineCustodyFieldForTest::*;
    let fields: [(&str, PostAllocationMachineCustodyFieldForTest); 7] = [
        ("source", Source),
        ("effects", Effects),
        ("machine", Machine),
        ("function_count", FunctionCount),
        ("instruction_count", InstructionCount),
        ("operand_count", OperandCount),
        ("unit_action_count", UnitActionCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let source = stage_optimized_active_resident_rematerialization(
                staged_active_resident_two_view_legality(target),
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                selected_lowering_budget(),
            )
            .unwrap();
            let machine = stage_optimized_post_allocation_machine_plan(&source).unwrap();
            (source, machine)
        };
        // An authentic foreign machine plan on the opposite architecture is
        // the donor for the nested allocation-evidence source field.
        let (_foreign_source, foreign_machine) = build(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        let (source, machine) = build(target);
        assert_ne!(
            machine.custody().source(),
            foreign_machine.custody().source(),
            "{target:?}: the foreign target must produce distinct allocation evidence",
        );
        for (name, field) in fields {
            let mut substituted = machine.clone();
            let honest = substituted.custody().clone();
            substituted.corrupt_custody_for_test(field, &foreign_machine);
            assert_ne!(
                substituted.custody(),
                &honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            assert_eq!(
                validate_optimized_post_allocation_machine_plan_custody(&source, &substituted),
                Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch),
                "{target:?}: independent replay must reject substituted machine-custody field {name}",
            );
        }
    }
}

#[test]
fn post_allocation_machine_plan_receipt_rejects_every_one_field_substitution() {
    use PostAllocationMachinePlanReceiptFieldForTest::*;
    let fields: [(&str, PostAllocationMachinePlanReceiptFieldForTest); 11] = [
        ("identity", Identity),
        ("selected", Selected),
        ("effects", Effects),
        ("homes", Homes),
        ("post_allocation_manifest", PostAllocationManifest),
        ("register_environment", RegisterEnvironment),
        ("function_count", FunctionCount),
        ("block_count", BlockCount),
        ("instruction_count", InstructionCount),
        ("operand_count", OperandCount),
        ("unit_action_count", UnitActionCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(target),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let machine = stage_optimized_post_allocation_machine_plan(&source).unwrap();
        for (name, field) in fields {
            let mut substituted = machine.clone();
            let honest = substituted.machine().receipt();
            substituted.corrupt_machine_receipt_for_test(field);
            assert_ne!(
                substituted.machine().receipt(),
                honest,
                "{target:?}: mutation `{name}` must change the retained machine receipt",
            );
            assert_eq!(
                validate_optimized_post_allocation_machine_plan_custody(&source, &substituted),
                Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch),
                "{target:?}: independent replay must reject substituted machine-receipt field {name}",
            );
        }
    }
}
