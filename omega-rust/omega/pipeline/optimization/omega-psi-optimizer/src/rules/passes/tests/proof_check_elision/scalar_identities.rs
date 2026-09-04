//! General proof-certified scalar identity tests.

use super::*;

#[test]
fn live_proof_certified_identity_elision_substitutes_and_replays_exact_custody() {
    let unit = live_exact_add_zero_unit();
    let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedIntegerIdentityEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("x + 0 has one canonical proof-certified identity rewrite");
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        unreachable!()
    };
    assert_eq!(
        patch.identity,
        ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight
    );
    assert_eq!(patch.replacement, id(303, ValueId::new));
    assert_eq!(candidate.consumed_facts().len(), 2);
    assert!(candidate.consumed_facts().iter().any(|fact| matches!(
        fact,
        omega_optimization_core::OptimizationFactReference::AcceptedObligation(_)
    )));
    assert!(candidate.consumed_facts().iter().any(|fact| matches!(
        fact,
        omega_optimization_core::OptimizationFactReference::ScalarConstant(_)
    )));

    let accepted = validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.live-proof-certified-integer-identity-elimination.v1"
        )
    );
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(308, OperationId::new))
    }));
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        O::Return { value, .. } if value == id(303, ValueId::new)
    ));
    assert!(
        accepted
            .provenance()
            .iter()
            .all(|row| row.disposition.is_realized())
    );

    let mut manager = crate::AnalysisManager::new(accepted.unit());
    let products = manager
        .require_all(accepted.unit(), contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerIdentityEliminationRule
            .propose(accepted.unit(), RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

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
            identity: ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &forged_kind),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
    let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        ScalarConstantFactIdentity::from_canonical_bytes(b"foreign identity constant"),
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    );
}

#[test]
fn live_proof_certified_identity_vocabulary_is_closed_and_canonical() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let left = id(303, ValueId::new);
    let right = id(304, ValueId::new);
    let result = id(305, ValueId::new);
    let source = id(308, OperationId::new);
    let obligation = id(309, ObligationId::new);
    let rows = [
        (
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(7),
            O::ExactIntegerAdd {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerAdd {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerSubtract {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
        ),
        (
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            O::ExactIntegerMultiply {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(1),
            O::ExactIntegerMultiply {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerShiftLeft {
                psi_operation: source,
                obligation,
                result,
                value_type: integer,
                count_type: integer,
                value: left,
                count: right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerShiftRight {
                psi_operation: source,
                obligation,
                result,
                value_type: integer,
                count_type: integer,
                value: left,
                count: right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
        ),
    ];
    for (left_constant, right_constant, operation, expected) in rows {
        let mut unit = exact_add_unit();
        for (index, constant) in [left_constant, right_constant].into_iter().enumerate() {
            let O::IntegerConstant { value, .. } =
                &mut unit.functions[0].blocks[0].nodes[index].operation
            else {
                unreachable!()
            };
            *value = constant;
            let defined = [left, right][index];
            for fact in &mut unit.functions[0].facts {
                if let OptimizationFact::IntegerConstant {
                    value,
                    constant: row,
                    ..
                } = fact
                    && *value == defined
                {
                    *row = constant;
                }
            }
        }
        unit.functions[0].blocks[0].nodes[2].operation = operation;
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        validate_psi_optimization_unit(&unit).unwrap();
        let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerIdentityEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.identity, expected);
        validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
    }

    let mut missing_proof = live_exact_add_zero_unit();
    missing_proof.accepted_obligation_facts.clear();
    missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
    let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&missing_proof);
    let products = manager
        .require_all(&missing_proof, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerIdentityEliminationRule
            .propose(&missing_proof, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}
