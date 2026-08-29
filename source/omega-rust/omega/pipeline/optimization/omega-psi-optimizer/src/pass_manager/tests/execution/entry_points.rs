//! Public run entry points and exact phase-projection custody.

use super::super::*;

#[test]
fn public_run_requires_and_retains_verified_optimizer_context() {
    let selections = OptimizationSelections::default();
    let registry = OrderedRuleRegistry::new(Vec::new()).unwrap();
    let run = run_psi_registry(verified_empty_unit(), &selections, &registry, budget(2)).unwrap();
    assert!(run.commits.is_empty());
    assert!(run.pass_manifests.is_empty());
    assert!(run.external_decisions().points().is_empty());
    assert_eq!(
        ExternalDecisionLog::decode(&run.external_decisions().encode()),
        Ok(run.external_decisions().clone())
    );
    assert_eq!(
        run.external_decisions().context().source(),
        run.transformation_ledger.input()
    );
    assert!(run.transformation_ledger.records().is_empty());
    assert_eq!(run.identity_bundle.selections(), selections.identity());
    assert_eq!(
        run.identity_bundle.transformation_ledger(),
        run.transformation_ledger.identity()
    );
    assert_eq!(run.usage.iterations, 0);
    assert_eq!(run.session.unit().psi, run.session.input().plan().psi);
}

#[test]
fn lower_only_suite_retains_the_request_but_executes_no_psi_pass() {
    let selections =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    let run = run_psi_pipeline(verified_empty_unit(), &selections, budget(2)).unwrap();

    assert_eq!(run.selections(), &selections);
    assert!(run.psi_selections().is_empty());
    assert_eq!(run.identity_bundle.selections(), selections.identity());
    assert_eq!(
        run.identity_bundle.rule_set(),
        OptimizationRuleSetIdentity::from_ordered_rules(&[]).unwrap()
    );
    assert!(run.commits.is_empty());
    assert!(run.pass_manifests.is_empty());
    assert!(run.decisions.records.is_empty());
    assert!(run.external_decisions().points().is_empty());
    assert_eq!(
        run.external_decisions().context().selections(),
        selections.identity()
    );
    assert_eq!(
        run.external_decisions().context().phase_selections(),
        run.psi_selections().identity()
    );
    assert!(run.transformation_ledger.records().is_empty());
    assert_eq!(run.usage, OptimizationRunUsage::default());
    assert_eq!(
        run.transformation_ledger.input(),
        run.transformation_ledger.output()
    );
}

#[test]
fn mixed_suite_executes_only_its_psi_projection() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let psi_selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let run = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let registry = built_in_psi_registry(&psi_selections).unwrap();

    assert_eq!(run.selections(), &selections);
    assert_eq!(run.psi_selections(), &psi_selections);
    assert_eq!(run.identity_bundle.selections(), selections.identity());
    assert_eq!(run.identity_bundle.rule_set(), registry.identity());
    assert_eq!(run.pass_manifests.len(), 1);
    assert_eq!(run.commits.len(), 1);
    assert_eq!(run.external_decisions().points().len(), 1);
    let external = &run.external_decisions().points()[0];
    let baseline = &run.decisions().records[0];
    assert_eq!(external.input(), baseline.input);
    assert_eq!(external.action(), baseline.outcome.into());
    assert_eq!(external.legal_candidates().len(), baseline.considered.len());
    assert_eq!(external.rule(), run.pass_manifests[0].decisions()[0].rule());
    assert_eq!(
        run.identity_bundle.decision_log(),
        Some(run.decisions().identity)
    );
    assert_ne!(
        run.external_decisions().identity(),
        run.decisions().identity
    );
    assert_eq!(
        ExternalDecisionLog::decode(&run.external_decisions().encode()),
        Ok(run.external_decisions().clone())
    );
}
