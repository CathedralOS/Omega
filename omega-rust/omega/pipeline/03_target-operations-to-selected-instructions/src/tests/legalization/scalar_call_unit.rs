//! Attached-Unit scalar-call production and independent corruption replay.

use crate::tests::fixtures::scalar_call_unit::scalar_call_unit_fixture;
use crate::{legalize_target_operations, validate_legalized_operations};

#[test]
fn bitwise_and_graph_preserves_operand_types_and_independent_replay() {
    bitwise_graph_preserves_operand_types_and_independent_replay(false);
}

fn bitwise_graph_preserves_operand_types_and_independent_replay(exclusive: bool) {
    use abstract_operations::{AbstractOperation, AbstractParameter};
    use legalized_operations::LegalizedScalarInstructionKind;
    use semantic_vocabulary::{
        FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, ScalarType, ValueId,
    };
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for bits in [8, 16, 32, 64] {
                let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
                let integer = IntegerType::new(sign, bits).unwrap();
                let left = ValueId::new(1).unwrap();
                let right = ValueId::new(2).unwrap();
                let result = ValueId::new(3).unwrap();
                source.functions[0].parameters = vec![
                    AbstractParameter {
                        value: left,
                        scalar_type: ScalarType::Integer(integer),
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ];
                source.functions[0].operations.insert(
                    0,
                    if exclusive {
                        AbstractOperation::IntegerBitwiseXor {
                            psi_operation: OperationId::new(1).unwrap(),
                            result,
                            scalar_type: integer,
                            left,
                            right,
                        }
                    } else {
                        AbstractOperation::IntegerBitwiseAnd {
                            psi_operation: OperationId::new(1).unwrap(),
                            result,
                            scalar_type: integer,
                            left,
                            right,
                        }
                    },
                );
                let targeted =
                    abstract_operations_to_target_operations::lower_to_target_operations(
                        &source,
                        abstract_operations_to_target_operations::TargetLoweringRequest::new(
                            native,
                        ),
                    )
                    .unwrap();
                let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                    &source,
                    FuelScheduleIdentity::new(1).unwrap(),
                )
                .unwrap();
                let legal = legalize_target_operations(&targeted, &source, &unit).unwrap();
                validate_legalized_operations(&targeted, &source, &unit, legal.plan().clone())
                    .unwrap();
                for mutation in ["left", "right", "result type", "operator"] {
                    let mut changed = legal.plan().clone();
                    let instruction = &mut changed.scalar_functions[0].blocks[0].instructions[0];
                    let (LegalizedScalarInstructionKind::BitwiseAnd {
                        left: actual_left,
                        right: actual_right,
                    }
                    | LegalizedScalarInstructionKind::BitwiseXor {
                        left: actual_left,
                        right: actual_right,
                    }) = &mut instruction.kind
                    else {
                        panic!("bitwise instruction");
                    };
                    match mutation {
                        "left" => *actual_left = right,
                        "right" => *actual_right = left,
                        "result type" => {
                            instruction.result.as_mut().unwrap().scalar_type = ScalarType::Boolean
                        }
                        "operator" => {
                            instruction.kind = if exclusive {
                                LegalizedScalarInstructionKind::BitwiseAnd { left, right }
                            } else {
                                LegalizedScalarInstructionKind::BitwiseXor { left, right }
                            };
                        }
                        _ => unreachable!(),
                    }
                    assert_ne!(
                        legalized_operations::legalized_operation_plan_identity(&changed),
                        legalized_operations::legalized_operation_plan_identity(legal.plan())
                    );
                    assert!(
                        validate_legalized_operations(&targeted, &source, &unit, changed).is_err(),
                        "accepted {mutation}"
                    );
                }
                let mut changed_target = targeted.clone();
                let target_operations::TargetUnitOperation::ScalarDefinition {
                    expression:
                        target_operations::TargetScalarExpression::Integer {
                            expression:
                                target_operations::TargetIntegerExpression::BitwiseAnd {
                                    left,
                                    right,
                                    ..
                                }
                                | target_operations::TargetIntegerExpression::BitwiseXor {
                                    left,
                                    right,
                                    ..
                                },
                            ..
                        },
                    ..
                } = &mut changed_target.functions[0].graph.blocks[0].operations[0]
                else {
                    panic!("target bitwise instruction");
                };
                std::mem::swap(left, right);
                assert!(legalize_target_operations(&changed_target, &source, &unit).is_err());
                let environment =
                    register_environment::baseline_target_register_environment(native).unwrap();
                let constraints = crate::selection_constraints(&legal, &environment);
                let selected = crate::select_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                crate::validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    selected.plan().clone(),
                )
                .unwrap();
                let mut corrupted = selected.plan().clone();
                let selected_bitwise = corrupted.functions[0].blocks[0]
                    .instructions
                    .iter_mut()
                    .find(|instruction| {
                        instruction.kind
                            == if exclusive {
                                selected_instructions::SelectedInstructionKind::BitwiseXorI64
                            } else {
                                selected_instructions::SelectedInstructionKind::BitwiseAndI64
                            }
                    })
                    .expect("selected bitwise instruction");
                selected_bitwise.operands[1].virtual_register =
                    selected_bitwise.operands[0].virtual_register;
                assert!(
                    crate::validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        corrupted
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn direct_call_result_home_is_reconstructed_before_receiving() {
    let (source, target, unit) = scalar_call_unit_fixture();
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    for mutation in ["missing", "operation", "value", "type", "shape"] {
        let mut changed = target.clone();
        let target_operations::TargetUnitOperation::Call { result, .. } =
            &mut changed.functions[0].graph.blocks[0].operations[2]
        else {
            panic!("source fixture has a direct scalar call");
        };
        if mutation == "missing" {
            *result = target_operations::TargetCallResult::Unit;
        } else {
            let target_operations::TargetCallResult::Scalar(home) = result else {
                panic!("source fixture has a scalar result");
            };
            match mutation {
                "operation" => {
                    home.defining_operation = semantic_vocabulary::OperationId::new(99).unwrap()
                }
                "value" => home.source_value = semantic_vocabulary::ValueId::new(99).unwrap(),
                "type" => home.scalar_type = semantic_vocabulary::ScalarType::Boolean,
                "shape" => home.shape = calling_conventions::ValueShape::integer(1, 1),
                _ => unreachable!(),
            }
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "{mutation}"
        );
        assert!(
            validate_legalized_operations(&changed, &source, &unit, legal.plan().clone()).is_err(),
            "receiving {mutation}"
        );
    }
}

#[test]
fn exact_u64_equality_three_call_chain_is_produced_and_replayed() {
    let (abstract_plan, target, unit) = scalar_call_unit_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("exact attached-Unit scalar call chain legalizes");
    assert_eq!(legalized.plan().scalar_functions.len(), 2);
    assert_eq!(legalized.receipt().function_count(), 2);
    let callee = &legalized.plan().scalar_functions[1];
    assert_eq!(callee.machine, abstract_plan.functions[1].machine);
    assert_eq!(callee.provenance, target.functions[1].provenance);
    assert_eq!(callee.blocks.len(), 3);
    assert!(matches!(
        callee.blocks[0].instructions[0].kind,
        legalized_operations::LegalizedScalarInstructionKind::Compare {
            predicate: legalized_operations::LegalizedScalarComparison::Equal,
            ..
        }
    ));
    validate_legalized_operations(&target, &abstract_plan, &unit, legalized.plan().clone())
        .expect("caller and equality callee replay through the same graph");
    let function = &legalized.plan().scalar_functions[0];
    let calls = function.blocks[0]
        .instructions
        .iter()
        .filter_map(|operation| match &operation.kind {
            legalized_operations::LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(function.blocks[0].instructions.len(), 5);
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].arguments, calls[1].arguments);
    assert_eq!(
        calls[2].arguments[0].scalar_source(),
        function.blocks[0].instructions[2]
            .result
            .as_ref()
            .map(|result| result.value)
    );
    assert_eq!(
        calls[2].arguments[1].scalar_source(),
        function.blocks[0].instructions[3]
            .result
            .as_ref()
            .map(|result| result.value)
    );

    let mut corruptions = Vec::new();
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0]
        .instructions
        .swap(0, 1);
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    call_mut(&mut corrupted.scalar_functions[0], 0)
        .arguments
        .swap(0, 1);
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0].instructions[4].result =
        corrupted.scalar_functions[0].blocks[0].instructions[3].result;
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0].instructions[3].fuel[0].units += 1;
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions.clear();
    corruptions.push(corrupted);
    for corrupted in corruptions {
        assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted).is_err());
    }
}

fn call_mut(
    function: &mut legalized_operations::LegalizedScalarFunction,
    index: usize,
) -> &mut legalized_operations::LegalizedScalarCall {
    function.blocks[0]
        .instructions
        .iter_mut()
        .filter_map(|operation| match &mut operation.kind {
            legalized_operations::LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .nth(index)
        .expect("call fixture")
}
