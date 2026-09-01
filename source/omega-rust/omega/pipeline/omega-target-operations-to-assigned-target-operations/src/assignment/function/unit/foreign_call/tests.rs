use super::normalized::{
    assign_normalized_foreign_scalar_arguments_for_plan,
    assign_normalized_foreign_scalar_call_for_plan,
};
use crate::assignment::shared::{
    BTreeMap, CallSignature, CallingPolicy, MachineRegister, NativeTarget, TargetUnitOperation,
    ValueId, ValueLocation, ValueShape,
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
        parameter_index: 0,
        source: omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation: OperationId::new(72).expect("operation"),
            source_value,
            scalar_type,
            value: IntegerValue::Signed(-19),
        },
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

fn assigned_argument(
    argument: &omega_target_operations::NormalizedForeignScalarArgument,
) -> omega_assigned_target_operations::AssignedNormalizedForeignScalarArgument {
    let source = match argument.source {
        omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        },
        omega_target_operations::TargetUnitScalarArgumentSource::Home(_) => {
            panic!("literal helper receives an immediate")
        }
    };
    omega_assigned_target_operations::AssignedNormalizedForeignScalarArgument {
        parameter_index: argument.parameter_index,
        source,
        placement: argument.placement.clone(),
    }
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
                &BTreeMap::new(),
            ),
            Ok(vec![assigned_argument(&argument)])
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
        assign_normalized_foreign_scalar_arguments_for_plan(
            &zero_plan,
            target,
            &[],
            &[],
            &BTreeMap::new(),
        ),
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
                parameter_index: 0,
                source: omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation: OperationId::new(83).expect("first constant"),
                    source_value: first_source,
                    scalar_type: first_type,
                    value: IntegerValue::Unsigned(513),
                },
                placement: plan.call.parameters[0].clone(),
            },
            omega_target_operations::NormalizedForeignScalarArgument {
                parameter_index: 1,
                source: omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation: OperationId::new(84).expect("second constant"),
                    source_value: second_source,
                    scalar_type: second_type,
                    value: IntegerValue::Signed(-29),
                },
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
                &plan,
                target,
                &arguments,
                &preceding,
                &BTreeMap::new(),
            ),
            Ok(arguments.iter().map(assigned_argument).collect())
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
                &BTreeMap::new(),
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
    if let omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
        source_value,
        ..
    } = &mut changed_source.source
    {
        *source_value = ValueId::new(73).expect("changed source");
    }
    mutations.push(changed_source);

    let mut changed_type = argument.clone();
    if let omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
        scalar_type,
        ..
    } = &mut changed_type.source
    {
        *scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    }
    mutations.push(changed_type);

    let mut changed_value = argument.clone();
    if let omega_target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
        value, ..
    } = &mut changed_value.source
    {
        *value = IntegerValue::Signed(-18);
    }
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
                &BTreeMap::new(),
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
            &BTreeMap::new(),
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
            &BTreeMap::new(),
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
                    parameter_index: u32::try_from(index).unwrap(),
                    source: argument.source,
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
                &plan,
                target,
                &arguments,
                &preceding,
                &BTreeMap::new(),
            ),
            Ok(arguments.iter().map(assigned_argument).collect())
        );

        let stack_argument_count = register_count + 2;
        let stack_argument_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4); stack_argument_count],
                result: None,
            },
        )
        .unwrap()
        .plan()
        .clone();
        assert!(matches!(
            stack_argument_plan
                .call
                .parameters
                .get(register_count)
                .unwrap()
                .locations
                .as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 0,
                ..
            }]
        ));
        assert!(matches!(
            stack_argument_plan
                .call
                .parameters
                .last()
                .unwrap()
                .locations
                .as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 8,
                ..
            }]
        ));
        let stack_arguments = (0..stack_argument_count)
            .map(
                |index| omega_target_operations::NormalizedForeignScalarArgument {
                    parameter_index: u32::try_from(index).unwrap(),
                    source: argument.source,
                    placement: stack_argument_plan.call.parameters[index].clone(),
                },
            )
            .collect::<Vec<_>>();
        assert_eq!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &stack_argument_plan,
                target,
                &stack_arguments,
                &preceding,
                &BTreeMap::new(),
            ),
            Ok(stack_arguments.iter().map(assigned_argument).collect()),
            "assignment retains the complete canonical register-and-stack plan on {target:?}",
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
            &BTreeMap::new(),
        )
        .is_err()
    );
}

#[test]
fn assignment_replays_the_complete_fixed_integer_result_family_and_rejects_drift() {
    let operation = OperationId::new(401).unwrap();
    let source = ValueId::new(402).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for (sign, bits) in [
            (IntegerSign::Signed, 8),
            (IntegerSign::Unsigned, 8),
            (IntegerSign::Signed, 16),
            (IntegerSign::Unsigned, 16),
            (IntegerSign::Signed, 32),
            (IntegerSign::Unsigned, 32),
            (IntegerSign::Signed, 64),
            (IntegerSign::Unsigned, 64),
        ] {
            let integer = IntegerType::new(sign, bits).unwrap();
            let bytes = bits.div_ceil(8);
            let shape = ValueShape::integer(bytes, bytes.next_power_of_two().min(8));
            let plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: Vec::new(),
                    result: Some(shape),
                },
            )
            .unwrap()
            .plan()
            .clone();
            let result = omega_target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: operation,
                source_value: source,
                scalar_type: integer,
                shape,
            };
            assert_eq!(
                assign_normalized_foreign_scalar_call_for_plan(
                    &plan,
                    target,
                    &[],
                    Some(&result),
                    operation,
                    &[],
                    &BTreeMap::new(),
                ),
                Ok(Vec::new())
            );

            let mut wrong_shape = result;
            wrong_shape.shape.alignment = wrong_shape.shape.alignment.saturating_add(1);
            assert!(
                assign_normalized_foreign_scalar_call_for_plan(
                    &plan,
                    target,
                    &[],
                    Some(&wrong_shape),
                    operation,
                    &[],
                    &BTreeMap::new(),
                )
                .is_err()
            );
            assert!(
                assign_normalized_foreign_scalar_call_for_plan(
                    &plan,
                    target,
                    &[],
                    Some(&result),
                    OperationId::new(403).unwrap(),
                    &[],
                    &BTreeMap::new(),
                )
                .is_err()
            );

            let mut wrong_fragment = plan.clone();
            let ValueLocation::Register { byte_size, .. } =
                &mut wrong_fragment.call.result.as_mut().unwrap().locations[0]
            else {
                unreachable!()
            };
            *byte_size = byte_size.saturating_add(1);
            assert!(
                assign_normalized_foreign_scalar_call_for_plan(
                    &wrong_fragment,
                    target,
                    &[],
                    Some(&result),
                    operation,
                    &[],
                    &BTreeMap::new(),
                )
                .is_err()
            );
        }
    }

    for integer in [
        IntegerType::new(IntegerSign::Signed, 24).unwrap(),
        IntegerType::address(64).unwrap(),
    ] {
        let plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(NativeTarget::linux_x64()),
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap()
        .plan()
        .clone();
        let invalid = omega_target_operations::TargetUnitScalarHomeRequirement {
            defining_operation: operation,
            source_value: source,
            scalar_type: integer,
            shape: ValueShape::integer(8, 8),
        };
        assert!(
            assign_normalized_foreign_scalar_call_for_plan(
                &plan,
                NativeTarget::linux_x64(),
                &[],
                Some(&invalid),
                operation,
                &[],
                &BTreeMap::new(),
            )
            .is_err()
        );
    }
}
