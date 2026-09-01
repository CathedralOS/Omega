use super::*;

#[test]
fn selects_native_register_and_stack_locations_for_runtime_parameters() {
    let register_cases = [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
        ),
    ];
    for (target, expected) in register_cases {
        let lowered = lower_to_target_operations(&parameter_return_plan(1), target).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TargetOperation::ReturnIntegerParameter {
                parameter_index: 0,
                location,
                ..
            } if location == expected
        ));
    }

    let stack_cases = [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ];
    for (target, expected) in stack_cases {
        let lowered = lower_to_target_operations(&parameter_return_plan(9), target).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TargetOperation::ReturnIntegerParameter {
                parameter_index: 8,
                location,
                ..
            } if location == expected
        ));
    }
}

#[test]
fn direct_calls_retain_stack_locations_from_the_callee_call_plan() {
    let stack_cases = [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ];
    for (target, expected) in stack_cases {
        let lowered = lower_to_target_operations(&direct_call_plan(9), target).unwrap();
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &lowered.functions[0].operation
        else {
            panic!("caller must return its call result")
        };
        let TargetIntegerExpression::Call {
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } = expression
        else {
            panic!("caller result must remain a direct call")
        };
        assert_eq!(arguments[8].location, expected);
        assert_eq!(requirement_obligations, &[ObligationId::new(700).unwrap()]);
        assert_eq!(
            crash_continuations,
            &[CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }]
        );
    }
}

#[test]
fn lowers_runtime_parameter_arithmetic_to_a_typed_target_expression() {
    let mut plan = parameter_return_plan(2);
    let function = &mut plan.functions[0];
    let sum = ValueId::new(50).expect("sum");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => unreachable!("fixture is integer"),
    };
    function.operations.insert(
        0,
        AbstractOperation::WrappingIntegerAdd {
            psi_operation: psi_core::OperationId::new(50).expect("operation"),
            result: sum,
            scalar_type,
            left: function.parameters[0].value,
            right: function.parameters[1].value,
        },
    );
    let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
        unreachable!("fixture ends in return")
    };
    *value = sum;

    let lowered = lower_to_target_operations(&plan, NativeTarget::host()).unwrap();
    assert!(matches!(
        &lowered.functions[0].operation,
        TargetOperation::ReturnIntegerExpression {
            source_value,
            scalar_type: result_type,
            expression: TargetIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
            ..
        } if *source_value == sum
            && *result_type == scalar_type
            && *psi_operation == psi_core::OperationId::new(50).expect("operation")
            && matches!(
                left.as_ref(),
                TargetIntegerExpression::Parameter {
                    parameter_index: 0,
                    ..
                }
            )
            && matches!(
                right.as_ref(),
                TargetIntegerExpression::Parameter {
                    parameter_index: 1,
                    ..
                }
            )
    ));
}

#[test]
fn folds_closed_wrapping_subtraction_at_the_declared_width() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let difference = ValueId::new(52).expect("difference");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(5),
            },
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(10),
            },
            AbstractOperation::WrappingIntegerSubtract {
                psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                result: difference,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let AbstractOperation::Return { value, .. } = function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = difference;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(251),
            ..
        } if source_value == difference && result_type == scalar_type
    ));
}

#[test]
fn folds_closed_saturating_subtraction_at_zero() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let difference = ValueId::new(52).expect("difference");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(5),
            },
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(10),
            },
            AbstractOperation::SaturatingIntegerSubtract {
                psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                result: difference,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let AbstractOperation::Return { value, .. } = function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = difference;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(0),
            ..
        } if source_value == difference && result_type == scalar_type
    ));
}

#[test]
fn folds_closed_wrapping_multiplication_at_the_declared_width() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let product = ValueId::new(52).expect("product");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(20),
            },
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(13),
            },
            AbstractOperation::WrappingIntegerMultiply {
                psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                result: product,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let AbstractOperation::Return { value, .. } = function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = product;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(4),
            ..
        } if source_value == product && result_type == scalar_type
    ));
}

#[test]
fn folds_closed_saturating_multiplication_at_the_declared_width() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    let left = ValueId::new(50).expect("left");
    let right = ValueId::new(51).expect("right");
    let product = ValueId::new(52).expect("product");
    let scalar_type = match scalar_result(function).scalar_type {
        ScalarType::Integer(integer) => integer,
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => unreachable!("fixture is integer"),
    };
    function.operations.splice(
        0..0,
        [
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                result: left,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(20),
            },
            AbstractOperation::IntegerConstant {
                psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                result: right,
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(13),
            },
            AbstractOperation::SaturatingIntegerMultiply {
                psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                result: product,
                scalar_type,
                left,
                right,
            },
        ],
    );
    let AbstractOperation::Return { value, .. } = function.operations.last_mut().expect("return")
    else {
        unreachable!("fixture ends in return")
    };
    *value = product;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type: result_type,
            value: IntegerValue::Unsigned(255),
            ..
        } if source_value == product && result_type == scalar_type
    ));
}

#[test]
fn lowers_a_boolean_runtime_parameter_with_its_selected_abi_location() {
    let mut plan = parameter_return_plan(1);
    let function = &mut plan.functions[0];
    function.parameters[0].scalar_type = ScalarType::Boolean;
    scalar_result_mut(function).scalar_type = ScalarType::Boolean;
    let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[0] else {
        unreachable!("fixture ends in return")
    };
    *scalar_type = ScalarType::Boolean;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        lowered.functions[0].operation,
        TargetOperation::ReturnBooleanParameter {
            parameter_index: 0,
            location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ..
        }
    ));
}

#[test]
fn lowers_runtime_boolean_equality_to_a_target_expression() {
    let mut plan = parameter_return_plan(2);
    let function = &mut plan.functions[0];
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Boolean;
    }
    scalar_result_mut(function).scalar_type = ScalarType::Boolean;
    let result = ValueId::new(50).expect("equality result");
    function.operations.insert(
        0,
        AbstractOperation::BooleanEqual {
            psi_operation: OperationId::new(50).expect("equality operation"),
            result,
            left: function.parameters[0].value,
            right: function.parameters[1].value,
        },
    );
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = result;
    *scalar_type = ScalarType::Boolean;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        &lowered.functions[0].operation,
        TargetOperation::ReturnBooleanExpression {
            source_value,
            expression: TargetBooleanExpression::Equal {
                psi_operation,
                left,
                right,
            },
            ..
        } if *source_value == result
            && *psi_operation == OperationId::new(50).expect("equality operation")
            && matches!(
                left.as_ref(),
                TargetBooleanExpression::Parameter { parameter_index: 0, .. }
            )
            && matches!(
                right.as_ref(),
                TargetBooleanExpression::Parameter { parameter_index: 1, .. }
            )
    ));
}

#[test]
fn lowers_runtime_integer_equality_to_a_typed_target_expression() {
    let mut plan = parameter_return_plan(2);
    let function = &mut plan.functions[0];
    let integer_type = match function.parameters[0].scalar_type {
        ScalarType::Integer(integer_type) => integer_type,
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => {
            unreachable!("fixture has integer parameters")
        }
    };
    scalar_result_mut(function).scalar_type = ScalarType::Boolean;
    let result = ValueId::new(51).expect("integer-equality result");
    function.operations.insert(
        0,
        AbstractOperation::IntegerEqual {
            psi_operation: OperationId::new(51).expect("integer-equality operation"),
            result,
            left: function.parameters[0].value,
            right: function.parameters[1].value,
        },
    );
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = result;
    *scalar_type = ScalarType::Boolean;

    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        &lowered.functions[0].operation,
        TargetOperation::ReturnBooleanExpression {
            source_value,
            expression: TargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type,
                left,
                right,
            },
            ..
        } if *source_value == result
            && *psi_operation == OperationId::new(51).expect("integer-equality operation")
            && *scalar_type == integer_type
            && matches!(
                left.as_ref(),
                TargetIntegerExpression::Parameter { parameter_index: 0, .. }
            )
            && matches!(
                right.as_ref(),
                TargetIntegerExpression::Parameter { parameter_index: 1, .. }
            )
    ));
}

#[test]
fn folds_a_compile_known_conditional_to_only_the_selected_arm() {
    let condition_operation = psi_core::OperationId::new(20).expect("condition operation");
    let true_operation = psi_core::OperationId::new(21).expect("true operation");
    let false_operation = psi_core::OperationId::new(22).expect("false operation");
    let true_edge = EdgeId::new(1).expect("true edge");
    let false_edge = EdgeId::new(2).expect("false edge");
    let true_return = EdgeId::new(3).expect("true return");
    let false_return = EdgeId::new(4).expect("false return");

    for (select_true, selected_operation, selected_edges) in [
        (true, true_operation, [true_edge, true_return]),
        (false, false_operation, [false_edge, false_return]),
    ] {
        let plan = constant_conditional_plan(select_true);
        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).expect("lower");
        let function = &lowered.functions[0];
        assert_eq!(
            function.provenance.operations,
            [condition_operation, selected_operation]
        );
        assert_eq!(function.provenance.edges, selected_edges);
        assert!(
            matches!(
                &function.operation,
                TargetOperation::ReturnIntegerExpression {
                    psi_edge,
                    expression:
                        TargetIntegerExpression::WrappingAdd { psi_operation, .. },
                    ..
                } if select_true && *psi_edge == true_return && *psi_operation == true_operation
            ) || matches!(
                &function.operation,
                TargetOperation::ReturnIntegerExpression {
                    psi_edge,
                    expression:
                        TargetIntegerExpression::SaturatingMultiply {
                            psi_operation,
                            ..
                        },
                    ..
                } if !select_true && *psi_edge == false_return && *psi_operation == false_operation
            )
        );
    }
}

pub(super) fn constant_conditional_plan(select_true: bool) -> AbstractOperationPlan {
    let machine = MachineId::new(20).expect("machine");
    let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let argument = ValueId::new(1).expect("argument");
    let condition = ValueId::new(2).expect("condition");
    let true_parameter = ValueId::new(3).expect("true parameter");
    let false_parameter = ValueId::new(4).expect("false parameter");
    let true_value = ValueId::new(5).expect("true value");
    let false_value = ValueId::new(6).expect("false value");
    let result = ValueId::new(7).expect("result");
    let true_edge = EdgeId::new(1).expect("true edge");
    let false_edge = EdgeId::new(2).expect("false edge");
    let true_return = EdgeId::new(3).expect("true return");
    let false_return = EdgeId::new(4).expect("false return");
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).expect("entry block"),
            parameters: vec![AbstractParameter {
                value: argument,
                scalar_type,
            }],
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                omega_abstract_operations::AbstractBlockEntry {
                    block: BlockId::new(1).expect("entry block"),
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                omega_abstract_operations::AbstractBlockEntry {
                    block: BlockId::new(2).expect("true block"),
                    parameters: Vec::new(),
                    operation_offset: 2,
                },
                omega_abstract_operations::AbstractBlockEntry {
                    block: BlockId::new(3).expect("false block"),
                    parameters: Vec::new(),
                    operation_offset: 4,
                },
            ],
            operations: vec![
                AbstractOperation::BooleanConstant {
                    psi_operation: psi_core::OperationId::new(20).expect("condition operation"),
                    result: condition,
                    value: select_true,
                },
                AbstractOperation::Conditional {
                    condition,
                    when_true: AbstractSuccessor {
                        psi_edge: true_edge,
                        target: BlockId::new(2).expect("true block"),
                        bindings: vec![ValueBinding {
                            parameter: true_parameter,
                            argument,
                            scalar_type,
                        }],
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: AbstractSuccessor {
                        psi_edge: false_edge,
                        target: BlockId::new(3).expect("false block"),
                        bindings: vec![ValueBinding {
                            parameter: false_parameter,
                            argument,
                            scalar_type,
                        }],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                AbstractOperation::WrappingIntegerAdd {
                    psi_operation: psi_core::OperationId::new(21).expect("true operation"),
                    result: true_value,
                    scalar_type: integer,
                    left: true_parameter,
                    right: true_parameter,
                },
                AbstractOperation::Return {
                    psi_edge: true_return,
                    result,
                    value: true_value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
                AbstractOperation::SaturatingIntegerMultiply {
                    psi_operation: psi_core::OperationId::new(22).expect("false operation"),
                    result: false_value,
                    scalar_type: integer,
                    left: false_parameter,
                    right: false_parameter,
                },
                AbstractOperation::Return {
                    psi_edge: false_return,
                    result,
                    value: false_value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn parameter_return_plan(parameter_count: usize) -> AbstractOperationPlan {
    let machine = MachineId::new(10).expect("machine");
    let result = ValueId::new(100).expect("result");
    let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let parameters = (0..parameter_count)
        .map(|index| AbstractParameter {
            value: ValueId::new(10 + index as u64).expect("parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let returned = parameters.last().expect("fixture has parameters").value;
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(10).expect("block"),
            parameters,
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: vec![AbstractOperation::Return {
                psi_edge: EdgeId::new(10).expect("edge"),
                result,
                value: returned,
                scalar_type,
                cleanup_actions: Vec::new(),
            }],
        }],
    }
}

fn direct_call_plan(parameter_count: usize) -> AbstractOperationPlan {
    let caller = MachineId::new(1).expect("caller");
    let callee = MachineId::new(2).expect("callee");
    let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let caller_parameters = (0..parameter_count)
        .map(|index| AbstractParameter {
            value: ValueId::new(10 + index as u64).expect("caller parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let callee_parameters = (0..parameter_count)
        .map(|index| AbstractParameter {
            value: ValueId::new(30 + index as u64).expect("callee parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let caller_result = ValueId::new(100).expect("caller result");
    let callee_result = ValueId::new(101).expect("callee result");
    AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(1).expect("caller block"),
                parameters: caller_parameters.clone(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: caller_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(1).expect("call"),
                        result: caller_result,
                        scalar_type,
                        callee,
                        arguments: caller_parameters
                            .iter()
                            .map(|parameter| parameter.value)
                            .collect(),
                        requirement_obligations: vec![ObligationId::new(700).unwrap()],
                        crash_continuations: vec![CrashRouteBucket {
                            cause: CrashCause::Trap,
                            alternatives: vec![CrashRouteGuard::Truth],
                        }],
                    },
                    AbstractOperation::Return {
                        psi_edge: EdgeId::new(1).expect("caller return"),
                        result: caller_result,
                        value: caller_result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: BlockId::new(2).expect("callee block"),
                parameters: callee_parameters.clone(),
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
                    value: callee_parameters.last().expect("parameter").value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}
