//! Dispatch must retain the exact owner of an operation result or state parameter.
use super::*;
use legalized_operations::{LegalizedScalarTerminator, LegalizedStructuralCaseSource};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralTypeId};

#[test]
fn case_dispatch_rejects_substituted_result_and_state_parameter_sources() {
    let owned_source = concat!(
        include_str!("choose.omg"),
        "\n",
        include_str!("owned_state.omg")
    );
    for (entry, source, expects_parameter) in [
        ("collect", include_str!("borrowed.omg"), false),
        ("collect_owned", owned_source, true),
    ] {
        let artifact = produce_source(entry, source);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let selections = OptimizationSelections::new([]).unwrap();
            let optimized = optimize_artifact_sections(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &AdmissionProfile::default(),
                compiler_baseline_request_v1(&selections),
            )
            .unwrap();
            let compiled =
                abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                    optimized, target,
                )
                .unwrap();
            let legalized = target_operations_to_selected_instructions::legalize_target_operations(
                compiled.target_operations(),
                compiled.optimized().plan(),
                compiled.optimized(),
            )
            .unwrap();
            let validate = |plan| {
                target_operations_to_selected_instructions::validate_legalized_operations(
                    compiled.target_operations(),
                    compiled.optimized().plan(),
                    compiled.optimized(),
                    plan,
                )
            };
            assert_eq!(validate(legalized.plan().clone()).unwrap(), legalized);
            let mut dispatches = 0;
            for (function_index, function) in legalized.plan().scalar_functions.iter().enumerate() {
                for (block_index, block) in function.blocks.iter().enumerate() {
                    let LegalizedScalarTerminator::StructuralCase { source, .. } =
                        &block.terminator
                    else {
                        continue;
                    };
                    assert_eq!(
                        matches!(source, LegalizedStructuralCaseSource::BlockParameter { .. }),
                        expects_parameter
                    );
                    dispatches += 1;
                    for mutation in 0..3 {
                        let mut changed = legalized.plan().clone();
                        let LegalizedScalarTerminator::StructuralCase { source, .. } =
                            &mut changed.scalar_functions[function_index].blocks[block_index]
                                .terminator
                        else {
                            unreachable!()
                        };
                        match source {
                            LegalizedStructuralCaseSource::OperationResult {
                                operation,
                                result,
                            } => match mutation {
                                0 => *operation = OperationId::new(999_999).unwrap(),
                                1 => result.place = PlaceId::new(999_999).unwrap(),
                                _ => {
                                    result.structural_type = StructuralTypeId::new(999_999).unwrap()
                                }
                            },
                            LegalizedStructuralCaseSource::BlockParameter {
                                block,
                                declaration,
                            } => match mutation {
                                0 => *block = BlockId::new(999_999).unwrap(),
                                1 => declaration.place = PlaceId::new(999_999).unwrap(),
                                _ => {
                                    declaration.structural_type =
                                        StructuralTypeId::new(999_999).unwrap()
                                }
                            },
                        }
                        assert!(
                            validate(changed).is_err(),
                            "{entry} mutation {mutation} on {target:?}"
                        );
                    }
                }
            }
            assert!(
                dispatches > 0,
                "the fixture must exercise its source variant"
            );
        }
    }
}
