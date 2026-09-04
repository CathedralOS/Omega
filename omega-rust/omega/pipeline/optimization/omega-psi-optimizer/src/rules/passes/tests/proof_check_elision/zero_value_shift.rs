//! Exact zero-value shift tests.

use super::*;

#[test]
fn proof_certified_exact_zero_value_shift_covers_directions_and_integer_signs() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for (left_shift, expected) in [
            (
                true,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
            ),
            (
                false,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
            ),
        ] {
            let unit = live_exact_zero_value_shift_unit(integer, left_shift);
            let contract = LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("each live proof-certified zero-value shift has one candidate");
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
                        b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1"
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
                LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn proof_certified_exact_zero_value_shift_declines_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = live_exact_zero_value_shift_unit(integer, true);
    let contract = LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
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
        ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
        ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
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
        ScalarConstantFactIdentity::from_canonical_bytes(b"foreign zero-shift literal"),
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
        omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign zero-shift proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
    let mut corrupted_provenance = candidate.provenance().to_vec();
    corrupted_provenance[0].fuel[0].units += 1;
    let corrupted_provenance = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        corrupted_provenance,
        constant_fact,
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &corrupted_provenance),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );

    let mut nonzero = live_exact_zero_value_shift_unit(integer, true);
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
        LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
            .propose(&nonzero, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut missing_proof = live_exact_zero_value_shift_unit(integer, true);
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
        LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
            .propose(&missing_proof, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut missing_active_reference = live_exact_zero_value_shift_unit(integer, true);
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
        LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
            .propose(&missing_active_reference, RuleAnalysisView::new(&products),)
            .unwrap()
            .is_empty()
    );

    let dead = discard_scalar_function_result(live_exact_zero_value_shift_unit(integer, true));
    let mut manager = crate::AnalysisManager::new(&dead);
    let products = manager
        .require_all(&dead, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
            .propose(&dead, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let zero_count_only = live_proof_binary_identity_unit(
        integer,
        IntegerValue::Unsigned(0),
        false,
        |psi_operation, obligation, result, value_type, value, count| O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type: integer,
            value,
            count,
        },
    );
    let mut manager = crate::AnalysisManager::new(&zero_count_only);
    let products = manager
        .require_all(&zero_count_only, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
            .propose(&zero_count_only, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let wrapping = live_proof_binary_identity_unit(
        integer,
        IntegerValue::Unsigned(0),
        true,
        |psi_operation, _obligation, result, value_type, value, count| {
            O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                count_type: integer,
                value,
                count,
            }
        },
    );
    let mut manager = crate::AnalysisManager::new(&wrapping);
    let products = manager
        .require_all(&wrapping, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
            .propose(&wrapping, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}
