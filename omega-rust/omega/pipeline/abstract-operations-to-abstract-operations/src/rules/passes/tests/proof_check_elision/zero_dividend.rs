//! Zero-dividend tests.

use super::*;

#[test]
fn proof_certified_zero_dividend_covers_divide_remainder_policies_and_signs() {
    let rows = [
        ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
    ];
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for (policy, expected) in rows.into_iter().enumerate() {
            let unit = live_zero_dividend_unit(
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
                    3 => O::ExactIntegerRemainder {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    4 => O::WrappingIntegerRemainder {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    5 => O::SaturatingIntegerRemainder {
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
            let contract = LiveProofCertifiedIntegerZeroDividendEliminationRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerZeroDividendEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("each proof-certified zero dividend has one candidate");
            let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.identity, expected);
            assert_eq!(patch.replacement, id(324, ValueId::new));
            assert_eq!(candidate.consumed_facts().len(), 2);
            assert!(
                candidate
                    .consumed_facts()
                    .iter()
                    .any(|fact| matches!(fact, OptimizationFactReference::ScalarConstant(_)))
            );
            assert!(
                candidate
                    .consumed_facts()
                    .iter()
                    .any(|fact| matches!(fact, OptimizationFactReference::AcceptedObligation(_)))
            );

            let accepted =
                validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1"
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
                O::Return { value, .. } if value == id(324, ValueId::new)
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
                LiveProofCertifiedIntegerZeroDividendEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn proof_certified_zero_dividend_declines_ineligible_shapes_and_rejects_corruption() {
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
    let unit = live_zero_dividend_unit(integer, make_exact);
    let contract = LiveProofCertifiedIntegerZeroDividendEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedIntegerZeroDividendEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        unreachable!()
    };
    let (constant_fact, obligation_fact) =
        candidate.proof_certified_scalar_identity_witness().unwrap();

    for identity in [
        ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
    ] {
        let forged = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite { identity, ..patch },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
    }
    let wrong_replacement = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        obligation_fact,
        candidate.predicted_cost_delta(),
        ProofCertifiedScalarIdentityRewrite {
            replacement: id(323, ValueId::new),
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &wrong_replacement),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
    let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        ScalarConstantFactIdentity::from_canonical_bytes(b"foreign zero-dividend literal"),
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
            b"foreign zero-dividend proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut nonzero = live_zero_dividend_unit(integer, make_exact);
    let O::IntegerConstant { value, .. } = &mut nonzero.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(1);
    let OptimizationFact::IntegerConstant { constant, .. } = &mut nonzero.functions[0].facts[0]
    else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(1);
    nonzero.identity = recompute_psi_optimization_unit_identity(&nonzero);
    let mut manager = crate::AnalysisManager::new(&nonzero);
    let products = manager
        .require_all(&nonzero, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerZeroDividendEliminationRule
            .propose(&nonzero, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut missing_proof = live_zero_dividend_unit(integer, make_exact);
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
        LiveProofCertifiedIntegerZeroDividendEliminationRule
            .propose(&missing_proof, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut missing_active_reference = live_zero_dividend_unit(integer, make_exact);
    missing_active_reference.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    missing_active_reference.identity =
        recompute_psi_optimization_unit_identity(&missing_active_reference);
    let mut manager = crate::AnalysisManager::new(&missing_active_reference);
    let products = manager
        .require_all(&missing_active_reference, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerZeroDividendEliminationRule
            .propose(&missing_active_reference, RuleAnalysisView::new(&products),)
            .unwrap()
            .is_empty()
    );

    let dead = discard_scalar_function_result(live_zero_dividend_unit(integer, make_exact));
    let mut manager = crate::AnalysisManager::new(&dead);
    let products = manager
        .require_all(&dead, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerZeroDividendEliminationRule
            .propose(&dead, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let right_zero =
        live_proof_binary_identity_unit(integer, IntegerValue::Unsigned(0), false, make_exact);
    let mut manager = crate::AnalysisManager::new(&right_zero);
    let products = manager
        .require_all(&right_zero, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerZeroDividendEliminationRule
            .propose(&right_zero, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}
