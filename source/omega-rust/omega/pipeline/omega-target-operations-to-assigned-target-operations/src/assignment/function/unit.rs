use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    Ok(match operation {
        TargetOperation::UnitBody(body) => {
            let operations = body
                .operations
                .iter()
                .enumerate()
                .map(|(operation_index, operation)| {
                    Ok(match operation {
                        TargetUnitOperation::EstablishByteSequenceLiteral {
                            psi_operation,
                            place,
                            structural_type,
                            bytes,
                        } => AssignedUnitOperation::EstablishByteSequenceLiteral {
                            psi_operation: *psi_operation,
                            place: place.clone(),
                            structural_type: structural_type.clone(),
                            bytes: bytes.clone(),
                        },
                        TargetUnitOperation::IntegerConstant {
                            psi_operation,
                            result,
                            scalar_type,
                            value,
                        } => AssignedUnitOperation::IntegerConstant {
                            psi_operation: *psi_operation,
                            result: *result,
                            scalar_type: *scalar_type,
                            value: *value,
                        },
                        TargetUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation,
                            place,
                            structural_type,
                        } => AssignedUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation: *psi_operation,
                            place: place.clone(),
                            structural_type: structural_type.clone(),
                        },
                        TargetUnitOperation::Call {
                            psi_operation,
                            callee,
                            arguments,
                            claim_transfers,
                        } => AssignedUnitOperation::Call {
                            psi_operation: *psi_operation,
                            callee: *callee,
                            result: None,
                            copies: arguments
                                .iter()
                                .map(|argument| AssignedAggregateCopy {
                                    place: argument.place,
                                    access: argument.access,
                                    path: argument.path.clone(),
                                    root_structural_type: argument.root_structural_type,
                                    structural_type: argument.structural_type,
                                    shape: argument.shape,
                                    source_byte_offset: argument.source_byte_offset,
                                    fixed_array_length: argument.fixed_array_length,
                                    element_stride: argument.element_stride,
                                    source: argument.source.clone(),
                                    destination: argument.destination.clone(),
                                })
                                .collect(),
                            claim_transfers: claim_transfers.clone(),
                        },
                        TargetUnitOperation::InstalledProviderCall {
                            psi_operation,
                            boundary,
                            ..
                        } => {
                            return Err(
                                AssignmentError::InstalledProviderCallRequiresOptimizedLane {
                                    machine: function.machine,
                                    operation: *psi_operation,
                                    boundary: *boundary,
                                },
                            );
                        }
                        TargetUnitOperation::NormalizedForeignCall {
                            psi_operation,
                            boundary,
                            provider_execution,
                            binding,
                            scalar_arguments,
                        } => AssignedUnitOperation::NormalizedForeignCall {
                            psi_operation: *psi_operation,
                            boundary: *boundary,
                            provider_execution: *provider_execution,
                            binding: binding.clone(),
                            scalar_arguments: assign_normalized_foreign_scalar_arguments(
                                binding,
                                target,
                                scalar_arguments,
                                &body.operations[..operation_index],
                            )?,
                        },
                        TargetUnitOperation::PortWrite {
                            psi_operation,
                            service,
                            port,
                            value,
                        } => AssignedUnitOperation::PortWrite {
                            psi_operation: *psi_operation,
                            service: *service,
                            port: *port,
                            value: *value,
                        },
                        TargetUnitOperation::BoundarySettlement {
                            psi_operation,
                            boundary,
                            provider_execution,
                            realization,
                            scalar_arguments,
                            arguments,
                            byte_sequence_arguments,
                            completion_claim_sources,
                            completion_receipts,
                        } => AssignedUnitOperation::BoundarySettlement {
                            psi_operation: *psi_operation,
                            boundary: *boundary,
                            provider_execution: *provider_execution,
                            realization: *realization,
                            scalar_arguments: scalar_arguments.clone(),
                            arguments: arguments.clone(),
                            byte_sequence_arguments: byte_sequence_arguments.clone(),
                            completion_claim_sources: completion_claim_sources.clone(),
                            completion_receipts: completion_receipts.clone(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge,
                            cleanup_actions,
                        } => AssignedUnitOperation::Return {
                            psi_edge: *psi_edge,
                            cleanup_actions: cleanup_actions.clone(),
                        },
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            AssignedOperation::UnitBody(AssignedUnitBody {
                structural_types: body.structural_types.clone(),
                call_plan: body.call_plan.clone(),
                parameters: body.parameters.clone(),
                operations,
            })
        }
        _ => unreachable!("Unit assignment receives a Unit body"),
    })
}

fn assign_normalized_foreign_scalar_arguments(
    binding: &omega_target_operations::NormalizedForeignCallBinding,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    preceding_operations: &[TargetUnitOperation],
) -> Result<Vec<omega_target_operations::NormalizedForeignScalarArgument>, AssignmentError> {
    if binding.locator.target().native_target() != target
        || target.object_format != omega_target::ObjectFormat::Elf
        || !matches!(
            binding.locator.locator(),
            omega_target::ForeignLocatorCandidate::ElfVersioned { .. }
        )
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    assign_normalized_foreign_scalar_arguments_for_plan(
        &binding.boundary_entry_plan,
        target,
        scalar_arguments,
        preceding_operations,
    )
}

fn assign_normalized_foreign_scalar_arguments_for_plan(
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    preceding_operations: &[TargetUnitOperation],
) -> Result<Vec<omega_target_operations::NormalizedForeignScalarArgument>, AssignmentError> {
    if scalar_arguments.len() > 3 {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    let signature = CallSignature {
        parameters: scalar_arguments
            .iter()
            .map(|argument| argument.placement.shape)
            .collect(),
        result: None,
    };
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
    let canonical = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &signature,
    )
    .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
    if validated.plan() != boundary_entry_plan
        || canonical.plan() != boundary_entry_plan
        || boundary_entry_plan.call.result.is_some()
        || boundary_entry_plan.call.parameters.len() != scalar_arguments.len()
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = argument.placement.locations.as_slice()
        else {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: argument.source_value,
                parameter_index,
            });
        };
        let expected_bytes = argument.scalar_type.bits().div_ceil(8);
        if argument.scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
            || !matches!(argument.scalar_type.bits(), 8 | 16 | 32 | 64)
            || argument.parameter_index != parameter_index as u32
            || argument.placement != boundary_entry_plan.call.parameters[parameter_index]
            || argument.placement.shape
                != ValueShape::integer(expected_bytes, expected_bytes.next_power_of_two().min(8))
            || u16::try_from(expected_bytes) != Ok(*byte_size)
            || psi_core::ScalarTerm::integer(argument.scalar_type, argument.immediate).is_err()
        {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: argument.source_value,
                parameter_index,
            });
        }
        let matching_constants = preceding_operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::IntegerConstant {
                    result,
                    scalar_type,
                    value,
                    ..
                } if *result == argument.source_value => Some((*scalar_type, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if matching_constants.as_slice() != [(argument.scalar_type, argument.immediate)] {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: argument.source_value,
                parameter_index,
            });
        }
        crate::assignment::placement::require_register_architecture(
            argument.source_value,
            *register,
            target.architecture,
        )?;
    }
    Ok(scalar_arguments.to_vec())
}

#[cfg(test)]
mod normalized_foreign_scalar_tests {
    use super::*;
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

        let three_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4); 3],
                result: None,
            },
        )
        .unwrap()
        .plan()
        .clone();
        let three_arguments = (0..3)
            .map(
                |index| omega_target_operations::NormalizedForeignScalarArgument {
                    source_value: argument.source_value,
                    scalar_type: argument.scalar_type,
                    immediate: argument.immediate,
                    parameter_index: index,
                    placement: three_plan.call.parameters[index as usize].clone(),
                },
            )
            .collect::<Vec<_>>();
        assert_eq!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &three_plan,
                target,
                &three_arguments,
                &preceding,
            ),
            Ok(three_arguments.clone())
        );

        let four_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4); 4],
                result: None,
            },
        )
        .unwrap()
        .plan()
        .clone();
        let four_arguments = (0..4)
            .map(
                |index| omega_target_operations::NormalizedForeignScalarArgument {
                    source_value: argument.source_value,
                    scalar_type: argument.scalar_type,
                    immediate: argument.immediate,
                    parameter_index: index,
                    placement: four_plan.call.parameters[index as usize].clone(),
                },
            )
            .collect::<Vec<_>>();
        assert!(
            assign_normalized_foreign_scalar_arguments_for_plan(
                &four_plan,
                target,
                &four_arguments,
                &preceding,
            )
            .is_err()
        );

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
}
