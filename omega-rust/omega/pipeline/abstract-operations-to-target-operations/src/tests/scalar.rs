use super::*;

mod fixtures;
mod shared_values;
mod straight_line_blocks;
use fixtures::{direct_call_plan, parameter_return_plan};

#[test]
fn scalar_graph_retains_incoming_register_and_stack_abi() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
    ] {
        for count in [1, 9] {
            let source = parameter_return_plan(count);
            let lowered = lower_to_target_operations(&source, target).unwrap();
            let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
                panic!("scalar parameters use the ordinary graph");
            };
            assert_eq!(graph.scalar_parameters.len(), count);
            assert_eq!(
                graph.scalar_parameters[count - 1].placement,
                graph.call_plan.parameters[count - 1]
            );
            let target_operations::TargetControlTerminator::ReturnScalar {
                source_value,
                expression,
                ..
            } = &graph.blocks[0].terminator
            else {
                panic!("scalar return");
            };
            assert_eq!(
                *source_value,
                source.functions[0].parameters[count - 1].value
            );
            let target_operations::TargetScalarExpression::Integer {
                expression:
                    TargetIntegerExpression::Parameter {
                        parameter_index,
                        location,
                        ..
                    },
                ..
            } = expression
            else {
                panic!("returned ABI parameter");
            };
            assert_eq!(*parameter_index, count - 1);
            assert_eq!(
                matches!(location, ScalarParameterLocation::IncomingStack { .. }),
                count == 9
            );
        }
    }
}

#[test]
fn scalar_graph_call_keeps_callee_abi_and_effect_custody() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = direct_call_plan(9);
        let lowered = lower_to_target_operations(&source, target).unwrap();
        let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
            panic!("ordinary caller graph")
        };
        let [
            TargetUnitOperation::ScalarCall {
                arguments,
                call_plan,
                requirement_obligations,
                crash_continuations,
                result_home,
                ..
            },
        ] = graph.blocks[0].operations.as_slice()
        else {
            panic!("one scalar call")
        };
        assert_eq!(arguments.len(), 9);
        for (argument, placement) in arguments.iter().zip(&call_plan.parameters) {
            assert_eq!(&argument.placement, placement);
        }
        assert_eq!(requirement_obligations, &[ObligationId::new(700).unwrap()]);
        assert_eq!(crash_continuations.len(), 1);
        assert!(matches!(&graph.blocks[0].terminator,
            target_operations::TargetControlTerminator::ReturnScalar {
                expression: target_operations::TargetScalarExpression::Integer {
                    expression: TargetIntegerExpression::ScalarHome(home), ..
                }, ..
            } if home == result_home));
    }
}

#[test]
fn scalar_graph_rejects_missing_block_ownership() {
    let mut source = parameter_return_plan(1);
    source.functions[0].block_entries.clear();
    assert!(matches!(
        lower_to_target_operations(&source, NativeTarget::linux_x64()),
        Err(LoweringError::UnsupportedControlFlow(_))
    ));
}

#[test]
fn scalar_graph_rejects_undefined_returns_and_unimplemented_cleanup() {
    let mut source = parameter_return_plan(1);
    let unknown = ValueId::new(999).unwrap();
    let AbstractOperation::Return { value, .. } = &mut source.functions[0].operations[0] else {
        panic!("return")
    };
    *value = unknown;
    assert_eq!(
        lower_to_target_operations(&source, NativeTarget::linux_x64()),
        Err(LoweringError::UnknownValue(unknown))
    );
    let mut source = parameter_return_plan(1);
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut source.functions[0].operations[0]
    else {
        panic!("return")
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
        PlaceId::new(999).unwrap(),
    ));
    assert!(lower_to_target_operations(&source, NativeTarget::linux_x64()).is_err());
}

pub(super) fn constant_conditional_plan(select_true: bool) -> AbstractOperationPlan {
    let machine = MachineId::new(20).expect("machine");
    let integer = IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8).expect("u8");
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
        structural_types: Vec::new().into(),
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
                abstract_operations::AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: BlockId::new(1).expect("entry block"),
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                abstract_operations::AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: BlockId::new(2).expect("true block"),
                    parameters: vec![AbstractParameter {
                        value: true_parameter,
                        scalar_type,
                    }],
                    operation_offset: 2,
                },
                abstract_operations::AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: BlockId::new(3).expect("false block"),
                    parameters: vec![AbstractParameter {
                        value: false_parameter,
                        scalar_type,
                    }],
                    operation_offset: 4,
                },
            ],
            operations: vec![
                AbstractOperation::BooleanConstant {
                    psi_operation: semantic_vocabulary::OperationId::new(20)
                        .expect("condition operation"),
                    result: condition,
                    value: select_true,
                },
                AbstractOperation::Conditional {
                    condition,
                    when_true: AbstractSuccessor {
                        structural_bindings: Vec::new(),
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
                        structural_bindings: Vec::new(),
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
                    psi_operation: semantic_vocabulary::OperationId::new(21)
                        .expect("true operation"),
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
                    psi_operation: semantic_vocabulary::OperationId::new(22)
                        .expect("false operation"),
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
