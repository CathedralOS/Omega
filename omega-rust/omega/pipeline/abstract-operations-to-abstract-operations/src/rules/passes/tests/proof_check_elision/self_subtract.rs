//! Exact self-subtraction tests.

use super::*;

#[test]
fn proof_certified_exact_self_subtract_materializes_typed_zero_with_exact_custody() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        let unit = live_exact_self_subtract_unit(integer);
        let contract = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract();
        let original_node = unit.functions[0].blocks[0].nodes[0].clone();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("live exact self-subtract has one candidate");
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        assert_eq!(patch.location.node, 0);
        assert_eq!(patch.source_operation, id(335, OperationId::new));
        assert_eq!(patch.result, id(334, ValueId::new));
        assert_eq!(patch.scalar_type, integer);
        assert_eq!(patch.constant, integer_zero(integer));
        assert!(candidate.substitutions().is_empty());
        assert_eq!(candidate.affected_blocks(), [id(332, BlockId::new)]);
        assert_eq!(candidate.consumed_facts().len(), 1);
        assert!(matches!(
            candidate.consumed_facts()[0],
            OptimizationFactReference::AcceptedObligation(_)
        ));

        let accepted =
            validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &candidate)
                .unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1"
            )
        );
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        let output_node = &accepted.unit().functions[0].blocks[0].nodes[0];
        assert!(matches!(
            output_node.operation,
            O::IntegerConstant {
                psi_operation,
                result,
                scalar_type: ScalarType::Integer(output_type),
                value,
            } if psi_operation == id(335, OperationId::new)
                && result == id(334, ValueId::new)
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
                    if *value == id(334, ValueId::new)
                        && *constant == integer_zero(integer)
                        && *support == id(335, OperationId::new)
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
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                .propose(accepted.unit(), RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn proof_certified_exact_self_subtract_declines_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = live_exact_self_subtract_unit(integer);
    let contract = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    let obligation_fact = candidate.accepted_obligation_witness().unwrap();

    for forged_patch in [
        IntegerConstantRewrite {
            constant: IntegerValue::Unsigned(1),
            ..patch
        },
        IntegerConstantRewrite {
            source_operation: id(338, OperationId::new),
            ..patch
        },
        IntegerConstantRewrite {
            result: id(339, ValueId::new),
            ..patch
        },
        IntegerConstantRewrite {
            scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            constant: IntegerValue::Signed(0),
            ..patch
        },
    ] {
        let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            forged_patch,
        )
        .unwrap();
        assert!(
            validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &forged).is_err()
        );
    }
    let foreign_fact = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign self-subtract proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &foreign_fact),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
    let mut corrupt_provenance = candidate.provenance().to_vec();
    corrupt_provenance[0].fuel[0].units += 1;
    let corrupt_provenance = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        corrupt_provenance,
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &corrupt_provenance,),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
    let substituted = PsiRewriteCandidate::new_integer_evaluation(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        vec![ScalarSubstitution {
            from: patch.result,
            to: id(333, ValueId::new),
            scalar_type: ScalarType::Integer(integer),
        }],
        candidate.provenance().to_vec(),
        IntegerEvaluationWitness::ProofCertifiedUnary {
            operand_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                b"foreign self-subtract operand",
            ),
            obligation_fact,
        },
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &substituted),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );
    let foreign_contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"foreign self-subtract rule"),
        OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
        1,
        contract.required_analyses(),
        contract.invalidated_analyses(),
        OptimizationSafetyClass::ProofCertified,
    )
    .unwrap();
    let foreign_rule = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        foreign_contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &foreign_rule),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );
    let terminal_location = NodeLocation {
        machine: patch.location.machine,
        block: patch.location.block,
        node: 1,
    };
    let terminal = &unit.functions[0].blocks[0].nodes[1];
    let terminal_site = PsiRealizationSite::Node(terminal_location);
    let wrong_location = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        vec![ProvenanceRewrite {
            input: terminal_site,
            disposition: ProvenanceDisposition::RealizedAt(terminal_site),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        }],
        obligation_fact,
        candidate.predicted_cost_delta(),
        IntegerConstantRewrite {
            location: terminal_location,
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &wrong_location),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let mut missing_catalog = unit.clone();
    missing_catalog.accepted_obligation_facts.clear();
    missing_catalog.identity = recompute_psi_optimization_unit_identity(&missing_catalog);
    let missing_catalog_candidate =
        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            missing_catalog.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(
            &missing_catalog,
            &missing_catalog_candidate,
        ),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut wrong_policy = unit.clone();
    wrong_policy.functions[0].blocks[0].nodes[0].operation = O::WrappingIntegerSubtract {
        psi_operation: patch.source_operation,
        result: patch.result,
        scalar_type: integer,
        left: id(333, ValueId::new),
        right: id(333, ValueId::new),
    };
    wrong_policy.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    wrong_policy.identity = recompute_psi_optimization_unit_identity(&wrong_policy);
    validate_psi_optimization_unit(&wrong_policy).unwrap();
    let wrong_policy_candidate =
        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            wrong_policy.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(
            &wrong_policy,
            &wrong_policy_candidate,
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let unequal = live_proof_binary_identity_unit(
        integer,
        IntegerValue::Unsigned(1),
        false,
        |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    );
    for ineligible in [
        unequal,
        live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(1),
            false,
            |psi_operation, _obligation, result, scalar_type, left, right| {
                O::WrappingIntegerSubtract {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        ),
        live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(1),
            false,
            |psi_operation, _obligation, result, scalar_type, left, right| {
                O::SaturatingIntegerSubtract {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        ),
        discard_scalar_function_result(live_exact_self_subtract_unit(integer)),
    ] {
        let mut manager = crate::AnalysisManager::new(&ineligible);
        let products = manager
            .require_all(&ineligible, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                .propose(&ineligible, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    for mut missing in [
        live_exact_self_subtract_unit(integer),
        live_exact_self_subtract_unit(integer),
    ]
    .into_iter()
    .enumerate()
    {
        if missing.0 == 0 {
            missing.1.accepted_obligation_facts.clear();
        } else {
            missing.1.functions[0].facts.retain(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            });
        }
        missing.1.identity = recompute_psi_optimization_unit_identity(&missing.1);
        let mut manager = crate::AnalysisManager::new(&missing.1);
        let products = manager
            .require_all(&missing.1, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                .propose(&missing.1, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}
