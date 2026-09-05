use super::super::*;

pub(super) fn optimized() -> ValidatedOptimizedAbstractPlan {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    publish_optimization_run(run(exact_add_verified(), selections)).unwrap()
}

pub(super) fn donor() -> ValidatedOptimizedAbstractPlan {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    publish_optimization_run(run(exact_add_verified(), selections)).unwrap()
}

pub(super) fn validate(
    optimized: &ValidatedOptimizedAbstractPlan,
    candidate: &PrePhysicalOptimizationManifest,
) -> Result<
    abstract_operations_to_abstract_operations::validation::ValidatedPrePhysicalOptimizationManifest,
    PrePhysicalOptimizationManifestError,
>{
    validate_pre_physical_optimization_manifest(
        candidate,
        optimized.verified_input(),
        optimized.unit(),
        optimized.selections(),
        optimized.psi_selections(),
        optimized.budget_per_pass(),
        work_usage(optimized.usage()),
        optimized.decisions(),
        optimized.pass_manifests(),
        optimized.transformation_ledger(),
        optimized.identity_bundle(),
        optimized.validation(),
    )
}
