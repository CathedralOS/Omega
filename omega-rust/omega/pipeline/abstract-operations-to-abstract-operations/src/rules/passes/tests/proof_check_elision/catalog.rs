//! Catalog-level coverage for proof check elision.

use super::*;

#[test]
fn proof_check_elision_binds_accepted_evidence_and_retains_its_catalog() {
    let unit = dead_exact_add_unit();
    validate_psi_optimization_unit(&unit).unwrap();
    let contract = ProofCertifiedDeadScalarEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = ProofCertifiedDeadScalarEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("the unused proof-certified exact add is the sole candidate");
    assert_eq!(candidate.node_decision_point().unwrap().node, 2);
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(unit.accepted_obligation_facts[0].identity)
    );
    assert_eq!(
        candidate.consumed_facts(),
        [
            optimization_core::OptimizationFactReference::AcceptedObligation(
                unit.accepted_obligation_facts[0].identity,
            )
        ]
    );
    let accepted = validate_dead_scalar_node_candidate(&unit, &candidate).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 3);
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(
        accepted.unit().functions[0]
            .facts
            .iter()
            .all(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }))
    );

    let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
        unreachable!()
    };
    let forged = PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign accepted obligation",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_dead_scalar_node_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut bare = unit.clone();
    bare.accepted_obligation_facts.clear();
    bare.identity = recompute_psi_optimization_unit_identity(&bare);
    let liveness = compute_analysis(&bare, AnalysisKind::ValueLiveness).unwrap();
    let effects = compute_analysis(&bare, AnalysisKind::EffectSummaries).unwrap();
    assert!(matches!(
        ProofCertifiedDeadScalarEliminationRule
            .propose(&bare, RuleAnalysisView::new(&[liveness, effects])),
        Err(RuleProposalError::MissingAcceptedObligation { .. })
    ));
}

#[test]
fn proof_check_elision_covers_the_closed_proof_bearing_scalar_vocabulary() {
    let seed = dead_exact_add_unit();
    let O::ExactIntegerAdd {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    } = seed.functions[0].blocks[0].nodes[2].operation
    else {
        unreachable!()
    };
    let operations = vec![
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
    ];
    for operation in operations {
        let mut unit = seed.clone();
        unit.functions[0].blocks[0].nodes[2].operation = operation;
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        validate_psi_optimization_unit(&unit).unwrap();
        let liveness = compute_analysis(&unit, AnalysisKind::ValueLiveness).unwrap();
        let effects = compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap();
        let [candidate] = ProofCertifiedDeadScalarEliminationRule
            .propose(&unit, RuleAnalysisView::new(&[liveness, effects]))
            .unwrap()
            .try_into()
            .expect("each exact binary proof shape proposes once");
        validate_dead_scalar_node_candidate(&unit, &candidate).unwrap();
    }

    let cast = discard_scalar_function_result(exact_cast_unit(7));
    validate_psi_optimization_unit(&cast).unwrap();
    let liveness = compute_analysis(&cast, AnalysisKind::ValueLiveness).unwrap();
    let effects = compute_analysis(&cast, AnalysisKind::EffectSummaries).unwrap();
    let [candidate] = ProofCertifiedDeadScalarEliminationRule
        .propose(&cast, RuleAnalysisView::new(&[liveness, effects]))
        .unwrap()
        .try_into()
        .expect("the exact cast proposes once");
    let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
        unreachable!()
    };
    assert_eq!(
        patch.scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
    );
    validate_dead_scalar_node_candidate(&cast, &candidate).unwrap();
}
