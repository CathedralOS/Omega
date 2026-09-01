use super::super::*;

#[test]
fn multi_pass_projection_retains_zero_commit_manifest_in_canonical_order() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let optimized =
        project_optimization_run(run_pipeline(exact_add_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 2);
    assert_eq!(optimized.pass_manifests()[0].work_usage().commits, 1);
    assert_eq!(optimized.pass_manifests()[1].work_usage().commits, 0);
}

#[test]
fn multi_pass_projection_rejects_reordered_or_omitted_manifests() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let mut reordered = run_pipeline(exact_add_verified(), selections.clone());
    reordered.pass_manifests.swap(0, 1);
    assert!(matches!(
        project_optimization_run(reordered),
        Err(OptimizedAbstractProjectionError::AppliedDecisionCustody {
            axis: AppliedDecisionCustodyAxis::ManifestPass,
            ..
        })
    ));

    let mut omitted = run_pipeline(exact_add_verified(), selections);
    omitted.pass_manifests.pop();
    assert!(matches!(
        project_optimization_run(omitted),
        Err(OptimizedAbstractProjectionError::AppliedDecisionCustody {
            axis: AppliedDecisionCustodyAxis::ManifestRoster,
            ..
        })
    ));
}
