//! Dead scalar elimination tests.

use super::*;

#[test]
fn dead_scalar_literals_rehome_operation_custody_without_tombstones() {
    let unit = dead_scalar_literals_unit();
    let contract = DeadScalarLiteralEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidates = DeadScalarLiteralEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap();
    assert_eq!(candidates.len(), 2);
    let first = candidates
        .iter()
        .find(|candidate| candidate.node_decision_point().unwrap().node == 0)
        .unwrap();
    let accepted = validate_dead_scalar_node_candidate(&unit, first).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
    assert_eq!(accepted.unit().functions[0].facts.len(), 1);
    assert_eq!(
        accepted.unit().functions[0].blocks[0].nodes[0].provenance,
        [
            PsiProvenance::Operation(id(1_206, OperationId::new)),
            PsiProvenance::Operation(id(1_205, OperationId::new)),
        ]
    );
    assert!(
        accepted
            .provenance()
            .iter()
            .all(|row| row.disposition.is_realized())
    );

    let next_unit = accepted.into_unit();
    let mut manager = crate::AnalysisManager::new(&next_unit);
    let products = manager
        .require_all(&next_unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [second] = DeadScalarLiteralEliminationRule
        .propose(&next_unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("only the inherited integer literal remains dead");
    let final_unit = validate_dead_scalar_node_candidate(&next_unit, &second)
        .unwrap()
        .into_unit();
    let terminal = &final_unit.functions[0].blocks[0].nodes[0];
    assert!(matches!(terminal.operation, O::ReturnUnit { .. }));
    assert_eq!(
        terminal.provenance,
        [
            PsiProvenance::Edge(id(1_207, EdgeId::new)),
            PsiProvenance::Operation(id(1_206, OperationId::new)),
            PsiProvenance::Operation(id(1_205, OperationId::new)),
        ]
    );

    let mut used = unit.clone();
    used.functions[0].blocks[0].nodes[2].operation = O::Return {
        psi_edge: id(1_207, EdgeId::new),
        result: id(1_204, ValueId::new),
        value: id(1_204, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        cleanup_actions: Vec::new(),
    };
    used.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: id(1_204, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    });
    used.functions[0].blocks[0].nodes[2].uses = vec![omega_optimization_unit::ValueUse {
        value: id(1_204, ValueId::new),
        block: id(1_202, BlockId::new),
        node: 2,
    }];
    used.identity = recompute_psi_optimization_unit_identity(&used);
    validate_psi_optimization_unit(&used).unwrap();
    let liveness = compute_analysis(&used, AnalysisKind::ValueLiveness).unwrap();
    let effects = compute_analysis(&used, AnalysisKind::EffectSummaries).unwrap();
    let proposed = DeadScalarLiteralEliminationRule
        .propose(&used, RuleAnalysisView::new(&[liveness, effects]))
        .unwrap();
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].node_decision_point().unwrap().node, 0);
}

#[test]
fn dead_total_scalar_rule_removes_wrapping_add_but_not_proof_bearing_exact_add() {
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
