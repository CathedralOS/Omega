use super::super::{
    AnalysisSet, ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionContextAxis, ExternalDecisionPoint, ExternalDecisionReplayError, Optimization,
    OptimizationCandidateIdentity, OptimizationReasonCode, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity, OptimizationRunError, OptimizationSelections,
    OptimizationUnitIdentity, TargetCostModelIdentity, ValidatedCandidateSummary, budget,
    external_log_with, replay_psi_pipeline, run_psi_pipeline, verified_empty_unit,
    verified_exact_add_unit,
};

#[test]
fn external_replay_preflights_every_context_axis() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let context = baseline.external_decisions().context();
    let contexts = [
        (
            ExternalDecisionContext::new(
                optimization_core::OptimizationDecisionSchemaIdentity::from_canonical_bytes(
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
                optimization_core::OptimizationSelectionIdentity::from_bytes([7; 32]),
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
                optimization_core::OptimizationSelectionIdentity::from_bytes([8; 32]),
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
                optimization_core::OptimizationDecisionTargetIdentity::from_canonical_bytes(
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

    let original_features = &point.legal_candidates()[0];
    let wrong_cost = [ExternalCandidateFeatures::new(
        ValidatedCandidateSummary {
            candidate: original_features.candidate(),
            predicted_cost_delta: original_features.predicted_cost_delta() + 1,
        },
        original_features.consumed_analyses(),
        original_features.consumed_facts().iter().copied(),
    )
    .unwrap()];
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

    let altered_evidence = [
        ExternalCandidateFeatures::new(
            original_features.summary(),
            AnalysisSet::default(),
            original_features.consumed_facts().iter().copied(),
        )
        .unwrap(),
        ExternalCandidateFeatures::new(
            original_features.summary(),
            original_features.consumed_analyses(),
            [],
        )
        .unwrap(),
    ];
    for features in altered_evidence {
        let altered_point =
            ExternalDecisionPoint::new(point.input(), point.rule(), [features], point.action())
                .unwrap();
        let altered = external_log_with(context, [altered_point]);
        assert!(matches!(
            replay_psi_pipeline(
                verified_exact_add_unit(),
                &selections,
                budget(8),
                &altered.encode(),
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::IllegalDecision { .. }
            ))
        ));
    }

    let competing = ExternalDecisionPoint::new(
        point.input(),
        point.rule(),
        point.legal_candidates().iter().cloned(),
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
        [ExternalCandidateFeatures::new(
            ValidatedCandidateSummary {
                candidate: OptimizationCandidateIdentity::from_canonical_bytes(
                    b"unreachable candidate",
                ),
                predicted_cost_delta: -1,
            },
            AnalysisSet::default(),
            [],
        )
        .unwrap()],
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
