use crate::tests::*;

use super::fixture::{caller_machine, staged_homes, staged_selected};

fn call_mut(
    plan: &mut legalized_operations::LegalizedOperationPlan,
    index: usize,
) -> &mut legalized_operations::LegalizedScalarCall {
    let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
        &mut plan.scalar_functions[0].blocks[0].instructions[index + 2].kind
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
        corrupted.scalar_functions[0].blocks[0]
            .instructions
            .swap(2, 3);
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        call_mut(&mut corrupted, 1).callee = caller_machine();
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        call_mut(&mut corrupted, 0).call_plan.parameters.swap(0, 1);
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        let replacement = call_mut(&mut corrupted, 0).arguments[1]
            .scalar_source()
            .unwrap();
        let legalized_operations::LegalizedScalarArgument::Scalar { source, .. } =
            &mut call_mut(&mut corrupted, 0).arguments[0]
        else {
            panic!("scalar call");
        };
        *source = replacement;
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        let replacement = call_mut(&mut corrupted, 0).arguments[1].placement().clone();
        let legalized_operations::LegalizedScalarArgument::Scalar { placement, .. } =
            &mut call_mut(&mut corrupted, 0).arguments[0]
        else {
            panic!("scalar call");
        };
        *placement = replacement;
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        corrupted.scalar_functions[0].blocks[0].instructions[3]
            .result
            .as_mut()
            .unwrap()
            .value = ValueId::new(SCALAR_CALL_UNIT_FIRST_RESULT).unwrap();
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        corrupted.scalar_functions[0]
            .provenance
            .operations
            .swap(0, 1);
        expect_rejected(corrupted);

        let mut corrupted = original.clone();
        corrupted.scalar_functions[0].blocks[0].instructions[2].fuel[0].units += 1;
        expect_rejected(corrupted);
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
