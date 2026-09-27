use super::super::super::validate_external_decision_recording;
use super::super::{
    ExternalDecisionAction, Optimization, OptimizationSelections, budget,
    verified_compatible_policy_cse_unit, verified_compatible_policy_phi_gvn_unit,
    verified_exact_add_unit,
};
use crate::{replay_psi_pipeline, run_psi_pipeline};

#[test]
fn external_policy_features_are_exact_manifest_evidence() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let [point] = baseline.external_decisions().points() else {
        panic!("exact-add fixture has one external decision point");
    };
    let [features] = point.legal_candidates() else {
        panic!("exact-add fixture has one legal candidate");
    };
    let manifest_decision = baseline
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .find(|decision| decision.candidate() == features.candidate())
        .expect("external feature row has an authoritative manifest decision");

    assert_eq!(features.summary().candidate, manifest_decision.candidate());
    assert_eq!(
        features.summary().predicted_cost_delta,
        baseline.decisions().records[0].considered[0].predicted_cost_delta
    );
    assert_eq!(
        features.consumed_analyses(),
        manifest_decision.consumed_analyses()
    );
    assert_eq!(
        features.consumed_facts(),
        manifest_decision.consumed_facts()
    );
    assert!(!features.consumed_analyses().is_empty());
    assert!(!features.consumed_facts().is_empty());
}

#[test]
fn external_decision_record_and_replay_preserve_compatible_policy_gvn() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let baseline = run_psi_pipeline(
        verified_compatible_policy_cse_unit(),
        &selections,
        budget(8),
    )
    .unwrap();
    let [point] = baseline.external_decisions().points() else {
        panic!("compatible-policy fixture has one external decision point");
    };
    assert_eq!(
        point.rule(),
        crate::rules::SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity()
    );
    assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));

    let replayed = replay_psi_pipeline(
        verified_compatible_policy_cse_unit(),
        &selections,
        budget(8),
        &baseline.external_decisions().encode(),
    )
    .unwrap();
    assert_eq!(replayed.session().unit(), baseline.session().unit());
    assert_eq!(replayed.commits(), baseline.commits());
    assert_eq!(replayed.decisions(), baseline.decisions());
    assert_eq!(replayed.external_decisions(), baseline.external_decisions());
    assert_eq!(replayed.pass_manifests(), baseline.pass_manifests());
    assert_eq!(
        replayed.transformation_ledger(),
        baseline.transformation_ledger()
    );
    validate_external_decision_recording(&replayed).unwrap();
}

#[test]
fn external_decision_record_and_replay_preserve_compatible_policy_phi_gvn() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let baseline = run_psi_pipeline(
        verified_compatible_policy_phi_gvn_unit(),
        &selections,
        budget(8),
    )
    .unwrap();
    let points = baseline.external_decisions().points();
    let point = points
        .iter()
        .find(|point| {
            point.rule()
                == crate::rules::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(
                )
                .identity()
        })
        .expect("compatible-policy phi fixture retains its exact decision point");
    assert_eq!(
        point.rule(),
        crate::rules::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract()
            .identity()
    );
    assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));
    assert!(points.iter().any(|point| {
        point.rule() == crate::rules::WrappingShiftZeroCountIdentityRule::contract().identity()
    }));
    let replayed = replay_psi_pipeline(
        verified_compatible_policy_phi_gvn_unit(),
        &selections,
        budget(8),
        &baseline.external_decisions().encode(),
    )
    .unwrap();
    assert_eq!(replayed.session().unit(), baseline.session().unit());
    assert_eq!(replayed.commits(), baseline.commits());
    assert_eq!(replayed.decisions(), baseline.decisions());
    assert_eq!(replayed.external_decisions(), baseline.external_decisions());
    assert_eq!(replayed.pass_manifests(), baseline.pass_manifests());
    assert_eq!(
        replayed.transformation_ledger(),
        baseline.transformation_ledger()
    );
    validate_external_decision_recording(&replayed).unwrap();
}
