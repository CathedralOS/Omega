use super::super::{
    ExternalDecisionAction, Optimization, OptimizationRunError, OptimizationSelections, budget,
    built_in_psi_registry, replay_psi_pipeline, replay_psi_registry, run_psi_pipeline,
    run_psi_registry, validate_external_decision_recording, verified_compatible_policy_cse_unit,
    verified_compatible_policy_phi_gvn_unit, verified_empty_unit, verified_exact_add_unit,
    verified_exact_remainder_by_one_unit, verified_exact_self_divide_unit,
    verified_exact_self_remainder_unit, verified_exact_signed_remainder_by_negative_one_unit,
};

#[test]
fn external_decision_recording_rejects_detached_valid_context() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let mut run = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let empty = run_psi_pipeline(
        verified_empty_unit(),
        &OptimizationSelections::default(),
        budget(2),
    )
    .unwrap();
    run.external_decisions = empty.external_decisions;

    assert_eq!(
        validate_external_decision_recording(&run),
        Err(OptimizationRunError::ExternalDecisionManifestMismatch)
    );
}

#[test]
fn external_decision_replay_preserves_the_complete_baseline_run() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let encoded = baseline.external_decisions().encode();
    let replayed =
        replay_psi_pipeline(verified_exact_add_unit(), &selections, budget(8), &encoded).unwrap();

    assert_eq!(replayed.session.unit(), baseline.session.unit());
    assert_eq!(replayed.commits, baseline.commits);
    assert_eq!(replayed.usage, baseline.usage);
    assert_eq!(replayed.decisions, baseline.decisions);
    assert_eq!(replayed.external_decisions, baseline.external_decisions);
    assert_eq!(replayed.pass_manifests, baseline.pass_manifests);
    assert_eq!(
        replayed.transformation_ledger,
        baseline.transformation_ledger
    );
    assert_eq!(replayed.identity_bundle, baseline.identity_bundle);
    validate_external_decision_recording(&replayed).unwrap();
}

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
fn external_decision_record_and_replay_preserve_self_remainder_validation() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let baseline =
        run_psi_pipeline(verified_exact_self_remainder_unit(), &selections, budget(8)).unwrap();
    let [point] = baseline.external_decisions().points() else {
        panic!("self-remainder fixture has one external decision point");
    };
    assert_eq!(
        point.rule(),
        crate::LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract().identity()
    );
    assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));
    assert_eq!(baseline.usage().validation_steps, 1);

    let replayed = replay_psi_pipeline(
        verified_exact_self_remainder_unit(),
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
    assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
    assert_eq!(replayed.usage().validation_steps, 1);
    validate_external_decision_recording(&replayed).unwrap();
}

#[test]
fn external_decision_record_and_replay_preserve_self_divide_validation() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let baseline =
        run_psi_pipeline(verified_exact_self_divide_unit(), &selections, budget(8)).unwrap();
    let [point] = baseline.external_decisions().points() else {
        panic!("self-divide fixture has one external decision point");
    };
    assert_eq!(
        point.rule(),
        crate::LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity()
    );
    assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));
    assert_eq!(baseline.usage().validation_steps, 1);

    let replayed = replay_psi_pipeline(
        verified_exact_self_divide_unit(),
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
    assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
    assert_eq!(replayed.usage().validation_steps, 1);
    validate_external_decision_recording(&replayed).unwrap();
}

#[test]
fn external_decision_record_and_replay_preserve_remainder_by_one_validation() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let baseline = run_psi_pipeline(
        verified_exact_remainder_by_one_unit(),
        &selections,
        budget(8),
    )
    .unwrap();
    let [point] = baseline.external_decisions().points() else {
        panic!("remainder-by-one fixture has one external decision point");
    };
    assert_eq!(
        point.rule(),
        crate::LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract().identity()
    );
    assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));
    assert_eq!(point.legal_candidates().len(), 1);
    assert_eq!(baseline.usage().validation_steps, 1);

    let replayed = replay_psi_pipeline(
        verified_exact_remainder_by_one_unit(),
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
    assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
    assert_eq!(replayed.usage().validation_steps, 1);
    validate_external_decision_recording(&replayed).unwrap();
}

#[test]
fn external_decision_replay_preserves_signed_remainder_by_negative_one_validation() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let baseline = run_psi_pipeline(
        verified_exact_signed_remainder_by_negative_one_unit(),
        &selections,
        budget(8),
    )
    .unwrap();
    let [point] = baseline.external_decisions().points() else {
        panic!("signed remainder-by-negative-one fixture has one external decision point");
    };
    assert_eq!(
        point.rule(),
        crate::LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule::contract()
            .identity()
    );
    assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));
    assert_eq!(point.legal_candidates().len(), 1);
    assert_eq!(baseline.usage().validation_steps, 1);

    let replayed = replay_psi_pipeline(
        verified_exact_signed_remainder_by_negative_one_unit(),
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
    assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
    assert_eq!(replayed.usage().validation_steps, 1);
    validate_external_decision_recording(&replayed).unwrap();
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
        crate::SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity()
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
                == crate::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract()
                    .identity()
        })
        .expect("compatible-policy phi fixture retains its exact decision point");
    assert_eq!(
        point.rule(),
        crate::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity()
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

#[test]
fn external_decision_replay_supports_the_exact_registry_entry_point() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let baseline =
        run_psi_registry(verified_exact_add_unit(), &selections, &registry, budget(8)).unwrap();
    let replayed = replay_psi_registry(
        verified_exact_add_unit(),
        &selections,
        &registry,
        budget(8),
        &baseline.external_decisions().encode(),
    )
    .unwrap();

    assert_eq!(replayed.session().unit(), baseline.session().unit());
    assert_eq!(replayed.decisions(), baseline.decisions());
    assert_eq!(replayed.external_decisions(), baseline.external_decisions());
    assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
}
