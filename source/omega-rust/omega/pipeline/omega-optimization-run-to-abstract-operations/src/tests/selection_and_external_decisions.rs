//! Selection and external-decision projection custody.

use super::*;

#[test]
fn empty_selection_projects_the_original_plan_deterministically() {
    let selections = OptimizationSelections::new([]).unwrap();
    let first = project_optimization_run(run(empty_verified(), selections.clone())).unwrap();
    let second = project_optimization_run(run(empty_verified(), selections)).unwrap();

    assert_eq!(first.plan(), first.verified_input().plan());
    assert_eq!(first.plan(), second.plan());
    assert_eq!(first.validation(), second.validation());
    assert!(first.commits().is_empty());
    assert!(first.validated_candidates().is_empty());
    assert!(first.pass_manifests().is_empty());
}

#[test]
fn projection_rejects_detached_external_decision_recording() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let mut optimized = run(exact_add_verified(), selections);
    let empty = run(empty_verified(), OptimizationSelections::default());
    optimized.external_decisions = empty.external_decisions;

    assert!(matches!(
        project_optimization_run(optimized),
        Err(OptimizedAbstractProjectionError::ExternalDecisionRecordingMismatch)
    ));
}

#[test]
fn externally_replayed_psi_decisions_reach_verified_terminal_projection() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_pipeline(exact_add_verified(), selections.clone());
    let external = baseline.external_decisions().clone();
    let replayed = replay_psi_pipeline(
        exact_add_verified(),
        &selections,
        work_budget(),
        &external.encode(),
    )
    .unwrap();
    let optimized = project_optimization_run(replayed).unwrap();

    assert_eq!(optimized.external_decisions(), &external);
    assert_eq!(optimized.commits().len(), 1);
    assert!(matches!(
        optimized.plan().functions[0].operations[2],
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(15),
            ..
        }
    ));
}

#[test]
fn lower_only_suite_records_no_psi_completion() {
    let selections =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    let optimized =
        project_optimization_run(run_pipeline(empty_verified(), selections.clone())).unwrap();

    assert_eq!(optimized.selections(), &selections);
    assert!(optimized.psi_selections().is_empty());
    assert_eq!(optimized.plan(), optimized.verified_input().plan());
    assert!(optimized.commits().is_empty());
    assert!(optimized.pass_manifests().is_empty());
    assert_eq!(
        optimized.pre_physical_manifest().record().selections,
        selections
    );
    assert!(
        optimized
            .pre_physical_manifest()
            .record()
            .psi_selections
            .is_empty()
    );
}

#[test]
fn mixed_suite_records_only_its_psi_completion() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let psi_selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let optimized =
        project_optimization_run(run_pipeline(exact_add_verified(), selections.clone())).unwrap();

    assert_eq!(optimized.selections(), &selections);
    assert_eq!(optimized.psi_selections(), &psi_selections);
    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(
        optimized.pre_physical_manifest().record().selections,
        selections
    );
    assert_eq!(
        optimized.pre_physical_manifest().record().psi_selections,
        psi_selections
    );
}

#[test]
fn projection_rejects_a_tampered_psi_phase_projection() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let mut run = run_pipeline(exact_add_verified(), selections);
    run.psi_selections = OptimizationSelections::default();

    assert!(matches!(
        project_optimization_run(run),
        Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch)
    ));
}
