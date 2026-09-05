//! Same-block leader choice and custody.

use super::*;

#[test]
fn same_block_cse_uses_earliest_typed_leader_and_moves_custody_forward() {
    let unit = local_cse_unit();
    let contract = SameBlockTotalScalarCseRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = SameBlockTotalScalarCseRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("swapped commutative operands have one exact CSE candidate");
    assert!(matches!(
        candidate.patch(),
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_)
    ));
    assert_eq!(
        candidate.substitutions(),
        [ScalarSubstitution {
            from: id(1_306, ValueId::new),
            to: id(1_305, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
        }]
    );
    let accepted = validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    let nodes = &output.functions[0].blocks[0].nodes;
    assert_eq!(nodes.len(), 3);
    assert!(
        matches!(nodes[1].operation, O::IntegerEqual { left, right, .. } if left == id(1_305, ValueId::new) && right == left)
    );
    assert_eq!(
        nodes[1].provenance,
        [
            PsiProvenance::Operation(id(1_310, OperationId::new)),
            PsiProvenance::Operation(id(1_309, OperationId::new))
        ]
    );
    assert_eq!(accepted.provenance().len(), 3);
    assert!(
        output.functions[0].blocks[0]
            .nodes
            .iter()
            .flat_map(|node| &node.uses)
            .all(|row| row.value != id(1_306, ValueId::new))
    );

    let mut manager = crate::AnalysisManager::new(output);
    let products = manager
        .require_all(output, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockTotalScalarCseRule
            .propose(output, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch() else {
        unreachable!()
    };
    let mut provenance = candidate.provenance().to_vec();
    provenance[0].disposition =
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(patch.leader));
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let forged = PsiRewriteCandidate::new_local_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        provenance,
        -1,
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
}

#[test]
fn proof_certified_same_block_cse_consumes_the_redundant_operations_fact() {
    let unit = proof_certified_local_cse_unit();
    let ordinary_contract = SameBlockTotalScalarCseRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, ordinary_contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockTotalScalarCseRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let contract = SameBlockProofCertifiedScalarCseRule::contract();
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
    let [candidate] = SameBlockProofCertifiedScalarCseRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("swapped exact-add operands produce one proof-certified CSE candidate");
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == id(1_309, OperationId::new))
        .expect("fixture retains the redundant operation fact")
        .identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    assert_eq!(
        candidate.consumed_facts(),
        [optimization_core::OptimizationFactReference::AcceptedObligation(redundant_fact,)]
    );
    let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch() else {
        unreachable!()
    };
    assert_eq!(patch.leader_operation, id(1_308, OperationId::new));
    assert_eq!(patch.redundant_operation, id(1_309, OperationId::new));
    let accepted = validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 3);
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
        matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_308, OperationId::new)
        )
    }));
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_309, OperationId::new)
        )
    }));

    let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign proof-certified local CSE fact",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut missing_leader = unit.clone();
    missing_leader
        .accepted_obligation_facts
        .retain(|fact| fact.operation != id(1_308, OperationId::new));
    missing_leader.identity = recompute_psi_optimization_unit_identity(&missing_leader);
    let uses = compute_analysis(&missing_leader, AnalysisKind::UseDefinition).unwrap();
    let effects = compute_analysis(&missing_leader, AnalysisKind::EffectSummaries).unwrap();
    assert!(
        SameBlockProofCertifiedScalarCseRule
            .propose(&missing_leader, RuleAnalysisView::new(&[uses, effects]))
            .unwrap()
            .is_empty()
    );
    let forged_without_leader_fact =
        PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
            missing_leader.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            redundant_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(
            &missing_leader,
            &forged_without_leader_fact,
        ),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut missing_redundant = unit.clone();
    missing_redundant
        .accepted_obligation_facts
        .retain(|fact| fact.operation != id(1_309, OperationId::new));
    missing_redundant.identity = recompute_psi_optimization_unit_identity(&missing_redundant);
    let uses = compute_analysis(&missing_redundant, AnalysisKind::UseDefinition).unwrap();
    let effects = compute_analysis(&missing_redundant, AnalysisKind::EffectSummaries).unwrap();
    assert!(
        SameBlockProofCertifiedScalarCseRule
            .propose(&missing_redundant, RuleAnalysisView::new(&[uses, effects]))
            .unwrap()
            .is_empty()
    );
}
