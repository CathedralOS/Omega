//! Correspondence tests only: these source rows do not manufacture arithmetic or cycle evidence.
use super::*;
use abstract_operations::{AbstractBlockEntry, AbstractParameter, AbstractResult, ValueBinding};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId, OperationId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}
fn block(ordinal: u64) -> BlockId {
    BlockId::new(ordinal).unwrap()
}
fn edge(ordinal: u64) -> EdgeId {
    EdgeId::new(ordinal).unwrap()
}
fn operation(ordinal: u64) -> OperationId {
    OperationId::new(ordinal).unwrap()
}
fn parameter(ordinal: u64) -> AbstractParameter {
    AbstractParameter {
        value: value(ordinal),
        scalar_type: ScalarType::Integer(u64_type()),
    }
}
fn successor(ordinal: u64, destination: u64) -> abstract_operations::AbstractSuccessor {
    abstract_operations::AbstractSuccessor {
        psi_edge: edge(ordinal),
        target: block(destination),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}
fn jump(ordinal: u64, argument: u64) -> AbstractOperation {
    AbstractOperation::Jump {
        psi_edge: edge(ordinal),
        target: block(2),
        bindings: vec![ValueBinding {
            parameter: value(2),
            argument: value(argument),
            scalar_type: ScalarType::Integer(u64_type()),
        }],
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn fixture(
    native: ::target::NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let scalar_type = ScalarType::Integer(u64_type());
    let caller = AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: block(1),
        parameters: vec![parameter(1)],
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: value(9),
            scalar_type,
        }),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: [(1, 0), (2, 1), (3, 5), (4, 9)]
            .map(|(ordinal, operation_offset)| AbstractBlockEntry {
                block: block(ordinal),
                operation_offset,
                parameters: if ordinal == 2 {
                    vec![parameter(2)]
                } else {
                    Vec::new()
                },
                structural_parameters: Vec::new(),
            })
            .to_vec(),
        operations: vec![
            jump(1, 1),
            AbstractOperation::IntegerConstant {
                psi_operation: operation(1),
                result: value(3),
                scalar_type,
                value: IntegerValue::Unsigned(0),
            },
            AbstractOperation::IntegerConstant {
                psi_operation: operation(2),
                result: value(4),
                scalar_type,
                value: IntegerValue::Unsigned(1),
            },
            AbstractOperation::IntegerLessThan {
                psi_operation: operation(3),
                result: value(5),
                left: value(3),
                right: value(2),
            },
            AbstractOperation::Conditional {
                condition: value(5),
                when_true: successor(2, 3),
                when_false: successor(3, 4),
            },
            AbstractOperation::ExactIntegerSubtract {
                psi_operation: operation(4),
                result: value(6),
                scalar_type: u64_type(),
                left: value(2),
                right: value(4),
                obligation: ObligationId::new(1).unwrap(),
            },
            AbstractOperation::Call {
                psi_operation: operation(5),
                result: value(7),
                scalar_type,
                callee: MachineId::new(2).unwrap(),
                arguments: vec![value(6)],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            AbstractOperation::ExactIntegerAdd {
                psi_operation: operation(6),
                result: value(8),
                scalar_type: u64_type(),
                left: value(7),
                right: value(3),
                obligation: ObligationId::new(2).unwrap(),
            },
            jump(4, 8),
            AbstractOperation::Return {
                psi_edge: edge(5),
                result: value(9),
                value: value(2),
                scalar_type,
                cleanup_actions: Vec::new(),
            },
        ],
    };
    let callee = AbstractFunction {
        machine: MachineId::new(2).unwrap(),
        attachment: None,
        entry: block(20),
        parameters: vec![parameter(20)],
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: value(21),
            scalar_type,
        }),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block: block(20),
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        }],
        operations: vec![AbstractOperation::Return {
            psi_edge: edge(20),
            result: value(21),
            value: value(20),
            scalar_type,
            cleanup_actions: Vec::new(),
        }],
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([93; 32]),
        },
        entry: caller.machine,
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![caller, callee],
    };
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (plan, target, unit)
}

fn check(
    plan: &AbstractOperationPlan,
    target: &TargetOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    super::super::validate_target(
        &target.functions[0],
        &plan.functions[0],
        &unit.functions[0],
        target,
        plan,
        unit,
    )
}

mod unobserved_owned;

#[test]
fn scalar_cycle_returns_replay_arithmetic_calls_and_result_abi() {
    for native in [
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::linux_arm64(),
        ::target::NativeTarget::windows_x64(),
        ::target::NativeTarget::macos_arm64(),
    ] {
        let (plan, target, unit) = fixture(native);
        check(&plan, &target, &unit).unwrap();
        let graph = &target.functions[0].graph;
        assert!(matches!(
            graph.blocks[2].operations.as_slice(),
            [
                TargetUnitOperation::ScalarDefinition { .. },
                TargetUnitOperation::ScalarCall { .. },
                TargetUnitOperation::ScalarDefinition { .. }
            ]
        ));
        assert!(
            matches!(&graph.blocks[3].terminator, TargetControlTerminator::ReturnScalar { source_value, .. } if *source_value == value(2))
        );
    }
}

#[test]
fn scalar_cycle_rejects_changed_control_returns_and_abi() {
    let (plan, target, unit) = fixture(::target::NativeTarget::macos_arm64());
    for mutation in [
        "target",
        "binding",
        "binding type",
        "return edge",
        "return operand",
        "expression operand",
        "return type",
        "cleanup",
        "graph result ABI",
        "graph parameter ABI",
        "result ABI",
        "result identity",
        "result type",
        "result placement",
    ] {
        let mut changed = target.clone();
        let function = &mut changed.functions[0];
        let graph = &mut function.graph;
        match mutation {
            "target" | "binding" | "binding type" => {
                let TargetControlTerminator::Jump { successor } = &mut graph.blocks[2].terminator
                else {
                    panic!("backedge");
                };
                match mutation {
                    "target" => successor.target = block(4),
                    "binding" => successor.bindings[0].argument = value(6),
                    _ => successor.bindings[0].scalar_type = ScalarType::Boolean,
                }
            }
            "graph result ABI" => graph.call_plan.result = None,
            "graph parameter ABI" => {
                graph.scalar_parameters[0].placement.shape = ValueShape::integer(4, 4)
            }
            "result ABI" => function.scalar_abi.as_mut().unwrap().call_plan.result = None,
            "result identity" => function.scalar_abi.as_mut().unwrap().result.value = value(2),
            "result type" => {
                function.scalar_abi.as_mut().unwrap().result.scalar_type =
                    ScalarType::Integer(u32_type())
            }
            "result placement" => {
                function.scalar_abi.as_mut().unwrap().result.placement.shape =
                    ValueShape::integer(4, 4)
            }
            _ => {
                let TargetControlTerminator::ReturnScalar {
                    psi_edge,
                    source_value,
                    expression,
                    cleanup_actions,
                } = &mut graph.blocks[3].terminator
                else {
                    panic!("return");
                };
                match mutation {
                    "return edge" => *psi_edge = edge(4),
                    "return operand" => *source_value = value(1),
                    "cleanup" => cleanup_actions.push(
                        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                            semantic_vocabulary::PlaceId::new(1).unwrap(),
                        ),
                    ),
                    _ => {
                        let TargetScalarExpression::Integer {
                            scalar_type,
                            expression: Expression::BlockParameter(parameter),
                        } = expression
                        else {
                            panic!("block return");
                        };
                        if mutation == "return type" {
                            *scalar_type = u32_type();
                        } else {
                            parameter.value = value(1);
                        }
                    }
                }
            }
        }
        assert!(
            check(&plan, &changed, &unit).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn scalar_return_rejects_a_coherently_substituted_nondominating_home() {
    let (mut plan, mut target, mut unit) = fixture(::target::NativeTarget::macos_arm64());
    let graph = &mut target.functions[0].graph;
    let TargetUnitOperation::ScalarDefinition { result_home, .. } = &graph.blocks[2].operations[2]
    else {
        panic!("body result");
    };
    let result_home = *result_home;
    let TargetControlTerminator::ReturnScalar {
        source_value,
        expression,
        ..
    } = &mut graph.blocks[3].terminator
    else {
        panic!("return");
    };
    *source_value = result_home.source_value;
    *expression = TargetScalarExpression::Integer {
        scalar_type: u64_type(),
        expression: Expression::ScalarHome(result_home),
    };
    // Even coherent source/target spelling cannot make a body-only result live
    // on the zero-iteration exit. Whole-unit validation is a separate obligation.
    for source in [
        &mut plan.functions[0].operations[9],
        &mut unit.functions[0].blocks[3].nodes[0].operation,
    ] {
        let AbstractOperation::Return { value, .. } = source else {
            panic!("source return");
        };
        *value = result_home.source_value;
    }
    assert!(check(&plan, &target, &unit).is_err());
}

#[test]
fn scalar_return_rejects_changed_source_result_identity_and_unit_exit() {
    for unit_exit in [false, true] {
        let (mut plan, mut target, mut unit) = fixture(::target::NativeTarget::macos_arm64());
        for source in [
            &mut plan.functions[0].operations[9],
            &mut unit.functions[0].blocks[3].nodes[0].operation,
        ] {
            if unit_exit {
                *source = AbstractOperation::ReturnUnit {
                    psi_edge: edge(5),
                    cleanup_actions: Vec::new(),
                };
            } else {
                let AbstractOperation::Return { result, .. } = source else {
                    panic!("source return");
                };
                *result = value(2);
            }
        }
        if unit_exit {
            let graph = &mut target.functions[0].graph;
            graph.blocks[3].terminator = TargetControlTerminator::Return {
                psi_edge: edge(5),
                cleanup_actions: Vec::new(),
            };
        }
        assert!(check(&plan, &target, &unit).is_err());
    }
}

#[test]
fn scalar_cycle_producer_rejects_mismatched_return_result_and_unit_exit() {
    let native = ::target::NativeTarget::macos_arm64();
    let (plan, target, _) = fixture(native);
    assert!(!target.functions[0].graph.blocks.is_empty());
    for mutation in ["result identity", "result type", "Unit return"] {
        let mut changed = plan.clone();
        let returned = &mut changed.functions[0].operations[9];
        if mutation == "Unit return" {
            *returned = AbstractOperation::ReturnUnit {
                psi_edge: edge(5),
                cleanup_actions: Vec::new(),
            };
        } else {
            let AbstractOperation::Return {
                result,
                scalar_type,
                ..
            } = returned
            else {
                panic!("source return");
            };
            if mutation == "result identity" {
                *result = value(2);
            } else {
                *scalar_type = ScalarType::Integer(u32_type());
            }
        }
        let lowered =
            abstract_operations_to_target_operations::lower_to_target_operations(&changed, native);
        assert!(
            lowered.is_err(),
            "producer accepted {mutation}: {lowered:?}"
        );
    }
}

#[test]
fn scalar_cycle_rejects_changed_arithmetic_and_call_rows() {
    let (plan, target, unit) = fixture(::target::NativeTarget::macos_arm64());
    for mutation in [
        "arithmetic obligation",
        "arithmetic operand",
        "arithmetic type",
        "arithmetic home",
        "call argument",
        "call result",
        "call ABI",
    ] {
        let mut changed = target.clone();
        let graph = &mut changed.functions[0].graph;
        if mutation.starts_with("arithmetic") {
            let TargetUnitOperation::ScalarDefinition {
                result_home,
                expression:
                    TargetScalarExpression::Integer {
                        scalar_type,
                        expression:
                            Expression::ExactSubtract {
                                obligation, left, ..
                            },
                    },
            } = &mut graph.blocks[2].operations[0]
            else {
                panic!("subtract");
            };
            match mutation {
                "arithmetic obligation" => *obligation = ObligationId::new(99).unwrap(),
                "arithmetic operand" => {
                    **left = Expression::Immediate {
                        source_value: value(3),
                        value: IntegerValue::Unsigned(0),
                    }
                }
                "arithmetic type" => *scalar_type = u32_type(),
                _ => result_home.source_value = value(8),
            }
        } else {
            let TargetUnitOperation::ScalarCall {
                arguments,
                result_home,
                call_plan,
                ..
            } = &mut graph.blocks[2].operations[1]
            else {
                panic!("call");
            };
            match mutation {
                "call argument" => {
                    arguments[0].source =
                        target_operations::TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: 0,
                            source_value: value(1),
                            scalar_type: ScalarType::Integer(u64_type()),
                        }
                }
                "call result" => result_home.source_value = value(8),
                _ => call_plan.result = None,
            }
        }
        assert!(
            check(&plan, &changed, &unit).is_err(),
            "accepted {mutation}"
        );
    }
}
