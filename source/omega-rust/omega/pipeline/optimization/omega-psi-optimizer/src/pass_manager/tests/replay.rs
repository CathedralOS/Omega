//! External-decision recording, byte replay, and corruption rejection.

use super::*;

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
    let [point] = baseline.external_decisions().points() else {
        panic!("compatible-policy phi fixture has one external decision point");
    };
    assert_eq!(
        point.rule(),
        crate::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity()
    );
    assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));

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
        point.legal_candidates().iter().copied(),
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
fn external_replay_preflights_every_context_axis() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let context = baseline.external_decisions().context();
    let contexts = [
        (
            ExternalDecisionContext::new(
                omega_optimization_core::OptimizationDecisionSchemaIdentity::from_canonical_bytes(
                    b"foreign schema",
                ),
                context.source(),
                context.selections(),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                context.cost_model(),
            ),
            ExternalDecisionContextAxis::Schema,
        ),
        (
            ExternalDecisionContext::new(
                context.schema(),
                OptimizationUnitIdentity::from_canonical_bytes(b"foreign source"),
                context.selections(),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                context.cost_model(),
            ),
            ExternalDecisionContextAxis::Source,
        ),
        (
            ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([7; 32]),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                context.cost_model(),
            ),
            ExternalDecisionContextAxis::Selections,
        ),
        (
            ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([8; 32]),
                context.target(),
                context.rule_set(),
                context.cost_model(),
            ),
            ExternalDecisionContextAxis::PhaseSelections,
        ),
        (
            ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                context.phase_selections(),
                omega_optimization_core::OptimizationDecisionTargetIdentity::from_canonical_bytes(
                    b"foreign target",
                ),
                context.rule_set(),
                context.cost_model(),
            ),
            ExternalDecisionContextAxis::Target,
        ),
        (
            ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                context.phase_selections(),
                context.target(),
                OptimizationRuleSetIdentity::from_canonical_bytes(b"foreign rules"),
                context.cost_model(),
            ),
            ExternalDecisionContextAxis::RuleSet,
        ),
        (
            ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                TargetCostModelIdentity::from_canonical_bytes(b"foreign cost model"),
            ),
            ExternalDecisionContextAxis::CostModel,
        ),
    ];

    for (supplied, expected_axis) in contexts {
        let external = external_log_with(
            supplied,
            baseline.external_decisions().points().iter().cloned(),
        );
        assert!(matches!(
            replay_psi_pipeline(
                verified_exact_add_unit(),
                &selections,
                budget(8),
                &external.encode(),
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::ContextMismatch(axis)
            )) if axis == expected_axis
        ));
    }
}

#[test]
fn external_replay_rejects_missing_illegal_duplicate_and_leftover_decisions() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let context = baseline.external_decisions().context();
    let point = baseline.external_decisions().points()[0].clone();

    let missing = external_log_with(context, []);
    assert!(matches!(
        replay_psi_pipeline(
            verified_exact_add_unit(),
            &selections,
            budget(8),
            &missing.encode(),
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::MissingDecision { .. }
        ))
    ));

    let mut wrong_cost = point.legal_candidates().to_vec();
    wrong_cost[0].predicted_cost_delta += 1;
    let illegal_point =
        ExternalDecisionPoint::new(point.input(), point.rule(), wrong_cost, point.action())
            .unwrap();
    let illegal = external_log_with(context, [illegal_point]);
    assert!(matches!(
        replay_psi_pipeline(
            verified_exact_add_unit(),
            &selections,
            budget(8),
            &illegal.encode(),
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::IllegalDecision { .. }
        ))
    ));

    let competing = ExternalDecisionPoint::new(
        point.input(),
        point.rule(),
        point.legal_candidates().iter().copied(),
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap();
    let duplicate = external_log_with(context, [point.clone(), competing]);
    assert!(matches!(
        replay_psi_pipeline(
            verified_exact_add_unit(),
            &selections,
            budget(8),
            &duplicate.encode(),
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::DuplicateDecision { .. }
        ))
    ));

    let empty_baseline = run_psi_pipeline(
        verified_empty_unit(),
        &OptimizationSelections::default(),
        budget(2),
    )
    .unwrap();
    let unreachable = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"unreachable input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"unreachable rule"),
        [ValidatedCandidateSummary {
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(
                b"unreachable candidate",
            ),
            predicted_cost_delta: -1,
        }],
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap();
    let leftover = external_log_with(empty_baseline.external_decisions().context(), [unreachable]);
    assert!(matches!(
        replay_psi_pipeline(
            verified_empty_unit(),
            &OptimizationSelections::default(),
            budget(2),
            &leftover.encode(),
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::LeftoverDecisions { remaining: 1, .. }
        ))
    ));
}

#[test]
fn external_replay_byte_boundary_rejects_exact_duplicate_and_foreign_action() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();

    let mut duplicated = baseline.external_decisions().encode();
    const LOG_POINT_COUNT_OFFSET: usize = 8 + 4 + 32 + 7 * 32;
    const LOG_POINTS_OFFSET: usize = LOG_POINT_COUNT_OFFSET + 4;
    let framed_point = duplicated[LOG_POINTS_OFFSET..].to_vec();
    duplicated[LOG_POINT_COUNT_OFFSET..LOG_POINTS_OFFSET].copy_from_slice(&2_u32.to_le_bytes());
    duplicated.extend_from_slice(&framed_point);
    assert!(matches!(
        replay_psi_pipeline(
            verified_exact_add_unit(),
            &selections,
            budget(8),
            &duplicated,
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::Schema(
                ExternalDecisionSchemaError::DuplicateDecisionPoint
            )
        ))
    ));

    let mut foreign_action = baseline.external_decisions().encode();
    const POINT_BODY_OFFSET: usize = LOG_POINTS_OFFSET + 4;
    const ACTION_CANDIDATE_OFFSET: usize = POINT_BODY_OFFSET + 8 + 4 + 32 + 32 + 32 + 4 + 40 + 1;
    foreign_action[ACTION_CANDIDATE_OFFSET..ACTION_CANDIDATE_OFFSET + 32]
        .copy_from_slice(&OptimizationCandidateIdentity::from_canonical_bytes(b"foreign").bytes());
    assert!(matches!(
        replay_psi_pipeline(
            verified_exact_add_unit(),
            &selections,
            budget(8),
            &foreign_action,
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::Schema(ExternalDecisionSchemaError::IllegalAction)
        ))
    ));
}

#[test]
fn external_policy_input_cannot_bypass_candidate_validation() {
    let unit = exact_add_unit();
    let original = unit.clone();
    let registry = OrderedRuleRegistry::new([
        Arc::new(InvalidEvaluationExactRule) as Arc<dyn PsiOptimizationRule>
    ])
    .unwrap();
    let mut analyses = AnalysisManager::new(&unit);
    let products = analyses
        .require_all(
            &unit,
            InvalidEvaluationExactRule.contract().required_analyses(),
        )
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = InvalidEvaluationExactRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .remove(0);
    let analysis_revision = analyses.revision();
    let cached_analyses = analyses.cached_kinds().collect::<Vec<_>>();
    assert!(matches!(
        validate_psi_rewrite_candidate(&unit, &candidate),
        Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
    ));
    assert_eq!(
        unit, original,
        "rejected validation cannot mutate its input"
    );
    assert_eq!(analyses.revision(), analysis_revision);
    assert_eq!(analyses.cached_kinds().collect::<Vec<_>>(), cached_analyses);

    let rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&[InvalidEvaluationExactRule
        .contract()
        .identity()])
    .unwrap();
    let context = ExternalDecisionContext::new(
        external_psi_decision_schema_v1_identity(),
        unit.identity,
        OptimizationSelections::default().identity(),
        OptimizationSelections::default().identity(),
        psi_target_neutral_decision_target_v1_identity(),
        rule_set,
        baseline_psi_cost_model_identity(),
    );
    let point = ExternalDecisionPoint::new(
        unit.identity,
        candidate.rule(),
        [ValidatedCandidateSummary {
            candidate: candidate.identity(),
            predicted_cost_delta: candidate.predicted_cost_delta(),
        }],
        ExternalDecisionAction::Choose(candidate.identity()),
    )
    .unwrap();
    let log = ExternalDecisionLog::new(context, [point]).unwrap();
    let mut cursor = ExternalDecisionReplayCursor::new(&log, context).unwrap();

    assert!(matches!(
        run_unit_inner(unit, &registry, budget(2), Some(&mut cursor)),
        Err(OptimizationRunError::CandidateValidation(
            OptimizationUnitValidationError::CandidateEvaluationMismatch
        ))
    ));
    assert_eq!(
        cursor.next, 0,
        "invalid candidate did not consume policy input"
    );
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
                omega_optimization_core::OptimizationFactReference::AcceptedObligation(_)
            ))
            .count(),
        1
    );
    assert_eq!(run.session.unit().accepted_obligation_facts.len(), 1);
    assert_eq!(run.session.unit().proof_questions.len(), 1);
    assert!(matches!(
        run.session.unit().proof_questions[0].owner,
        omega_optimization_unit::ProofQuestionOwner::Operation { .. }
    ));
    assert_eq!(run.session.input().context().accepted_facts().len(), 1);
    assert_eq!(
        run.session.input().context().accepted_facts()[0].obligation,
        psi_core::ObligationId::new(419).unwrap()
    );
    assert!(matches!(
        run.session.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(15),
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
            omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
        )
    }));
    assert!(matches!(
        run.session.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::Return { value, .. }
            if value == psi_core::ValueId::new(413).unwrap()
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
