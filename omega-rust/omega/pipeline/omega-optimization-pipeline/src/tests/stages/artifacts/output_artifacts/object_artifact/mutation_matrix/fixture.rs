//! Public validated object-artifact fixture shared by mutation families.

use crate::tests::*;

pub(super) fn staged_object_artifact() -> StagedValidatedOptimizedObjectArtifact {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let terminal = canonical_artifact(&semantic, &proof);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| panic!("CBNZ must complete its direct post-allocation realization"));
    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    stage_validated_optimized_object_artifact(terminal, object).unwrap()
}
