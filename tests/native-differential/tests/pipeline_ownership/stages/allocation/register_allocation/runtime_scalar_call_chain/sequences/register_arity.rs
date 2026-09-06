//! Register call signatures carry their authored arity, not a pair-shaped recipe.

use crate::tests::*;

fn artifact(argument_count: usize) -> (Vec<u8>, Vec<u8>) {
    scalar_call_unit_artifact_with(|module| {
        let callee = &mut module.machines[1];
        let parameter = callee.parameters[0];
        callee.parameters = (0..argument_count)
            .map(|index| ValueDeclaration {
                id: ValueId::new(24_000 + index as u64).unwrap(),
                scalar_type: parameter.scalar_type,
            })
            .collect();
        callee.blocks.truncate(1);
        callee.blocks[0].operations.clear();
        let returned = match callee.parameters.last() {
            Some(parameter) => parameter.id,
            None => {
                let value = ValueId::new(24_100).unwrap();
                callee.blocks[0].operations.push(Operation {
                    id: OperationId::new(24_101).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value,
                        scalar_type: parameter.scalar_type,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(19),
                    },
                });
                value
            }
        };
        callee.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(24_102).unwrap(),
            value: returned,
            cleanup_actions: Vec::new(),
        };
        let operations = &mut module.machines[0].blocks[0].operations;
        operations.truncate(3);
        let OperationKind::Call { arguments, .. } = &mut operations[2].kind else {
            unreachable!()
        };
        *arguments = (0..argument_count)
            .map(|index| {
                ValueId::new(if index % 2 == 0 {
                    SCALAR_CALL_UNIT_LEFT
                } else {
                    SCALAR_CALL_UNIT_RIGHT
                })
                .unwrap()
            })
            .collect();
    })
}

#[test]
fn register_argument_rosters_use_shared_selection() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let maximum = if target == NativeTarget::linux_x64() {
            6
        } else {
            8
        };
        for argument_count in 0..=maximum {
            let (semantic, proof) = artifact(argument_count);
            let selections = OptimizationSelections::new([]).unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                compiler_baseline_request_v1(&selections),
            )
            .unwrap();
            let lowered = lower_optimized_to_target_operations(optimized, target).unwrap();
            assert!(
                target_operations_to_selected_instructions::is_fragment_publication_program(
                    &lowered
                ),
                "register-only arity {argument_count} must use the shared route on {target:?}"
            );
            stage_optimized_instruction_selection(lowered).unwrap();
        }
    }
}

#[test]
fn register_call_arity_reaches_image_and_installation_with_empty_and_selected_phases() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let maximum = if target == NativeTarget::linux_x64() {
            6
        } else {
            8
        };
        for argument_count in [0, 1, 3, maximum] {
            for choices in [Vec::new(), vec![Optimization::CopyPropagation]] {
                let (semantic, proof) = artifact(argument_count);
                let selections = OptimizationSelections::new(choices).unwrap();
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    compiler_baseline_request_v1(&selections),
                )
                .unwrap();
                let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
                    optimized,
                    target,
                    &[],
                )
                .unwrap_or_else(|error| panic!("{target:?}, arity {argument_count}: {error:?}"));
                let emitted = stage_optimized_function_fragment_emission(
                    physical.into_function_fragment_emission_source(),
                )
                .unwrap();
                let framed = stage_function_fragment_frame_application(emitted).unwrap();
                let text = stage_optimized_fixed_frame_text_section(framed).unwrap();
                let source = stage_optimized_relocation_free_object_container(text).unwrap();
                let object =
                    image_emission::build_function_fragment_object_artifact(&source).unwrap();
                image_emission::validate_function_fragment_object_artifact(&source, &object)
                    .unwrap();
                assert_eq!(object.entry_function().unit_call_stacks.len(), 1);
                let demand = image_emission::derive_stack_demand(&object, object.entry()).unwrap();
                let image = image_emission::emit_executable_image(&object, 3).unwrap();
                image_emission::validate_executable_image(&object, &image).unwrap();
                let record = image_emission::build_installation_record(
                    &image,
                    semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
                )
                .unwrap();
                let encoded = image_emission::encode_installation_record(&record).unwrap();
                let decoded = image_emission::decode_installation_record(&encoded).unwrap();
                image_emission::validate_installation_record(&decoded, &image).unwrap();
                assert_eq!(
                    image_emission::derive_installation_stack_demand(
                        &decoded,
                        &image,
                        object.entry()
                    )
                    .unwrap(),
                    demand,
                );
            }
        }
    }
}

#[test]
fn stack_argument_calls_do_not_claim_register_only_publication() {
    for (target, argument_count) in [
        (NativeTarget::linux_x64(), 7),
        (NativeTarget::linux_arm64(), 9),
    ] {
        let (semantic, proof) = artifact(argument_count);
        let selections = OptimizationSelections::new([]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let lowered = lower_optimized_to_target_operations(optimized, target).unwrap();
        assert!(
            !target_operations_to_selected_instructions::is_fragment_publication_program(&lowered)
        );
        assert!(stage_optimized_instruction_selection(lowered).is_err());
    }
}
