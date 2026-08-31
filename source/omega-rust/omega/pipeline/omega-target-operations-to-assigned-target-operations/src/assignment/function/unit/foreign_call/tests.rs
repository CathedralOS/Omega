use super::normalized::assign_normalized_foreign_scalar_arguments_for_plan;
use crate::assignment::shared::{
    CallSignature, CallingPolicy, MachineRegister, NativeTarget, TargetUnitOperation, ValueId,
    ValueLocation, ValueShape,
};
use psi_core::{IntegerSign, IntegerType, IntegerValue, OperationId};

fn fixture(
    target: NativeTarget,
) -> (
    omega_calling_conventions::BoundaryEntryPlan,
    omega_target_operations::NormalizedForeignScalarArgument,
    Vec<TargetUnitOperation>,
) {
    let source_value = ValueId::new(71).expect("source");
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let shape = ValueShape::integer(4, 4);
    let plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .expect("evaluated foreign plan")
    .plan()
    .clone();
    let argument = omega_target_operations::NormalizedForeignScalarArgument {
        source_value,
        scalar_type,
        immediate: IntegerValue::Signed(-19),
        parameter_index: 0,
        placement: plan.call.parameters[0].clone(),
    };
    let preceding = vec![TargetUnitOperation::IntegerConstant {
        psi_operation: OperationId::new(72).expect("operation"),
        result: source_value,
        scalar_type,
        value: IntegerValue::Signed(-19),
    }];
    (plan, argument, preceding)
}

#[test]
fn assignment_replays_literal_and_exact_register_placement_on_both_linux_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (plan, argument, preceding) = fixture(target);
        assert_eq!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &plan,
                target,
                std::slice::from_ref(&argument),
                &preceding,
            ),
            Ok(vec![argument])
        );
    }

    let target = NativeTarget::linux_x64();
    let zero_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature::default(),
    )
    .expect("zero-argument plan")
    .plan()
    .clone();
    assert_eq!(
        assign_normalized_foreign_scalar_arguments_for_plan(&zero_plan, target, &[], &[],),
        Ok(Vec::new())
    );
}

#[test]
fn assignment_replays_two_ordered_register_literals_on_both_linux_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let first_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let second_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let first_source = ValueId::new(81).expect("first source");
        let second_source = ValueId::new(82).expect("second source");
        let plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(2, 2), ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .expect("two-register foreign plan")
        .plan()
        .clone();
        let arguments = vec![
            omega_target_operations::NormalizedForeignScalarArgument {
                source_value: first_source,
                scalar_type: first_type,
                immediate: IntegerValue::Unsigned(513),
                parameter_index: 0,
                placement: plan.call.parameters[0].clone(),
            },
            omega_target_operations::NormalizedForeignScalarArgument {
                source_value: second_source,
                scalar_type: second_type,
                immediate: IntegerValue::Signed(-29),
                parameter_index: 1,
                placement: plan.call.parameters[1].clone(),
            },
        ];
        let preceding = vec![
            TargetUnitOperation::IntegerConstant {
                psi_operation: OperationId::new(83).expect("first constant"),
                result: first_source,
                scalar_type: first_type,
                value: IntegerValue::Unsigned(513),
            },
            TargetUnitOperation::IntegerConstant {
                psi_operation: OperationId::new(84).expect("second constant"),
                result: second_source,
                scalar_type: second_type,
                value: IntegerValue::Signed(-29),
            },
        ];
        assert_eq!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &plan, target, &arguments, &preceding,
            ),
            Ok(arguments.clone())
        );

        let mut stack_argument = arguments;
        stack_argument[1].placement.locations = vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }];
        assert!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &plan,
                target,
                &stack_argument,
                &preceding,
            )
            .is_err()
        );
    }
}

#[test]
fn assignment_rejects_literal_identity_type_value_order_and_placement_drift() {
    let target = NativeTarget::linux_x64();
    let (plan, argument, preceding) = fixture(target);
    let mut mutations = Vec::new();

    let mut changed_source = argument.clone();
    changed_source.source_value = ValueId::new(73).expect("changed source");
    mutations.push(changed_source);

    let mut changed_type = argument.clone();
    changed_type.scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    mutations.push(changed_type);

    let mut changed_value = argument.clone();
    changed_value.immediate = IntegerValue::Signed(-18);
    mutations.push(changed_value);

    let mut changed_order = argument.clone();
    changed_order.parameter_index = 1;
    mutations.push(changed_order);

    let mut changed_placement = argument.clone();
    changed_placement.placement.locations = vec![ValueLocation::Register {
        register: MachineRegister::X86Rsi,
        value_byte_offset: 0,
        byte_size: 4,
    }];
    mutations.push(changed_placement);

    let mut stack_placement = argument.clone();
    stack_placement.placement.locations = vec![ValueLocation::Stack {
        stack_byte_offset: 0,
        value_byte_offset: 0,
        byte_size: 4,
        alignment: 4,
    }];
    mutations.push(stack_placement);

    for mutation in mutations {
        assert!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &plan,
                target,
                &[mutation],
                &preceding,
            )
            .is_err()
        );
    }

    assert!(
        assign_normalized_foreign_scalar_arguments_for_plan(
            &plan,
            target,
            &[argument.clone(), argument.clone()],
            &preceding,
        )
        .is_err()
    );
    let mut result_plan = plan;
    result_plan.call.result = Some(argument.placement.clone());
    assert!(
        assign_normalized_foreign_scalar_arguments_for_plan(
            &result_plan,
            target,
            &[argument.clone()],
            &preceding,
        )
        .is_err()
    );

    for (target, register_count, expected_last_register) in [
        (NativeTarget::linux_x64(), 6, MachineRegister::X86R9),
        (NativeTarget::linux_arm64(), 8, MachineRegister::Aarch64X(7)),
    ] {
        let plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4); register_count],
                result: None,
            },
        )
        .unwrap()
        .plan()
        .clone();
        let arguments = (0..register_count)
            .map(
                |index| omega_target_operations::NormalizedForeignScalarArgument {
                    source_value: argument.source_value,
                    scalar_type: argument.scalar_type,
                    immediate: argument.immediate,
                    parameter_index: u32::try_from(index).unwrap(),
                    placement: plan.call.parameters[index].clone(),
                },
            )
            .collect::<Vec<_>>();
        assert!(matches!(
            arguments[register_count - 1].placement.locations.as_slice(),
            [ValueLocation::Register { register, .. }] if *register == expected_last_register
        ));
        assert_eq!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &plan, target, &arguments, &preceding,
            ),
            Ok(arguments)
        );

        let first_stack_count = register_count + 1;
        let first_stack_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4); first_stack_count],
                result: None,
            },
        )
        .unwrap()
        .plan()
        .clone();
        assert!(matches!(
            first_stack_plan
                .call
                .parameters
                .last()
                .unwrap()
                .locations
                .as_slice(),
            [ValueLocation::Stack { .. }]
        ));
        let first_stack_arguments = (0..first_stack_count)
            .map(
                |index| omega_target_operations::NormalizedForeignScalarArgument {
                    source_value: argument.source_value,
                    scalar_type: argument.scalar_type,
                    immediate: argument.immediate,
                    parameter_index: u32::try_from(index).unwrap(),
                    placement: first_stack_plan.call.parameters[index].clone(),
                },
            )
            .collect::<Vec<_>>();
        assert!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &first_stack_plan,
                target,
                &first_stack_arguments,
                &preceding,
            )
            .is_err(),
            "the first stack-resident literal remains fenced on {target:?}",
        );
    }

    let (_, mut wrong_policy_argument, preceding) = fixture(target);
    let wrong_policy_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: None,
        },
    )
    .unwrap()
    .plan()
    .clone();
    wrong_policy_argument.placement = wrong_policy_plan.call.parameters[0].clone();
    assert!(
        assign_normalized_foreign_scalar_arguments_for_plan(
            &wrong_policy_plan,
            target,
            &[wrong_policy_argument],
            &preceding,
        )
        .is_err()
    );
}
