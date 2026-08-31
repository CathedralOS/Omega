//! Fixed-view-copy recovery through fragments, object artifact, and callable custody.

use crate::tests::*;

#[test]
fn fixed_view_copy_recovery_reaches_fragments_object_and_callable_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_forwarded_parameter_artifact();
        let selections = OptimizationSelections::new([
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery { realization } = physical
        else {
            panic!("the fixed-view rule must complete the shared recovery realization")
        };
        let StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) =
            realization.source()
        else {
            panic!("the generic recovery carrier must retain fixed-view source custody")
        };
        let transformation = homes.reanalysis_stage().transformation_stage();
        assert_eq!(
            homes
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [PostAllocationSelectedTransformation::FixedViewCopy(
                transformation.copies().receipt().identity(),
            )]
        );
        assert_eq!(
            realization
                .manifest()
                .record()
                .allocation_recovery_selections,
            selections.identity()
        );
        let fragments = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::AllocationRecovery(realization),
        )
        .unwrap();
        assert_eq!(
            fragments.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
        );
        let copies: Vec<_> = fragments
            .fragments()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|row| {
                row.alternative.family
                    == omega_selected_instructions::MachineAlternativeFamily::CopyI64
            })
            .collect();
        assert_eq!(copies.len(), 1);
        assert!(!copies[0].bytes.is_empty());
        let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
        let object = stage_optimized_relocation_free_object_container(text).unwrap();
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap();
        assert_eq!(artifact.artifact().selections, selections.identity());
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact)
            .expect("fixed-view recovery preserves ordinary callable custody");
        assert_eq!(callable.entry().selections, selections.identity());
        validate_optimized_ordinary_callable_entry(&callable).unwrap();
    }
}
