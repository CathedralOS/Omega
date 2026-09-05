//! Division-by-one tests.

use super::*;

#[test]
fn proof_certified_divide_by_one_covers_every_policy_and_integer_sign() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for (policy, expected) in [
            (
                0,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
            ),
            (
                1,
                ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
            ),
            (
                2,
                ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
            ),
        ] {
            let unit = live_divide_by_one_unit(
                integer,
                |psi_operation, obligation, result, scalar_type, left, right| match policy {
                    0 => O::ExactIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    1 => O::WrappingIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    2 => O::SaturatingIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    _ => unreachable!(),
                },
            );
            let contract = LiveProofCertifiedIntegerDivideByOneEliminationRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerDivideByOneEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("each typed divide by literal one has one candidate");
            let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.identity, expected);
            assert_eq!(patch.replacement, id(323, ValueId::new));
            assert_eq!(candidate.consumed_facts().len(), 2);
            let accepted =
                validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1"
                )
            );
            assert_eq!(
                accepted.unit().accepted_obligation_facts,
                unit.accepted_obligation_facts
            );
            assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            }));
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[1].operation,
                O::Return { value, .. } if value == id(323, ValueId::new)
            ));
            assert_eq!(
                accepted.unit().functions[0].blocks[0].nodes[1]
                    .provenance
                    .len(),
                2
            );
            assert_eq!(
                accepted.unit().functions[0].blocks[0].nodes[1].fuel.len(),
                2
            );

            let mut manager = crate::AnalysisManager::new(accepted.unit());
            let products = manager
                .require_all(accepted.unit(), contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerDivideByOneEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn proof_certified_divide_by_one_declines_missing_evidence_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let make_exact =
        |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        };
    let unit = live_divide_by_one_unit(integer, make_exact);
    let contract = LiveProofCertifiedIntegerDivideByOneEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedIntegerDivideByOneEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        unreachable!()
    };
    let (constant_fact, obligation_fact) =
        candidate.proof_certified_scalar_identity_witness().unwrap();

    let forged_kind = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        obligation_fact,
        candidate.predicted_cost_delta(),
        ProofCertifiedScalarIdentityRewrite {
            identity: ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &forged_kind),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
    let forged_old_family = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        obligation_fact,
        candidate.predicted_cost_delta(),
        ProofCertifiedScalarIdentityRewrite {
            identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &forged_old_family),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
    let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        ScalarConstantFactIdentity::from_canonical_bytes(b"foreign divide-one literal"),
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    );
    let foreign_obligation = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign divide-one proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut wrong_literal = live_divide_by_one_unit(integer, make_exact);
    let O::IntegerConstant { value, .. } =
        &mut wrong_literal.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(0);
    let OptimizationFact::IntegerConstant { constant, .. } =
        &mut wrong_literal.functions[0].facts[0]
    else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(0);
    wrong_literal.identity = recompute_psi_optimization_unit_identity(&wrong_literal);
    validate_psi_optimization_unit(&wrong_literal).unwrap();
    let mut manager = crate::AnalysisManager::new(&wrong_literal);
    let products = manager
        .require_all(&wrong_literal, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerDivideByOneEliminationRule
            .propose(&wrong_literal, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut missing_proof = live_divide_by_one_unit(integer, make_exact);
    missing_proof.accepted_obligation_facts.clear();
    missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
    let mut manager = crate::AnalysisManager::new(&missing_proof);
    let products = manager
        .require_all(&missing_proof, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerDivideByOneEliminationRule
            .propose(&missing_proof, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}
