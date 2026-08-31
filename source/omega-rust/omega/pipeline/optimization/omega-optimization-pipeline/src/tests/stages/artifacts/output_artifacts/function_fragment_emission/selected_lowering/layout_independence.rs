//! Fragment emission when no function-relative layout rule is selected.

use crate::tests::*;

#[test]
fn selected_lowering_fragment_emission_does_not_require_a_layout_rule() {
    for (subtract, optimization) in [
        (false, Optimization::SelectedIncomingU12ExactAddImmediate),
        (
            true,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ),
    ] {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (semantic, proof) = conditional_exact_binary_artifact(subtract);
            let selections = OptimizationSelections::new([optimization]).unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
            )
            .unwrap();
            let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                target,
                &[],
            )
            .unwrap();
            let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } =
                physical
            else {
                panic!("selected lowering must retain its completed realization")
            };
            assert!(realization.relaxation().is_none());
            let fragments = stage_optimized_function_fragment_emission(
                StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(Box::new(
                    realization,
                )),
            )
            .unwrap();
            assert_eq!(
                fragments.manifest().record().source_kind,
                FunctionFragmentEmissionSourceKind::SelectedLoweringV1
            );
            assert_eq!(
                FunctionFragmentEmissionManifest::decode(&fragments.manifest().record().encode()),
                Ok(fragments.manifest().record().clone())
            );
            assert_eq!(
                validate_optimized_function_fragment_emission(&fragments).unwrap(),
                fragments.custody()
            );
            let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
            assert_eq!(
                text.manifest().record().source_kind,
                FunctionFragmentEmissionSourceKind::SelectedLoweringV1
            );
            assert_eq!(
                FunctionFragmentTextSectionManifest::decode(&text.manifest().record().encode()),
                Ok(text.manifest().record().clone())
            );
            let object = stage_optimized_relocation_free_object_container(text).unwrap();
            assert_eq!(
                validate_optimized_relocation_free_object_container(&object).unwrap(),
                object.custody()
            );
            let artifact = stage_validated_optimized_object_artifact(
                canonical_artifact(&semantic, &proof),
                object,
            )
            .unwrap();
            assert_eq!(
                validate_optimized_object_artifact(&artifact).unwrap(),
                artifact.custody()
            );
            let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
            assert_eq!(
                validate_optimized_ordinary_callable_entry(&callable).unwrap(),
                callable.custody()
            );
        }
    }
}
