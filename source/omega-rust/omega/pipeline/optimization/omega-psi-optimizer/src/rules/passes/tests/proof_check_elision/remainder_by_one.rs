//! Remainder-by-one tests.

use super::*;

#[test]
fn proof_certified_remainder_by_one_materializes_typed_zero_for_every_policy_and_sign() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for policy in [
            SelfRemainderPolicy::Exact,
            SelfRemainderPolicy::Wrapping,
            SelfRemainderPolicy::Saturating,
        ] {
            let unit = live_remainder_by_one_unit(integer, policy);
            let contract = LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract();
            let original_node = unit.functions[0].blocks[0].nodes[1].clone();
            let accepted_catalog = unit.accepted_obligation_facts.clone();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerRemainderByOneEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("one live remainder-by-one candidate");
            let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.location.machine, id(321, MachineId::new));
            assert_eq!(patch.location.block, id(322, BlockId::new));
            assert_eq!(patch.location.node, 1);
            assert_eq!(patch.source_operation, id(327, OperationId::new));
            assert_eq!(patch.result, id(325, ValueId::new));
            assert_eq!(patch.scalar_type, integer);
            assert_eq!(patch.constant, integer_zero(integer));
            assert_eq!(candidate.predicted_cost_delta(), -1);
            assert!(candidate.substitutions().is_empty());
            assert_eq!(candidate.affected_blocks(), [id(322, BlockId::new)]);
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
                validate_proof_certified_integer_remainder_by_one_candidate(&unit, &candidate)
                    .unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1"
                )
            );
            assert_eq!(accepted.unit().accepted_obligation_facts, accepted_catalog);
            let output_node = &accepted.unit().functions[0].blocks[0].nodes[1];
            assert!(matches!(
                output_node.operation,
                O::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type: ScalarType::Integer(output_type),
                    value,
                } if psi_operation == id(327, OperationId::new)
                    && result == id(325, ValueId::new)
                    && output_type == integer
                    && value == integer_zero(integer)
            ));
            assert_eq!(output_node.provenance, original_node.provenance);
            assert_eq!(output_node.fuel, original_node.fuel);
            assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            }));
            assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::IntegerConstant { value, constant, support }
                        if *value == id(325, ValueId::new)
                            && *constant == integer_zero(integer)
                            && *support == id(327, OperationId::new)
                )
            }));

            let mut manager = crate::AnalysisManager::new(accepted.unit());
            let products = manager
                .require_all(accepted.unit(), contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerRemainderByOneEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn proof_certified_remainder_by_one_declines_closed_shapes_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let contract = LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract();
    let unit = live_remainder_by_one_unit(integer, SelfRemainderPolicy::Exact);
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedIntegerRemainderByOneEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let mut propagated_products = products.clone();
    let AnalysisProduct::ScalarConstants(constants) = propagated_products
        .iter_mut()
        .find(|product| product.kind() == AnalysisKind::ScalarConstants)
        .unwrap()
    else {
        unreachable!()
    };
    constants
        .facts
        .iter_mut()
        .find(|fact| fact.value == id(324, ValueId::new))
        .unwrap()
        .support
        .edges
        .push(id(329, EdgeId::new));
    assert!(
        LiveProofCertifiedIntegerRemainderByOneEliminationRule
            .propose(&unit, RuleAnalysisView::new(&propagated_products))
            .unwrap()
            .is_empty(),
        "a propagated-one fact is not a direct literal witness"
    );
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    let (constant_fact, obligation_fact) =
        candidate.proof_certified_scalar_identity_witness().unwrap();

    for forged_patch in [
        IntegerConstantRewrite {
            constant: IntegerValue::Unsigned(1),
            ..patch
        },
        IntegerConstantRewrite {
            source_operation: id(330, OperationId::new),
            ..patch
        },
        IntegerConstantRewrite {
            result: id(330, ValueId::new),
            ..patch
        },
        IntegerConstantRewrite {
            scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            constant: IntegerValue::Signed(0),
            ..patch
        },
    ] {
        let forged = PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            forged_patch,
        )
        .unwrap();
        assert!(
            validate_proof_certified_integer_remainder_by_one_candidate(&unit, &forged).is_err()
        );
    }

    for (bad_constant, bad_obligation, expected) in [
        (
            ScalarConstantFactIdentity::from_canonical_bytes(b"foreign remainder-one literal"),
            obligation_fact,
            OptimizationUnitValidationError::CandidateOperandFactMismatch,
        ),
        (
            constant_fact,
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign remainder-one proof",
            ),
            OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
        ),
    ] {
        let forged = PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            bad_constant,
            bad_obligation,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_remainder_by_one_candidate(&unit, &forged),
            Err(expected)
        );
    }

    let mut corrupt_provenance = candidate.provenance().to_vec();
    corrupt_provenance[0].fuel[0].units += 1;
    let forged = PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        corrupt_provenance,
        constant_fact,
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_integer_remainder_by_one_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );

    let mut foreign_operation = unit.clone();
    foreign_operation.functions[0].blocks[0].nodes[1].operation = O::ExactIntegerDivide {
        psi_operation: id(327, OperationId::new),
        obligation: id(328, ObligationId::new),
        result: id(325, ValueId::new),
        scalar_type: integer,
        left: id(323, ValueId::new),
        right: id(324, ValueId::new),
    };
    foreign_operation.identity = recompute_psi_optimization_unit_identity(&foreign_operation);
    validate_psi_optimization_unit(&foreign_operation).unwrap();
    let foreign_candidate =
        PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
            foreign_operation.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_proof_certified_integer_remainder_by_one_candidate(
            &foreign_operation,
            &foreign_candidate,
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let mut non_one = unit.clone();
    let O::IntegerConstant { value, .. } = &mut non_one.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(2);
    for fact in &mut non_one.functions[0].facts {
        if let OptimizationFact::IntegerConstant {
            value, constant, ..
        } = fact
            && *value == id(324, ValueId::new)
        {
            *constant = IntegerValue::Unsigned(2);
        }
    }
    non_one.identity = recompute_psi_optimization_unit_identity(&non_one);
    validate_psi_optimization_unit(&non_one).unwrap();

    let mut missing_reference = unit.clone();
    missing_reference.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    missing_reference.identity = recompute_psi_optimization_unit_identity(&missing_reference);
    let mut missing_catalog = unit.clone();
    missing_catalog.accepted_obligation_facts.clear();
    missing_catalog.identity = recompute_psi_optimization_unit_identity(&missing_catalog);
    for ineligible in [
        non_one,
        missing_reference,
        missing_catalog,
        discard_scalar_function_result(unit.clone()),
        live_self_remainder_unit(integer, SelfRemainderPolicy::Exact),
    ] {
        let mut manager = crate::AnalysisManager::new(&ineligible);
        let products = manager
            .require_all(&ineligible, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerRemainderByOneEliminationRule
                .propose(&ineligible, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}
