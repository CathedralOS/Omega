use crate::tests::*;

use super::fixture::{caller_machine, staged_homes, staged_selected};

fn call_mut(
    plan: &mut legalized_operations::LegalizedOperationPlan,
    index: usize,
) -> &mut legalized_operations::LegalizedScalarCallUnitCall {
    let legalized_operations::LegalizedScalarCallUnitOperation::Call(call) =
        &mut plan.scalar_call_unit_functions[0].operations[index + 2]
    else {
        panic!("the fixture starts with two constants followed by its calls")
    };
    call
}

#[test]
fn legal_call_order_callee_plan_arguments_lineage_and_evidence_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_selected(target);
        let original = staged.legalized().plan();
        let validate = |plan| {
            validate_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };
        let expect_rejected = |plan| {
            assert_eq!(
                validate(plan),
                Err(LegalizationError::NonCanonicalLegalizedPlan)
            );
        };

        let mut corrupted = original.clone();
        corrupted.scalar_call_unit_functions[0]
            .operations
            .swap(2, 3);
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        call_mut(&mut corrupted, 1).callee = caller_machine();
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        call_mut(&mut corrupted, 0).call_plan.parameters.swap(0, 1);
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        let replacement = call_mut(&mut corrupted, 0).arguments[1].source;
        call_mut(&mut corrupted, 0).arguments[0].source = replacement;
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        let replacement = call_mut(&mut corrupted, 0).arguments[1].placement.clone();
        call_mut(&mut corrupted, 0).arguments[0].placement = replacement;
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        call_mut(&mut corrupted, 1).result_home.source_value =
            ValueId::new(SCALAR_CALL_UNIT_FIRST_RESULT).unwrap();
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        corrupted.scalar_call_unit_functions[0]
            .provenance
            .operations
            .swap(0, 1);
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        call_mut(&mut corrupted, 0).fuel[0].units += 1;
        expect_rejected(corrupted);
    }
}

#[test]
fn selected_clobbers_fixed_views_and_call_evidence_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_selected(target);
        let caller_index = staged
            .selected()
            .plan()
            .functions
            .iter()
            .position(|function| function.machine == caller_machine())
            .unwrap();

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[caller_index].blocks[0].instructions[8]
            .clobbers
            .pop();
        assert!(validate_raw_selection(&staged, corrupted).is_err());

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[caller_index].blocks[0].instructions[8].operands[0].fixed_view = None;
        assert!(validate_raw_selection(&staged, corrupted).is_err());

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[caller_index].blocks[0].instructions[8]
            .provenance
            .operations[0] = OperationId::new(SCALAR_CALL_UNIT_FIRST_CALL).unwrap();
        assert!(validate_raw_selection(&staged, corrupted).is_err());

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[caller_index].blocks[0].instructions[8]
            .provenance
            .fuel[0]
            .units += 1;
        assert!(validate_raw_selection(&staged, corrupted).is_err());

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[caller_index].blocks[0].instructions[8].kind =
            SelectedInstructionKind::CallI64 {
                callee: caller_machine(),
            };
        assert_ne!(
            selected_instruction_plan_identity(staged.selected().plan()),
            selected_instruction_plan_identity(&corrupted)
        );
        assert!(validate_raw_selection(&staged, corrupted).is_err());
    }
}

#[test]
fn cross_target_selected_and_allocator_receipts_fail_closed() {
    let x64_selected = staged_selected(NativeTarget::linux_x64());
    let arm_selected_plan = staged_selected(NativeTarget::linux_arm64())
        .selected()
        .plan()
        .clone();
    assert!(validate_raw_selection(&x64_selected, arm_selected_plan).is_err());

    let x64 = staged_homes(NativeTarget::linux_x64());
    let arm = staged_homes(NativeTarget::linux_arm64());
    let arm_legality = arm.legality_stage();
    let arm_ranges = arm_legality.live_range_stage();
    let arm_environment = arm_ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    assert_eq!(
        validate_register_homes(
            arm_legality.legality(),
            arm_ranges.ranges(),
            arm_environment.identity(),
            arm_environment.physical(),
            arm_environment.constraints(),
            arm_environment.reservations(),
            &arm_environment.allocation_constraint_keys(),
            x64.homes().plan().clone(),
        ),
        Err(RegisterHomeError::RootMismatch)
    );
}
