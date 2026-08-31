//! Cross-block dominating-leader behavior.

use super::*;

#[test]
fn proof_certified_dominator_gvn_consumes_cross_block_redundant_evidence() {
    let unit = proof_certified_dominator_gvn_unit();
    let contract = DominatorProofCertifiedScalarGvnRule::contract();
    assert_eq!(
        contract.safety_class(),
        OptimizationSafetyClass::ProofCertified
    );
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = DominatorProofCertifiedScalarGvnRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("the entry exact add dominates one proof-certified duplicate");
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == id(1_351, OperationId::new))
        .expect("fixture retains the dominated operation fact")
        .identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(patch.leader.block, id(1_343, BlockId::new));
    assert_eq!(patch.redundant.block, id(1_342, BlockId::new));
    assert_eq!(patch.leader_operation, id(1_349, OperationId::new));
    assert_eq!(patch.redundant_operation, id(1_351, OperationId::new));
    let accepted =
        validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_351, OperationId::new)
        )
    }));

    let forged = PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign proof-certified dominator GVN fact",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
}

#[test]
fn dominator_gvn_reuses_a_canonical_cross_block_total_scalar_expression() {
    let unit = dominator_gvn_unit();
    let local_contract = SameBlockTotalScalarCseRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let local_products = manager
        .require_all(&unit, local_contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockTotalScalarCseRule
            .propose(&unit, RuleAnalysisView::new(&local_products))
            .unwrap()
            .is_empty()
    );

    let contract = DominatorTotalScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = DominatorTotalScalarGvnRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("entry expression strictly dominates one cross-block duplicate");
    let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(patch.leader.block, id(1_343, BlockId::new));
    assert_eq!(patch.redundant.block, id(1_342, BlockId::new));
    let accepted =
        validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 2);
    assert!(
        matches!(output.functions[0].blocks[0].nodes[0].operation, O::IntegerEqual { left, right, .. } if left == id(1_346, ValueId::new) && right == left)
    );
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].provenance,
        [
            PsiProvenance::Operation(id(1_352, OperationId::new)),
            PsiProvenance::Operation(id(1_351, OperationId::new))
        ]
    );
    assert!(
        output.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .all(|row| row.value != id(1_347, ValueId::new))
    );

    let mut forged_patch = patch;
    forged_patch.leader.node = 1;
    forged_patch.leader_operation = id(1_350, OperationId::new);
    let forged = PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        -1,
        forged_patch,
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
}

#[test]
fn dominator_gvn_cascades_through_a_non_topological_diamond_to_fixed_point() {
    let mut unit = diamond_dominator_gvn_unit();
    let contract = DominatorTotalScalarGvnRule::contract();
    for (expected_redundant, expected_leader) in [
        (id(1_410, ValueId::new), id(1_408, ValueId::new)),
        (id(1_411, ValueId::new), id(1_409, ValueId::new)),
    ] {
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("one newly exposed cross-block value number");
        let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.redundant_result, expected_redundant);
        assert_eq!(patch.leader_result, expected_leader);
        assert_eq!(
            candidate.affected_blocks(),
            [
                id(1_402, BlockId::new),
                id(1_403, BlockId::new),
                id(1_404, BlockId::new),
                id(1_405, BlockId::new)
            ]
        );
        unit = validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate)
            .unwrap()
            .into_unit();
    }
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
    let join = &unit.functions[0].blocks[0];
    assert_eq!(join.nodes.len(), 1);
    assert!(
        matches!(join.nodes[0].operation, O::Return { value, .. } if value == id(1_409, ValueId::new))
    );
    assert_eq!(
        join.nodes[0].provenance,
        [
            PsiProvenance::Edge(id(1_414, EdgeId::new)),
            PsiProvenance::Operation(id(1_413, OperationId::new)),
            PsiProvenance::Operation(id(1_412, OperationId::new))
        ]
    );
}

#[test]
fn dominator_gvn_rejects_an_equivalent_sibling_expression_at_a_join() {
    let unit = sibling_only_gvn_unit();
    let contract = DominatorTotalScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let function = &unit.functions[0];
    let leader = NodeLocation {
        machine: function.machine,
        block: id(1_443, BlockId::new),
        node: 0,
    };
    let redundant = NodeLocation {
        machine: function.machine,
        block: id(1_442, BlockId::new),
        node: 0,
    };
    let (affected, provenance) =
        node_elision_accounting(function, redundant, id(1_449, ValueId::new)).unwrap();
    let forged = PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        affected,
        provenance,
        -1,
        DominatingScalarCommonSubexpressionRewrite {
            leader,
            redundant,
            leader_operation: id(1_452, OperationId::new),
            redundant_operation: id(1_450, OperationId::new),
            leader_result: id(1_448, ValueId::new),
            redundant_result: id(1_449, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        },
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
}
