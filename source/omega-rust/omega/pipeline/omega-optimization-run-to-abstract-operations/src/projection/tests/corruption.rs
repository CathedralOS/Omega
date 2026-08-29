use super::*;

#[test]
fn independent_validation_rejects_projected_operation_corruption() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let optimized = project_optimization_run(run(exact_add_verified(), selections)).unwrap();
    let mut corrupted = optimized.plan().clone();
    let AbstractOperation::IntegerConstant { value, .. } =
        &mut corrupted.functions[0].operations[2]
    else {
        panic!("folded operation must be a constant")
    };
    *value = IntegerValue::Unsigned(16);
    let registry = built_in_psi_registry(optimized.selections()).unwrap();

    assert_eq!(
        validate_optimized_abstract_plan_projection(
            optimized.verified_input(),
            optimized.unit(),
            &corrupted,
            optimized.selections(),
            optimized.psi_selections(),
            registry.identity(),
            baseline_psi_cost_model_identity(),
            optimized.decisions(),
            optimized.pass_manifests(),
            optimized.transformation_ledger(),
            optimized.identity_bundle(),
        ),
        Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch)
    );
}

#[test]
fn independent_validation_rejects_block_offset_corruption() {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized =
        project_optimization_run(run(redundant_block_parameter_verified(), selections)).unwrap();
    let mut corrupted = optimized.plan().clone();
    corrupted.functions[0].block_entries[1].operation_offset += 1;
    let registry = built_in_psi_registry(optimized.selections()).unwrap();

    assert_eq!(
        validate_optimized_abstract_plan_projection(
            optimized.verified_input(),
            optimized.unit(),
            &corrupted,
            optimized.selections(),
            optimized.psi_selections(),
            registry.identity(),
            baseline_psi_cost_model_identity(),
            optimized.decisions(),
            optimized.pass_manifests(),
            optimized.transformation_ledger(),
            optimized.identity_bundle(),
        ),
        Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch)
    );
}

#[test]
fn candidate_replay_rejects_corrupted_commit_custody() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let mut run = run(exact_add_verified(), selections);
    run.commits[0].input = run.commits[0].output;

    assert!(matches!(
        project_optimization_run(run),
        Err(OptimizedAbstractProjectionError::CommitReplayMismatch)
    ));
}
