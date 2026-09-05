//! Exact multiplication-by-zero tests.

use super::*;

#[test]
fn proof_certified_exact_multiply_by_zero_covers_both_sides_and_integer_signs() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for (zero_left, expected) in [
            (
                true,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
            ),
            (
                false,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
            ),
        ] {
            let unit = live_exact_multiply_by_zero_unit(integer, zero_left);
            let contract = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("each exact multiplication with one literal zero has one candidate");
            let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.identity, expected);
            assert_eq!(patch.replacement, id(324, ValueId::new));
            assert_eq!(candidate.consumed_facts().len(), 2);
            let accepted =
                validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
            assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1"
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
                LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn proof_certified_exact_zero_product_is_canonical_and_closed() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let contract = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract();
    let mut both_zero = exact_add_unit();
    for node in &mut both_zero.functions[0].blocks[0].nodes[..2] {
        let O::IntegerConstant { value, .. } = &mut node.operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0);
    }
    for fact in &mut both_zero.functions[0].facts {
        if let OptimizationFact::IntegerConstant { constant, .. } = fact {
            *constant = IntegerValue::Unsigned(0);
        }
    }
    both_zero.functions[0].blocks[0].nodes[2].operation = O::ExactIntegerMultiply {
        psi_operation: id(308, OperationId::new),
        obligation: id(309, ObligationId::new),
        result: id(305, ValueId::new),
        scalar_type: integer,
        left: id(303, ValueId::new),
        right: id(304, ValueId::new),
    };
    both_zero.identity = recompute_psi_optimization_unit_identity(&both_zero);
    validate_psi_optimization_unit(&both_zero).unwrap();
    let mut manager = crate::AnalysisManager::new(&both_zero);
    let products = manager
        .require_all(&both_zero, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
        .propose(&both_zero, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("zero times zero has one canonical candidate");
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        unreachable!()
    };
    assert_eq!(
        patch.identity,
        ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft
    );
    assert_eq!(patch.replacement, id(303, ValueId::new));
    validate_proof_certified_scalar_identity_candidate(&both_zero, &candidate).unwrap();

    for wrapping in [true, false] {
        let unit = live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(0),
            false,
            |psi_operation, _obligation, result, scalar_type, left, right| {
                if wrapping {
                    O::WrappingIntegerMultiply {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                } else {
                    O::SaturatingIntegerMultiply {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                }
            },
        );
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn proof_certified_exact_multiply_by_zero_declines_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = live_exact_multiply_by_zero_unit(integer, true);
    let contract = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
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
        ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
        ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
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
        ScalarConstantFactIdentity::from_canonical_bytes(b"foreign zero-product literal"),
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
            b"foreign zero-product proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let nonzero = live_divide_by_one_unit(
        integer,
        |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    );
    let mut manager = crate::AnalysisManager::new(&nonzero);
    let products = manager
        .require_all(&nonzero, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&nonzero, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut missing_proof = live_exact_multiply_by_zero_unit(integer, false);
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
        LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&missing_proof, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut unused = live_exact_multiply_by_zero_unit(integer, false);
    let O::Return { value, .. } = &mut unused.functions[0].blocks[0].nodes[2].operation else {
        unreachable!()
    };
    *value = id(324, ValueId::new);
    unused.functions[0].blocks[0].nodes[2].uses[0].value = id(324, ValueId::new);
    unused.identity = recompute_psi_optimization_unit_identity(&unused);
    validate_psi_optimization_unit(&unused).unwrap();
    let mut manager = crate::AnalysisManager::new(&unused);
    let products = manager
        .require_all(&unused, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&unused, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut detached = unit.clone();
    detached.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    detached.identity = recompute_psi_optimization_unit_identity(&detached);
    assert!(matches!(
        validate_psi_optimization_unit(&detached),
        Err(OptimizationUnitValidationError::FactIndexMismatch(_))
    ));
}
