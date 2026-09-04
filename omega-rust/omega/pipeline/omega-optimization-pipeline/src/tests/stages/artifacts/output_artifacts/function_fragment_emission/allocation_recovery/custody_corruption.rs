//! Generic recovery carrier rejection for detached sources and corrupt retained custody.

use crate::tests::*;

fn staged_fixed_view_allocation_recovery_realization(
    target: NativeTarget,
) -> StagedAllocationRecoveryFunctionRelativeRealization {
    let (semantic, proof) = conditional_forwarded_parameter_artifact();
    let selections = OptimizationSelections::new([
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    *(stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
        .unwrap())
    .into_allocation_recovery_for_test()
    .expect("fixture must complete allocation recovery")
}

#[test]
fn generic_allocation_recovery_realization_rejects_detached_and_corrupt_custody() {
    let target = NativeTarget::linux_x64();
    let mut source = staged_fixed_view_allocation_recovery_realization(target);
    let mut foreign = staged_active_resident_allocation_recovery_realization(target);
    swap_allocation_recovery_realization_source_for_test(&mut source, &mut foreign);
    assert!(validate_allocation_recovery_function_relative_realization(&source).is_err());

    let mut encoding = staged_fixed_view_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_encoding_for_test(&mut encoding);
    assert!(validate_allocation_recovery_function_relative_realization(&encoding).is_err());

    let mut layout = staged_fixed_view_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_layout_for_test(&mut layout);
    assert!(validate_allocation_recovery_function_relative_realization(&layout).is_err());

    let mut exit = staged_fixed_view_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_exit_for_test(&mut exit);
    assert!(validate_allocation_recovery_function_relative_realization(&exit).is_err());

    let mut manifest = staged_fixed_view_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_manifest_for_test(&mut manifest);
    assert!(validate_allocation_recovery_function_relative_realization(&manifest).is_err());

    let mut custody = staged_fixed_view_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_custody_for_test(&mut custody);
    assert!(validate_allocation_recovery_function_relative_realization(&custody).is_err());
}
