//! Uncanned exact scalar DAGs use the ordinary shared physical pipeline.

use crate::tests::*;
use legalized_operations::LegalizedScalarInstructionKind;

#[test]
fn scalar_sequence_replay_rejects_substituted_order_operands_proofs_and_fuel() {
    let (semantic, proof) = sequence_artifact(2);
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target).unwrap();
        let legalized = legalize_target_operations(
            target.target_operations(),
            target.optimized().plan(),
            target.optimized().unit(),
        )
        .unwrap();
        let original = legalized.plan();
        let validate = |plan| {
            validate_legalized_operations(
                target.target_operations(),
                target.optimized().plan(),
                target.optimized().unit(),
                plan,
            )
        };
        validate(original.clone()).unwrap();
        for mutation in 0..6 {
            let mut corrupted = original.clone();
            assert!(corrupted.functions.is_empty());
            let instructions = &mut corrupted.scalar_functions[0].blocks[0].instructions;
            let LegalizedScalarInstructionKind::ExactBinary {
                left: first_left,
                accepted_fact: first_fact,
                ..
            } = instructions[3].kind
            else {
                panic!("the first binary follows three constants")
            };
            match mutation {
                0 => {
                    instructions.remove(3);
                }
                1 => instructions.swap(3, 4),
                4 => instructions[4].fuel[0].units += 1,
                2 | 3 | 5 => {
                    let LegalizedScalarInstructionKind::ExactBinary {
                        left,
                        accepted_fact,
                        operator,
                        ..
                    } = &mut instructions[4].kind
                    else {
                        unreachable!()
                    };
                    match mutation {
                        2 => *left = first_left,
                        3 => *accepted_fact = first_fact,
                        5 => {
                            *operator =
                                legalized_operations::LegalizedExactIntegerOperator::Subtract
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                validate(corrupted).is_err(),
                "mutation {mutation} must reject"
            );
        }
    }
}

#[test]
fn exact_scalar_sequences_reach_shared_native_publication() {
    for (extra_operations, parameter) in [(1, false), (2, false), (7, false), (2, true), (9, true)]
    {
        let (semantic, proof) = if parameter {
            parameter_sequence_artifact(extra_operations)
        } else {
            sequence_artifact(extra_operations)
        };
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            for choices in [Vec::new(), vec![Optimization::CopyPropagation]] {
                let selections = OptimizationSelections::new(choices).unwrap();
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    compiler_baseline_request_v1(&selections),
                )
                .expect("the source DAG and every arithmetic proof are independently verified");
                let post_terminal = optimized.selections().project_post_terminal();
                let target_program =
                    lower_optimized_to_target_operations(optimized, target).unwrap();
                assert!(
                    target_operations_to_selected_instructions::is_fragment_publication_program(
                        &target_program
                    ),
                    "ordinary exact scalar DAGs must not select the Assigned route"
                );
                let physical = stage_optimized_verified_physical_pipeline(
                    target_program,
                    post_terminal.selections(),
                )
                .unwrap_or_else(|error| panic!("shared physical sequence ({extra_operations} extra, parameter={parameter}, {target:?}): {error:?}"));
                let fragments = stage_optimized_function_fragment_emission(
                    physical.into_function_fragment_emission_source(),
                )
                .unwrap();
                let text: StagedOptimizedObjectTextSectionSource =
                    if fragments.source().frame_protocol().is_some() {
                        let applied = stage_function_fragment_frame_application(fragments).unwrap();
                        stage_optimized_fixed_frame_text_section(applied)
                            .unwrap()
                            .into()
                    } else {
                        stage_optimized_relocation_free_text_section(fragments)
                            .unwrap()
                            .into()
                    };
                let source = stage_optimized_relocation_free_object_container(text).unwrap();
                let object = image_emission::build_function_fragment_object_artifact(&source)
                    .expect("shared scalar sequence object");
                image_emission::validate_function_fragment_object_artifact(&source, &object)
                    .unwrap();
                let image = image_emission::emit_executable_image(&object, 3).unwrap();
                image_emission::validate_executable_image(&object, &image).unwrap();
                let installation = image_emission::build_installation_record(
                    &image,
                    semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
                )
                .unwrap();
                let encoded = image_emission::encode_installation_record(&installation).unwrap();
                let decoded = image_emission::decode_installation_record(&encoded).unwrap();
                image_emission::validate_installation_record(&decoded, &image).unwrap();
                assert_eq!(decoded, installation);
            }
        }
    }
}

fn parameter_sequence_artifact(parameter_count: usize) -> (Vec<u8>, Vec<u8>) {
    let (semantic, _) = sequence_artifact(2);
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let machine = &mut module.machines[0];
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let parameter = ValueId::new(9_400).unwrap();
    let zero = ValueId::new(9_401).unwrap();
    let result = ValueId::new(9_402).unwrap();
    machine.parameters.push(ValueDeclaration {
        id: ValueId::new(9_399).unwrap(),
        scalar_type,
    });
    machine.parameters.push(ValueDeclaration {
        id: parameter,
        scalar_type,
    });
    // The full ABI remains declared even when its stack parameters are unused.
    for parameter_index in 2..parameter_count {
        machine.parameters.push(ValueDeclaration {
            id: ValueId::new(9_500 + parameter_index as u64).unwrap(),
            scalar_type,
        });
    }
    let body = &mut machine.blocks[0];
    body.operations = vec![Operation {
        id: OperationId::new(9_410).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: zero,
            scalar_type,
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        },
    }];
    let Terminator::Return { value, .. } = &mut body.terminator else {
        unreachable!()
    };
    body.operations.push(Operation {
        id: OperationId::new(9_411).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        kind: OperationKind::ExactIntegerAdd {
            left: parameter,
            right: zero,
            obligation: ObligationId::new(9_421).unwrap(),
        },
    });
    *value = result;
    let proof = operation_proof_bundle(&module);
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

fn sequence_artifact(extra_operations: usize) -> (Vec<u8>, Vec<u8>) {
    let (semantic, _) = conditional_active_resident_exact_add_chain_artifact();
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let machine = &mut module.machines[0];
    let mut body = machine.blocks.remove(1);
    machine.parameters.clear();
    machine.entry = body.id;
    let mut previous = body.operations.last().unwrap().result.scalar().unwrap().id;
    let operands = body.operations[..3]
        .iter()
        .map(|operation| operation.result.scalar().unwrap().id)
        .collect::<Vec<_>>();
    for index in 0..extra_operations {
        let result = ValueId::new(9_100 + index as u64).unwrap();
        body.operations.push(Operation {
            id: OperationId::new(9_200 + index as u64).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            }),
            kind: if index + 1 != extra_operations {
                OperationKind::ExactIntegerAdd {
                    left: previous,
                    right: operands[index % operands.len()],
                    obligation: ObligationId::new(9_300 + index as u64).unwrap(),
                }
            } else {
                OperationKind::ExactIntegerSubtract {
                    left: previous,
                    right: operands[index % operands.len()],
                    obligation: ObligationId::new(9_300 + index as u64).unwrap(),
                }
            },
        });
        previous = result;
    }
    let Terminator::Return { value, .. } = &mut body.terminator else {
        unreachable!()
    };
    *value = previous;
    machine.blocks = vec![body];
    let proof = operation_proof_bundle(&module);
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}
