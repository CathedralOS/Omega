//! Exact signed negative-one right-shift tests.

use super::*;

#[test]
fn proof_certified_exact_signed_negative_one_shift_right_reuses_the_literal() {
    for bits in [1, 8] {
        let integer = IntegerType::new(IntegerSign::Signed, bits).unwrap();
        let unit = live_exact_signed_negative_one_shift_right_unit(integer);
        let contract =
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("a defined signed negative-one right shift proposes once");
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(
            patch.identity,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue
        );
        assert_eq!(patch.replacement, id(324, ValueId::new));
        assert_eq!(candidate.consumed_facts().len(), 2);

        let accepted =
            validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1"
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
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule
                .propose(accepted.unit(), RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn proof_certified_exact_signed_negative_one_shift_right_declines_other_shapes() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let contract =
        LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule::contract();

    let mut non_negative_one = live_exact_signed_negative_one_shift_right_unit(signed);
    let O::IntegerConstant { value, .. } =
        &mut non_negative_one.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Signed(-2);
    let OptimizationFact::IntegerConstant { constant, .. } =
        &mut non_negative_one.functions[0].facts[0]
    else {
        unreachable!()
    };
    *constant = IntegerValue::Signed(-2);
    non_negative_one.identity = recompute_psi_optimization_unit_identity(&non_negative_one);

    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned = live_proof_binary_identity_unit(
        unsigned,
        IntegerValue::Unsigned(u8::MAX.into()),
        true,
        |psi_operation, obligation, result, value_type, value, count| O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type: unsigned,
            value,
            count,
        },
    );
    let left_shift = live_proof_binary_identity_unit(
        signed,
        IntegerValue::Signed(-1),
        true,
        |psi_operation, obligation, result, value_type, value, count| O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type: signed,
            value,
            count,
        },
    );
    let wrapping = live_proof_binary_identity_unit(
        signed,
        IntegerValue::Signed(-1),
        true,
        |psi_operation, _obligation, result, value_type, value, count| {
            O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                count_type: signed,
                value,
                count,
            }
        },
    );
    let dead =
        discard_scalar_function_result(live_exact_signed_negative_one_shift_right_unit(signed));
    let mut missing_proof = live_exact_signed_negative_one_shift_right_unit(signed);
    missing_proof.accepted_obligation_facts.clear();
    missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
    let mut missing_active_reference = live_exact_signed_negative_one_shift_right_unit(signed);
    missing_active_reference.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    missing_active_reference.identity =
        recompute_psi_optimization_unit_identity(&missing_active_reference);

    for unit in [
        non_negative_one,
        unsigned,
        left_shift,
        wrapping,
        dead,
        missing_proof,
        missing_active_reference,
    ] {
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn proof_certified_exact_signed_negative_one_shift_right_rejects_forged_custody() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let unit = live_exact_signed_negative_one_shift_right_unit(integer);
    let contract =
        LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        unreachable!()
    };
    let (constant_fact, obligation_fact) =
        candidate.proof_certified_scalar_identity_witness().unwrap();

    let wrong_identity = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        obligation_fact,
        -1,
        ProofCertifiedScalarIdentityRewrite {
            identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &wrong_identity),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let wrong_replacement = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        obligation_fact,
        -1,
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
        ScalarConstantFactIdentity::from_canonical_bytes(b"foreign negative-one literal"),
        obligation_fact,
        -1,
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
            b"foreign negative-one shift proof",
        ),
        -1,
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
}
