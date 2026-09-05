//! AArch64 CBNZ zero-span custody and private entry-symbol construction.

use crate::tests::*;

#[test]
fn relocation_free_cbnz_object_container_retains_zero_span_source_and_private_entry() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
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
        .unwrap_or_else(|| panic!("CBNZ must complete its direct function-relative realization"));
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::from(realization),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let staged = stage_optimized_relocation_free_object_container(placed).unwrap();
    assert_eq!(staged.object().text_section.alignment, 4);
    assert_eq!(
        staged.object().text_section.bytes,
        staged.source().text_section().bytes
    );
    assert!(
        staged.source().text_section().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| instruction.byte_count == 0)
    );
    assert_eq!(
        staged.object().symbols[0].role,
        omega_object_file::RelocationFreeObjectSymbolRole::SemanticEntryV1
    );
    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged).unwrap(),
        staged.custody()
    );
}
