use super::*;

fn fixed_integer(bits: u16) -> IntegerType {
    IntegerType::new(IntegerSign::Signed, bits).expect("fixed integer")
}

fn scalar_parameter(value: u64, scalar_type: IntegerType) -> AbstractParameter {
    AbstractParameter {
        value: ValueId::new(value).expect("parameter value"),
        scalar_type: ScalarType::Integer(scalar_type),
    }
}

pub(super) fn attached_unit_scalar_call_plan() -> AbstractOperationPlan {
    let attached_machine = MachineId::new(1).expect("attached machine");
    let scalar_machine = MachineId::new(2).expect("scalar machine");
    let attachment = StructuralTypeId::new(1).expect("attachment");
    let integer_type = fixed_integer(32);
    let scalar_type = ScalarType::Integer(integer_type);
    let constant = ValueId::new(10).expect("constant");
    let first_result = ValueId::new(11).expect("first result");
    let second_result = ValueId::new(12).expect("second result");
    let callee_parameter = scalar_parameter(20, integer_type);
    let callee_result = ValueId::new(21).expect("callee result");

    AbstractOperationPlan {
        psi: identity(),
        entry: attached_machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: attached_machine,
                attachment: Some(attachment),
                entry: BlockId::new(1).expect("attached entry"),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: BlockId::new(1).expect("attached entry"),
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(10).expect("constant operation"),
                        result: constant,
                        scalar_type,
                        value: IntegerValue::Signed(17),
                    },
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(11).expect("first call"),
                        result: first_result,
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![constant],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(12).expect("second call"),
                        result: second_result,
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![first_result],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).expect("return edge"),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: scalar_machine,
                attachment: None,
                entry: BlockId::new(2).expect("callee entry"),
                parameters: vec![callee_parameter],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: callee_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![AbstractOperation::Return {
                    psi_edge: EdgeId::new(2).expect("callee return"),
                    result: callee_result,
                    value: callee_parameter.value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}

#[test]
fn attached_unit_calls_retain_immediates_and_prior_results_with_durable_homes() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
    ] {
        let lowered = lower_to_target_operations(&attached_unit_scalar_call_plan(), target)
            .expect("attached Unit scalar calls lower");
        let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("attached machine must remain a Unit body")
        };
        assert_eq!(lowered.functions[0].fixed_integer_scalar_abi, None);
        let [
            TargetUnitOperation::IntegerConstant { .. },
            TargetUnitOperation::ScalarCall {
                psi_operation: first_operation,
                call_plan: first_plan,
                result_home: first_home,
                arguments: first_arguments,
                ..
            },
            TargetUnitOperation::ScalarCall {
                psi_operation: second_operation,
                call_plan: second_plan,
                result_home: second_home,
                arguments: second_arguments,
                ..
            },
            TargetUnitOperation::Return { .. },
        ] = body.operations.as_slice()
        else {
            panic!("Unit body must retain both scalar calls in order")
        };

        assert_eq!(first_home.defining_operation, *first_operation);
        assert_eq!(first_home.source_value, ValueId::new(11).unwrap());
        assert_eq!(
            first_home.scalar_type,
            psi_core::ScalarType::Integer(fixed_integer(32))
        );
        assert_eq!(first_home.shape, first_plan.result.as_ref().unwrap().shape);
        assert_eq!(second_home.defining_operation, *second_operation);
        assert_eq!(second_home.source_value, ValueId::new(12).unwrap());
        assert_eq!(
            second_home.shape,
            second_plan.result.as_ref().unwrap().shape
        );

        assert_eq!(first_arguments.len(), 1);
        assert_eq!(first_arguments[0].parameter_index, 0);
        assert_eq!(first_arguments[0].placement, first_plan.parameters[0]);
        assert!(matches!(
            first_arguments[0].source,
            TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value: IntegerValue::Signed(17),
            } if defining_operation == OperationId::new(10).unwrap()
                && source_value == ValueId::new(10).unwrap()
                && scalar_type == fixed_integer(32)
        ));

        assert_eq!(second_arguments.len(), 1);
        assert_eq!(second_arguments[0].parameter_index, 0);
        assert_eq!(second_arguments[0].placement, second_plan.parameters[0]);
        assert_eq!(
            second_arguments[0].source,
            TargetUnitScalarArgumentSource::Home(*first_home)
        );
        assert_eq!(
            body.operations
                .iter()
                .filter(|operation| matches!(operation, TargetUnitOperation::ScalarCall { .. }))
                .count(),
            2
        );
        let callee_abi = lowered.functions[1]
            .fixed_integer_scalar_abi
            .as_ref()
            .expect("service-free fixed-integer callee ABI");
        assert_eq!(callee_abi.call_plan, *first_plan);
        assert_eq!(callee_abi.parameters.len(), 1);
        assert_eq!(callee_abi.parameters[0].value, ValueId::new(20).unwrap());
        assert_eq!(callee_abi.parameters[0].scalar_type, fixed_integer(32));
        assert_eq!(callee_abi.parameters[0].placement, first_plan.parameters[0]);
        assert_eq!(callee_abi.result.value, ValueId::new(21).unwrap());
        assert_eq!(callee_abi.result.scalar_type, fixed_integer(32));
        assert_eq!(
            callee_abi.result.placement,
            first_plan.result.clone().unwrap()
        );
    }
}

#[test]
fn attached_unit_calls_retain_ordered_register_and_stack_arguments() {
    let mut plan = attached_unit_scalar_call_plan();
    let integer_type = fixed_integer(32);
    let scalar_type = ScalarType::Integer(integer_type);
    let extra_constants = (1..9)
        .map(|index| AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(20 + index).unwrap(),
            result: ValueId::new(30 + index).unwrap(),
            scalar_type,
            value: IntegerValue::Signed(index.into()),
        })
        .collect::<Vec<_>>();
    plan.functions[0].operations.splice(1..1, extra_constants);
    let constant_arguments = std::iter::once(ValueId::new(10).unwrap())
        .chain((1..9).map(|index| ValueId::new(30 + index).unwrap()))
        .collect::<Vec<_>>();
    let prior_result_arguments = vec![ValueId::new(11).unwrap(); 9];
    let mut calls =
        plan.functions[0]
            .operations
            .iter_mut()
            .filter_map(|operation| match operation {
                AbstractOperation::Call { arguments, .. } => Some(arguments),
                _ => None,
            });
    *calls.next().unwrap() = constant_arguments;
    *calls.next().unwrap() = prior_result_arguments;
    assert!(calls.next().is_none());
    plan.functions[1].parameters = (0..9)
        .map(|index| scalar_parameter(20 + index, integer_type))
        .collect();
    let returned_parameter = plan.functions[1].parameters[0].value;
    let AbstractOperation::Return { value, .. } = &mut plan.functions[1].operations[0] else {
        unreachable!()
    };
    *value = returned_parameter;

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target).expect("nine scalar arguments");
        let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            unreachable!()
        };
        let calls = body
            .operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::ScalarCall {
                    call_plan,
                    arguments,
                    ..
                } => Some((call_plan, arguments)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        for (call_plan, arguments) in calls {
            assert_eq!(arguments.len(), 9);
            assert!(matches!(
                arguments[8].placement.locations.as_slice(),
                [ValueLocation::Stack { .. }]
            ));
            assert!(arguments.iter().enumerate().all(|(index, argument)| {
                argument.parameter_index == index as u32
                    && argument.placement == call_plan.parameters[index]
            }));
        }
    }
}

#[test]
fn unit_scalar_calls_require_an_attachment_and_service_free_scalar_callee() {
    let mut unattached = attached_unit_scalar_call_plan();
    unattached.functions[0].attachment = None;
    assert_eq!(
        lower_to_target_operations(&unattached, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallRequiresAttachedMachine {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(11).unwrap(),
        })
    );

    let mut serviceful = attached_unit_scalar_call_plan();
    serviceful.functions[1]
        .published_service_ceiling
        .push(psi_core::ServiceId::new(1).expect("service"));
    assert_eq!(
        lower_to_target_operations(&serviceful, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallTargetPublishesServices(
            MachineId::new(2).unwrap()
        ))
    );
}

#[test]
fn unit_scalar_calls_reject_wrong_arity_type_and_unknown_values() {
    let mut wrong_arity = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { arguments, .. } = &mut wrong_arity.functions[0].operations[1]
    else {
        unreachable!()
    };
    arguments.clear();
    assert_eq!(
        lower_to_target_operations(&wrong_arity, NativeTarget::linux_x64()),
        Err(LoweringError::CallArgumentCountMismatch {
            callee: MachineId::new(2).unwrap(),
            expected: 1,
            actual: 0,
        })
    );

    let mut wrong_type = attached_unit_scalar_call_plan();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let AbstractOperation::IntegerConstant {
        scalar_type, value, ..
    } = &mut wrong_type.functions[0].operations[0]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(unsigned);
    *value = IntegerValue::Unsigned(17);
    assert_eq!(
        lower_to_target_operations(&wrong_type, NativeTarget::linux_x64()),
        Err(LoweringError::CallArgumentTypeMismatch {
            callee: MachineId::new(2).unwrap(),
            argument: ValueId::new(10).unwrap(),
        })
    );

    let mut unknown = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { arguments, .. } = &mut unknown.functions[0].operations[1] else {
        unreachable!()
    };
    arguments[0] = ValueId::new(99).expect("unknown value");
    assert_eq!(
        lower_to_target_operations(&unknown, NativeTarget::linux_x64()),
        Err(LoweringError::UnknownValue(ValueId::new(99).unwrap()))
    );
}

#[test]
fn unit_scalar_calls_reject_address_boolean_wide_and_structural_shapes() {
    let mut address = attached_unit_scalar_call_plan();
    let address_type = IntegerType::address(64).expect("address carrier");
    let AbstractOperation::Call { scalar_type, .. } = &mut address.functions[0].operations[1]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(address_type);
    address.functions[1].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(21).unwrap(),
        scalar_type: ScalarType::Integer(address_type),
    });
    assert_eq!(
        lower_to_target_operations(&address, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            ValueId::new(11).unwrap()
        ))
    );

    let mut boolean = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { scalar_type, .. } = &mut boolean.functions[0].operations[1]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Boolean;
    boolean.functions[1].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(21).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    assert_eq!(
        lower_to_target_operations(&boolean, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            ValueId::new(11).unwrap()
        ))
    );

    let mut wide = attached_unit_scalar_call_plan();
    let wide_type = fixed_integer(128);
    let AbstractOperation::Call { scalar_type, .. } = &mut wide.functions[0].operations[1] else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(wide_type);
    wide.functions[1].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(21).unwrap(),
        scalar_type: ScalarType::Integer(wide_type),
    });
    assert_eq!(
        lower_to_target_operations(&wide, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            ValueId::new(11).unwrap()
        ))
    );

    let mut structural = attached_unit_scalar_call_plan();
    structural.functions[1]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    assert_eq!(
        lower_to_target_operations(&structural, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallTargetShapeUnsupported(
            MachineId::new(2).unwrap()
        ))
    );
}

#[test]
fn unit_scalar_calls_reject_declared_result_and_argument_carrier_drift() {
    let mut result_mismatch = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { scalar_type, .. } =
        &mut result_mismatch.functions[0].operations[1]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(fixed_integer(64));
    assert_eq!(
        lower_to_target_operations(&result_mismatch, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallResultTypeMismatch {
            callee: MachineId::new(2).unwrap(),
            result: ValueId::new(11).unwrap(),
        })
    );

    let mut address_argument = attached_unit_scalar_call_plan();
    let address_type = IntegerType::address(64).unwrap();
    address_argument.functions[1].parameters[0].scalar_type = ScalarType::Integer(address_type);
    assert_eq!(
        lower_to_target_operations(&address_argument, NativeTarget::linux_x64()),
        Err(LoweringError::UnitScalarCallTargetShapeUnsupported(
            MachineId::new(2).unwrap()
        ))
    );
}
