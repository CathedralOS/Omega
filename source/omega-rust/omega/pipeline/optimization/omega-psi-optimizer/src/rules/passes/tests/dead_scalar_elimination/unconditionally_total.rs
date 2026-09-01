//! Unconditionally-total dead scalar elimination semantics and refusal boundaries.

use super::*;

#[test]
fn rule_removes_wrapping_add_but_not_proof_bearing_exact_add() {
    let unit = dead_wrapping_add_unit();
    let contract = DeadUnconditionallyTotalScalarEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = DeadUnconditionallyTotalScalarEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("only the unused wrapping add is in this rule family");
    assert_eq!(candidate.node_decision_point().unwrap().node, 2);
    let accepted = validate_dead_scalar_node_candidate(&unit, &candidate).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 3);
    assert_eq!(
        accepted.unit().functions[0].blocks[0].nodes[2]
            .provenance
            .len(),
        2
    );

    let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
        unreachable!()
    };
    let wrong_family = PsiRewriteCandidate::new_dead_scalar_node(
        unit.identity,
        DeadScalarLiteralEliminationRule::contract(),
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_dead_scalar_node_candidate(&unit, &wrong_family),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let exact = dead_exact_add_unit();
    validate_psi_optimization_unit(&exact).unwrap();
    let liveness = compute_analysis(&exact, AnalysisKind::ValueLiveness).unwrap();
    let effects = compute_analysis(&exact, AnalysisKind::EffectSummaries).unwrap();
    assert!(
        DeadUnconditionallyTotalScalarEliminationRule
            .propose(&exact, RuleAnalysisView::new(&[liveness, effects]))
            .unwrap()
            .is_empty()
    );
}
