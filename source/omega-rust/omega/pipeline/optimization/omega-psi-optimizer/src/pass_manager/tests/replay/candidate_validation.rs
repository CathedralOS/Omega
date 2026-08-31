use super::super::{
    AnalysisManager, Arc, CandidateContractAxis, DetachedCandidateContractRule,
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionReplayCursor,
    InvalidEvaluationExactRule, OptimizationRuleSetIdentity, OptimizationRunError,
    OptimizationSelections, OptimizationUnitValidationError, OrderedRuleRegistry,
    PsiOptimizationRule, RuleAnalysisView, ValidatedCandidateSummary,
    baseline_psi_cost_model_identity, budget, exact_add_unit,
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
    run_unit, run_unit_inner, validate_psi_rewrite_candidate,
};

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
        external_psi_decision_schema_v2_identity(),
        unit.identity,
        OptimizationSelections::default().identity(),
        OptimizationSelections::default().identity(),
        psi_target_neutral_decision_target_v2_identity(),
        rule_set,
        baseline_psi_cost_model_identity(),
    );
    let point = ExternalDecisionPoint::new(
        unit.identity,
        candidate.rule(),
        [ExternalCandidateFeatures::new(
            ValidatedCandidateSummary {
                candidate: candidate.identity(),
                predicted_cost_delta: candidate.predicted_cost_delta(),
            },
            InvalidEvaluationExactRule.contract().required_analyses(),
            candidate.consumed_facts().iter().copied(),
        )
        .unwrap()],
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
        cursor.consumed_points(),
        0,
        "invalid candidate did not consume policy input"
    );
}

#[test]
fn candidate_contract_cannot_detach_policy_features_from_the_scheduled_rule() {
    let registry = OrderedRuleRegistry::new([
        Arc::new(DetachedCandidateContractRule) as Arc<dyn PsiOptimizationRule>
    ])
    .unwrap();

    assert!(matches!(
        run_unit(exact_add_unit(), &registry, budget(2)),
        Err(OptimizationRunError::CandidateContractMismatch {
            axis: CandidateContractAxis::Rule,
            ..
        })
    ));
}
