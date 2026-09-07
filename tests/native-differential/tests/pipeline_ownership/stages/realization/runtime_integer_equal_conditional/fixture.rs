use crate::tests::*;

pub(super) fn staged_object_artifact(
    target: NativeTarget,
) -> StagedValidatedOptimizedObjectArtifact {
    let (semantic, proof) = conditional_u64_integer_equal_parameters_artifact();
    let selections = match target.architecture {
        target::Architecture::X86_64 => OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
        ])
        .unwrap(),
        target::Architecture::Aarch64 => {
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()
        }
    };
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let source = {
        let source = (physical).into_function_fragment_emission_source();
        assert_eq!(source.frame_layout(), source.program().frame.as_ref());
        source
    };
    let fragments = stage_optimized_function_fragment_emission(source).unwrap();
    let object = {
        let applied = stage_function_fragment_frame_application(fragments).unwrap();
        let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
        stage_optimized_relocation_free_object_container(text).unwrap()
    };
    stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
        .unwrap()
}
