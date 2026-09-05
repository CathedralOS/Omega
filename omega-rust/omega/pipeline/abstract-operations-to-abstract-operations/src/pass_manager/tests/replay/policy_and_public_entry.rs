use super::super::{
    AbstractOperation, BaselineDecisionOutcome, ExternalDecisionAction, ExternalDecisionPoint,
    Optimization, OptimizationReasonCode, OptimizationRunError, OptimizationSelections,
    OrderedRuleRegistry, budget, built_in_psi_registry, external_log_with, replay_psi_pipeline,
    run_psi_pipeline, run_psi_registry, run_unit, validate_external_decision_recording,
    verified_empty_unit, verified_exact_add_unit, verified_exact_add_zero_unit,
};

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

#[test]
fn public_run_folds_proof_admitted_exact_arithmetic_and_retains_its_context() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let run =
        run_psi_registry(verified_exact_add_unit(), &selections, &registry, budget(8)).unwrap();

    assert_eq!(run.commits.len(), 1);
    assert_eq!(run.transformation_ledger.records().len(), 1);
    assert_eq!(run.pass_manifests[0].decisions().len(), 1);
    assert_eq!(
        run.pass_manifests[0].decisions()[0]
            .consumed_facts()
            .iter()
            .filter(|fact| matches!(
                fact,
                optimization_core::OptimizationFactReference::AcceptedObligation(_)
            ))
            .count(),
        1
    );
    assert_eq!(run.session.unit().accepted_obligation_facts.len(), 1);
    assert_eq!(run.session.unit().proof_questions.len(), 1);
    assert!(matches!(
        run.session.unit().proof_questions[0].owner,
        optimization_unit::ProofQuestionOwner::Operation { .. }
    ));
    assert_eq!(run.session.input().context().accepted_facts().len(), 1);
    assert_eq!(
        run.session.input().context().accepted_facts()[0].obligation,
        semantic_vocabulary::ObligationId::new(419).unwrap()
    );
    assert!(matches!(
        run.session.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(15),
            ..
        }
    ));
}

#[test]
fn public_run_elides_live_proof_certified_identity_and_reaches_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let run = run_psi_pipeline(verified_exact_add_zero_unit(), &selections, budget(8)).unwrap();

    assert_eq!(run.commits.len(), 1);
    assert_eq!(run.pass_manifests.len(), 1);
    assert_eq!(run.pass_manifests[0].ordered_rules().len(), 12);
    assert_eq!(run.pass_manifests[0].decisions().len(), 1);
    assert_eq!(
        run.pass_manifests[0].decisions()[0].consumed_facts().len(),
        2
    );
    assert_eq!(run.session.unit().accepted_obligation_facts.len(), 1);
    assert!(run.session.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            optimization_unit::OptimizationFact::OperationObligationReference { .. }
        )
    }));
    assert!(matches!(
        run.session.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::Return { value, .. }
            if value == semantic_vocabulary::ValueId::new(413).unwrap()
    ));

    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, _, ledger) =
        run_unit(run.session.unit().clone(), &registry, budget(8)).unwrap();
    assert_eq!(output.identity, run.session.unit().identity);
    assert!(commits.is_empty());
    assert_eq!(usage.iterations, 1);
    assert!(ledger.records().is_empty());
}

#[test]
fn public_run_rejects_a_registry_detached_from_named_selections() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let empty = OrderedRuleRegistry::new(Vec::new()).unwrap();
    assert!(matches!(
        run_psi_registry(verified_empty_unit(), &selections, &empty, budget(2)),
        Err(OptimizationRunError::SelectionRegistryMismatch)
    ));
}
