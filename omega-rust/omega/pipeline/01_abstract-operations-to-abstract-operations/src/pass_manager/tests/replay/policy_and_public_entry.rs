use super::super::super::validate_external_decision_recording;
use super::super::{
    BaselineDecisionOutcome, ExternalDecisionAction, ExternalDecisionPoint, Optimization,
    OptimizationReasonCode, OptimizationSelections, budget, external_log_with,
    verified_exact_add_unit,
};
use crate::{replay_psi_pipeline, run_psi_pipeline};

#[test]
fn external_skip_can_override_the_baseline_choice() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let [point] = baseline.external_decisions().points() else {
        panic!("exact-add fixture has one decision point");
    };
    let skipped = ExternalDecisionPoint::new(
        point.input(),
        point.rule(),
        point.legal_candidates().iter().cloned(),
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap();
    let external = external_log_with(baseline.external_decisions().context(), [skipped]);
    let replayed = replay_psi_pipeline(
        verified_exact_add_unit(),
        &selections,
        budget(8),
        &external.encode(),
    )
    .unwrap();

    assert!(replayed.commits().is_empty());
    let [retained] = replayed.validated_candidates() else {
        panic!("the skipped validated candidate declaration must be retained");
    };
    assert_eq!(retained.pass(), replayed.pass_manifests()[0].pass());
    assert_eq!(
        retained.declaration().identity(),
        replayed.pass_manifests()[0].decisions()[0].candidate()
    );
    assert_eq!(
        Some(retained.validator()),
        replayed.pass_manifests()[0].decisions()[0].validator()
    );
    assert_eq!(
        replayed.transformation_ledger().input(),
        replayed.transformation_ledger().output()
    );
    assert_eq!(replayed.external_decisions(), &external);
    assert_eq!(
        replayed.decisions().records[0].outcome,
        BaselineDecisionOutcome::Skip(OptimizationReasonCode::NotProfitable)
    );
    assert_eq!(replayed.usage().validation_steps, 1);
    validate_external_decision_recording(&replayed).unwrap();
}
