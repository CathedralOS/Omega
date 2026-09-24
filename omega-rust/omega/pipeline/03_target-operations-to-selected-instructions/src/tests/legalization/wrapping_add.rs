//! Modular arithmetic retains ordered, width-normalized SSA definitions.
use abstract_operations::{
    AbstractFunctionResult, AbstractOperation, AbstractParameter, AbstractResult,
};
use legalized_operations::LegalizedScalarInstructionKind;
use selected_instructions::SelectedInstructionKind;
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::{TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation};

use crate::{
    legalize_target_operations, select_instructions, validate_legalized_operations,
    validate_selected_instructions,
};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

#[test]
fn wrapping_add_replays_width_policy_snapshot_and_normalized_consumers() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for bits in [8, 16, 32, 64] {
            for native in [
                NativeTarget::linux_x64(),
                NativeTarget::linux_arm64(),
                NativeTarget::macos_arm64(),
                NativeTarget::windows_x64(),
            ] {
                let integer = IntegerType::new(sign, bits).unwrap();
                let scalar_type = ScalarType::Integer(integer);
                let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
                let function = &mut source.functions[0];
                function.parameters = [value(1), value(2)]
                    .map(|value| AbstractParameter { value, scalar_type })
                    .to_vec();
                function.result = AbstractFunctionResult::Scalar(AbstractResult {
                    value: value(5),
                    scalar_type,
                });
                function.operations = vec![
                    AbstractOperation::WrappingIntegerAdd {
                        psi_operation: OperationId::new(1).unwrap(),
                        result: value(3),
                        scalar_type: integer,
                        left: value(1),
                        right: value(2),
                    },
                    // A produced value may be both operands without duplication of its producer.
                    AbstractOperation::WrappingIntegerAdd {
                        psi_operation: OperationId::new(2).unwrap(),
                        result: value(4),
                        scalar_type: integer,
                        left: value(3),
                        right: value(3),
                    },
                    AbstractOperation::Return {
                        psi_edge: EdgeId::new(1).unwrap(),
                        result: value(5),
                        value: value(4),
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ];
                let target = abstract_operations_to_target_operations::lower_to_target_operations(
                    &source,
                    abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
                )
                .unwrap();
                let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                    &source,
                    FuelScheduleIdentity::new(1).unwrap(),
                )
                .unwrap();
                optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
                let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
                validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                    .unwrap();
                let mut changed = legalized.plan().clone();
                let row = &mut changed.scalar_functions[0].blocks[0].instructions[1];
                let LegalizedScalarInstructionKind::WrappingAdd { left, .. } = &mut row.kind else {
                    panic!("second wrapping definition")
                };
                *left = value(1);
                assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());

                for mutation in ["operand", "width", "result", "policy", "reevaluation"] {
                    let mut changed = target.clone();
                    let first_expression = match &changed.functions[0].graph.blocks[0].operations[0]
                    {
                        TargetUnitOperation::ScalarDefinition {
                            expression: TargetScalarExpression::Integer { expression, .. },
                            ..
                        } => expression.clone(),
                        _ => panic!("first wrapping definition"),
                    };
                    let TargetUnitOperation::ScalarDefinition {
                        result_home,
                        expression:
                            TargetScalarExpression::Integer {
                                scalar_type,
                                expression,
                            },
                    } = &mut changed.functions[0].graph.blocks[0].operations[1]
                    else {
                        panic!("second wrapping definition")
                    };
                    let TargetIntegerExpression::WrappingAdd {
                        psi_operation,
                        left,
                        right,
                    } = expression
                    else {
                        panic!("wrapping policy")
                    };
                    match mutation {
                        "operand" => {
                            **left = TargetIntegerExpression::Immediate {
                                source_value: value(99),
                                value: semantic_vocabulary::IntegerValue::Unsigned(0),
                            }
                        }
                        "width" => {
                            *scalar_type =
                                IntegerType::new(sign, if bits == 64 { 32 } else { 64 }).unwrap()
                        }
                        "result" => result_home.source_value = value(3),
                        "reevaluation" => **left = first_expression,
                        _ => {
                            *expression = TargetIntegerExpression::ExactAdd {
                                psi_operation: *psi_operation,
                                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                                left: left.clone(),
                                right: right.clone(),
                            }
                        }
                    }
                    assert!(
                        legalize_target_operations(&changed, &source, &unit).is_err(),
                        "{sign:?}{bits} {native:?} {mutation}"
                    );
                    assert!(
                        validate_legalized_operations(
                            &changed,
                            &source,
                            &unit,
                            legalized.plan().clone()
                        )
                        .is_err(),
                        "{mutation}"
                    );
                }

                let environment =
                    register_environment::baseline_target_register_environment(native).unwrap();
                let constraints = crate::selection_constraints(&legalized, &environment);
                let selected = select_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    selected.plan().clone(),
                )
                .unwrap();
                let instructions = &selected.plan().functions[0].blocks[0].instructions;
                let additions = instructions
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| row.kind == SelectedInstructionKind::WrappingAddI64)
                    .map(|(position, _)| position)
                    .collect::<Vec<_>>();
                assert_eq!(additions.len(), 2);
                let raw = instructions[additions[0]].operands[2].virtual_register;
                for mutation in [
                    "exact policy",
                    "missing normalization",
                    "wrong normalization",
                    "raw consumer",
                    "double fuel",
                ] {
                    if bits == 64 && mutation != "exact policy" {
                        continue;
                    }
                    let mut changed = selected.plan().clone();
                    let instructions = &mut changed.functions[0].blocks[0].instructions;
                    match mutation {
                        "exact policy" => {
                            instructions[additions[0]].kind = SelectedInstructionKind::ExactAddI64 {
                                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                                accepted_fact:
                                    optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                        [42; 32],
                                    ),
                            }
                        }
                        "missing normalization" => {
                            instructions.remove(additions[0] + 1);
                        }
                        "wrong normalization" => {
                            instructions[additions[0] + 1].kind = SelectedInstructionKind::CopyI64
                        }
                        "raw consumer" => {
                            instructions[additions[1]].operands[0].virtual_register = raw
                        }
                        _ => {
                            instructions[additions[0] + 1].provenance =
                                instructions[additions[0]].provenance.clone()
                        }
                    }
                    assert!(
                        validate_selected_instructions(
                            &legalized,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                            changed
                        )
                        .is_err(),
                        "{sign:?}{bits} {native:?} {mutation}"
                    );
                }
            }
        }
    }
}
