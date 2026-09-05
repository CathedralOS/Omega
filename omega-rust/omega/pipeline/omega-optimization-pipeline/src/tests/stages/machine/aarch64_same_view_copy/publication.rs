use crate::FunctionFragmentReplayInputs;
use crate::tests::*;

pub(super) fn assert_no_candidate_reaches_object_and_callable(
    rule: Optimization,
    target: NativeTarget,
) {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections = OptimizationSelections::new([rule]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let realization = (physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| {
            panic!("the exact post-allocation selection must use the generic machine route")
        });
    assert_eq!(realization.optimization().optimization(), rule);
    assert_eq!(realization.optimization().action_count(), 0);

    let fragments = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::PostAllocationMachine(Box::new(realization)).into(),
    )
    .unwrap();
    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    assert_eq!(artifact.artifact().selections, selections.identity());
    let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
    assert_eq!(callable.entry().selections, selections.identity());
    validate_optimized_ordinary_callable_entry(&callable).unwrap();
}
