//! Calling bodies publish from current physical data, not invented stack homes.

use super::{Sequence, sequence_artifact};
use crate::tests::*;

fn staged(
    target: NativeTarget,
    sequence: Sequence,
    selections: &OptimizationSelections,
) -> object_file::StagedOptimizedRelocationFreeObjectContainer {
    let (semantic, proof) = sequence_artifact(sequence);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(selections),
    )
    .unwrap();
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let emitted = stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let applied = stage_function_fragment_frame_application(emitted).unwrap();
    let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
    stage_optimized_relocation_free_object_container(text).unwrap()
}

#[test]
fn ordered_scalar_calls_reach_native_publication() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for sequence in [
            Sequence::Single,
            Sequence::EqualConstants,
            Sequence::InterleavedCallees,
        ] {
            for choices in [
                Vec::new(),
                vec![Optimization::CopyPropagation],
                vec![if target.architecture == target::Architecture::X86_64 {
                    Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
                } else {
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                }],
            ] {
                let selections = OptimizationSelections::new(choices).unwrap();
                let source = staged(target, sequence, &selections);
                let object = image_emission::build_function_fragment_object_artifact(&source)
                    .unwrap_or_else(|error| {
                        panic!("{target:?}, {sequence:?}, {selections:?}: {error:?}")
                    });
                image_emission::validate_function_fragment_object_artifact(&source, &object)
                    .unwrap();
                assert_eq!(object.text_bytes(), source.source().text_section().bytes);
                let expected_calls = match sequence {
                    Sequence::Single => 1,
                    Sequence::EqualConstants => 3,
                    Sequence::InterleavedCallees => 4,
                };
                let caller = object.entry_function();
                assert_eq!(caller.unit_call_stacks.len(), expected_calls);
                assert!(caller.unit_scalar_homes.is_empty());
                assert!(caller.internal_unit_scalar_calls.is_empty());
                let demand = image_emission::derive_stack_demand(&object, object.entry()).unwrap();
                let expected = caller
                    .unit_call_stacks
                    .iter()
                    .map(|call| {
                        let callee = object
                            .functions()
                            .iter()
                            .find(|function| function.machine == call.target)
                            .unwrap();
                        u64::from(call.caller_live_bytes)
                            + u64::from(callee.scalar_stack.unwrap().local_peak_bytes)
                    })
                    .fold(
                        u64::from(caller.unit_stack.unwrap().local_peak_bytes),
                        u64::max,
                    );
                assert_eq!(demand.ceiling_bytes(), expected);
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
                    demand
                );
            }
        }
    }
}

#[test]
fn fragment_publication_scope_rejects_a_different_program_or_object() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selections = OptimizationSelections::default();
        let mut source = staged(target, Sequence::InterleavedCallees, &selections);
        let object = image_emission::build_function_fragment_object_artifact(&source).unwrap();
        let different_source = staged(target, Sequence::Single, &selections);
        let different =
            image_emission::build_function_fragment_object_artifact(&different_source).unwrap();
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &different)
                .is_err()
        );
        let current = source
            .source()
            .source()
            .source()
            .optimized_target()
            .optimized();
        let plan = current.plan();
        let validation = current.validation();
        let demands =
            boundary_applications::TerminalBoundaryApplicationDemands::new(plan.psi, Vec::new())
                .unwrap();
        let realizations = boundary_applications::TerminalBoundaryApplicationRealizations::new(
            &demands,
            Vec::new(),
        )
        .unwrap();
        let coverage =
            boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
                .unwrap();
        let scope = |plan, object| {
            native_artifact::NativePhysicalEvidenceScope::from_validated_fragment_publication(
                plan,
                validation.psi(),
                validation.identity(),
                validation.final_unit(),
                &coverage,
                &source,
                object,
            )
        };
        assert!(scope(plan, &object).is_ok());
        assert!(scope(plan, &different).is_err());
        let mut detached = plan.clone();
        detached.functions[0].operations.clear();
        assert!(scope(&detached, &object).is_err());
        source.corrupt_custody_source_text_section_manifest_for_test();
        assert!(image_emission::build_function_fragment_object_artifact(&source).is_err());
    }
}
