//! Ranked proof custody crosses the same selected/frame/publication stages.

use optimization_core::{Optimization, OptimizationSelections};

pub(super) fn selections(_target: target::NativeTarget) -> [OptimizationSelections; 2] {
    [
        OptimizationSelections::default(),
        OptimizationSelections::new([Optimization::CopyPropagation])
            .expect("one target-owned post-Terminal selection"),
    ]
}

fn target_program(
    semantic: &[u8],
    proof: &[u8],
    ranked: &abstract_operations::RankedNativeAbstractOperationPlan,
    target: target::NativeTarget,
    selections: &OptimizationSelections,
) -> abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations {
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        semantic,
        proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independently admit ranked optimization input");
    let optimized = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(selections),
    )
    .expect("execute selected abstract phases with checked ranked custody");
    abstract_operations_to_target_operations::lower_validated_ranked_to_target_operations(
        optimized, ranked, target,
    )
    .expect("bind current ranked target data to its independently checked authority")
}

pub(super) fn publish(
    semantic: &[u8],
    proof: &[u8],
    ranked: &abstract_operations::RankedNativeAbstractOperationPlan,
    target: target::NativeTarget,
    selections: &OptimizationSelections,
) -> (
    std::sync::Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    image_emission::ObjectArtifact,
) {
    let target_program = target_program(semantic, proof, ranked, target, selections);
    let post_terminal = target_program
        .optimized()
        .selections()
        .project_post_terminal();
    let physical = crate::stage_optimized_verified_physical_pipeline(
        target_program,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| {
        panic!("{target:?} {selections:?}: ranked common physical stages: {error:?}")
    });
    let emitted = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("emit ordinary ranked instructions");
    let applied = machine_emission::stage_function_fragment_frame_application(emitted)
        .expect("apply checked ranked frame");
    let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
        .expect("place framed ranked text");
    let source = object_file::stage_optimized_relocation_free_object_container(text)
        .expect("place ranked object container");
    let source = std::sync::Arc::new(source);
    let object = image_emission::build_function_fragment_object_artifact(source.clone())
        .expect("independently publish ranked current graph");
    image_emission::validate_function_fragment_object_artifact(&source, &object)
        .expect("independently replay ranked publication");
    (source, object)
}

#[test]
fn ranked_countdown_empty_and_selected_phases_reach_common_publication() {
    let checked = crate::tests::fixtures::checked_source::checked(
        super::native_dispatch::RANKED_COUNTDOWN_SOURCE,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::countdown")
        .expect("lower checked ranked source");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let admitted =
        terminal_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("ranked native proof admission");
    let terminal_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(
        ranked,
    ) = admitted
    else {
        panic!("ranked source retains its native authority");
    };
    custody_codec::check(&ranked.countdown);
    let mut previous_target_object = None;
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let selected =
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                target_program(
                    &semantic,
                    &proof,
                    &ranked,
                    target,
                    &OptimizationSelections::default(),
                ),
                register_environment::baseline_target_register_environment(target).unwrap(),
            )
            .expect("ranked common selection");
        normalization::reject_substituted_transport(&selected);
        for selection in selections(target) {
            let (source, object) = publish(&semantic, &proof, &ranked, target, &selection);
            if let Some((previous_target, previous)) = &previous_target_object
                && *previous_target != target
            {
                assert!(
                    image_emission::validate_function_fragment_object_artifact(&source, previous,)
                        .is_err(),
                    "another target's otherwise valid ranked artifact is not current evidence"
                );
            }
            let record = object.functions()[0]
                .ranked_u32_countdown
                .as_ref()
                .expect("common publication retains ranked semantic custody");
            assert_eq!(record.custody, ranked.countdown);
            let image = image_emission::emit_executable_image(&object, 0)
                .expect("emit ranked common image");
            image_emission::validate_executable_image(&object, &image).unwrap();
            let installation = image_emission::build_installation_record(
                &image,
                semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
            )
            .expect("install common ranked image");
            let encoded = image_emission::encode_installation_record(&installation).unwrap();
            let decoded = image_emission::decode_installation_record(&encoded).unwrap();
            image_emission::validate_installation_record(&decoded, &image).unwrap();
            previous_target_object = Some((target, object));
        }
    }
}

mod custody_codec;
mod normalization;
