//! Self-remainder tests.

use super::*;

#[test]
fn proof_certified_self_remainder_materializes_typed_zero_for_every_policy_and_sign() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for policy in [
            SelfRemainderPolicy::Exact,
            SelfRemainderPolicy::Wrapping,
            SelfRemainderPolicy::Saturating,
        ] {
            let unit = live_self_remainder_unit(integer, policy);
            let contract = LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract();
            let original_node = unit.functions[0].blocks[0].nodes[0].clone();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerSelfRemainderEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("one live same-operand remainder candidate");
            let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(
                patch.location,
                NodeLocation {
                    machine: id(341, MachineId::new),
                    block: id(342, BlockId::new),
                    node: 0,
                }
            );
            assert_eq!(patch.source_operation, id(345, OperationId::new));
            assert_eq!(patch.result, id(344, ValueId::new));
            assert_eq!(patch.scalar_type, integer);
            assert_eq!(patch.constant, integer_zero(integer));
            assert_eq!(candidate.predicted_cost_delta(), -1);
            assert!(candidate.substitutions().is_empty());
            assert_eq!(candidate.affected_blocks(), [id(342, BlockId::new)]);
            assert_eq!(candidate.consumed_facts().len(), 1);
            assert!(matches!(
                candidate.consumed_facts()[0],
                OptimizationFactReference::AcceptedObligation(_)
            ));

            let accepted =
                validate_proof_certified_integer_self_remainder_candidate(&unit, &candidate)
                    .unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1"
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
                } if psi_operation == id(345, OperationId::new)
                    && result == id(344, ValueId::new)
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
                        if *value == id(344, ValueId::new)
                            && *constant == integer_zero(integer)
                            && *support == id(345, OperationId::new)
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
                LiveProofCertifiedIntegerSelfRemainderEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn proof_certified_self_remainder_declines_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = live_self_remainder_unit(integer, SelfRemainderPolicy::Exact);
    let contract = LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedIntegerSelfRemainderEliminationRule
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
            source_operation: id(348, OperationId::new),
            ..patch
        },
        IntegerConstantRewrite {
            result: id(349, ValueId::new),
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
        assert!(validate_proof_certified_integer_self_remainder_candidate(&unit, &forged).is_err());
    }

    let foreign_fact = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign self-remainder proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_integer_self_remainder_candidate(&unit, &foreign_fact),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    for corrupt_provenance in [
        {
            let mut rows = candidate.provenance().to_vec();
            rows[0].fuel[0].units += 1;
            rows
        },
        {
            let mut rows = candidate.provenance().to_vec();
            rows[0].sources[0] = PsiProvenance::Operation(id(348, OperationId::new));
            rows[0].fuel[0].site = rows[0].sources[0];
            rows
        },
    ] {
        let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
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
            validate_proof_certified_integer_self_remainder_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
    }

    let foreign_contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"foreign self-remainder rule"),
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
        validate_proof_certified_integer_self_remainder_candidate(&unit, &foreign_rule),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );

    let terminal_location = NodeLocation {
        machine: patch.location.machine,
        block: patch.location.block,
        node: 1,
    };
    let terminal = &unit.functions[0].blocks[0].nodes[1];
    let terminal_site = PsiRealizationSite::Node(terminal_location);
    assert!(
        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            vec![id(350, BlockId::new)],
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .is_err()
    );
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
        validate_proof_certified_integer_self_remainder_candidate(&unit, &wrong_location),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let substituted = PsiRewriteCandidate::new_integer_evaluation(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        vec![ScalarSubstitution {
            from: patch.result,
            to: id(343, ValueId::new),
            scalar_type: ScalarType::Integer(integer),
        }],
        candidate.provenance().to_vec(),
        IntegerEvaluationWitness::ProofCertifiedUnary {
            operand_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                b"foreign self-remainder operand",
            ),
            obligation_fact,
        },
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_integer_self_remainder_candidate(&unit, &substituted),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
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
        validate_proof_certified_integer_self_remainder_candidate(
            &missing_catalog,
            &missing_catalog_candidate,
        ),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let unequal = live_proof_binary_identity_unit(
        integer,
        IntegerValue::Unsigned(1),
        false,
        |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    );
    let mut foreign_operation = unit.clone();
    foreign_operation.functions[0].blocks[0].nodes[0].operation = O::ExactIntegerDivide {
        psi_operation: patch.source_operation,
        obligation: id(346, ObligationId::new),
        result: patch.result,
        scalar_type: integer,
        left: id(343, ValueId::new),
        right: id(343, ValueId::new),
    };
    foreign_operation.identity = recompute_psi_optimization_unit_identity(&foreign_operation);
    validate_psi_optimization_unit(&foreign_operation).unwrap();
    let foreign_operation_candidate =
        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            foreign_operation.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_proof_certified_integer_self_remainder_candidate(
            &foreign_operation,
            &foreign_operation_candidate,
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    for ineligible in [
        unequal,
        discard_scalar_function_result(live_self_remainder_unit(
            integer,
            SelfRemainderPolicy::Exact,
        )),
    ] {
        let mut manager = crate::AnalysisManager::new(&ineligible);
        let products = manager
            .require_all(&ineligible, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerSelfRemainderEliminationRule
                .propose(&ineligible, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    for (remove_catalog, mut missing) in [
        (
            true,
            live_self_remainder_unit(integer, SelfRemainderPolicy::Exact),
        ),
        (
            false,
            live_self_remainder_unit(integer, SelfRemainderPolicy::Exact),
        ),
    ] {
        if remove_catalog {
            missing.accepted_obligation_facts.clear();
        } else {
            missing.functions[0].facts.retain(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            });
        }
        missing.identity = recompute_psi_optimization_unit_identity(&missing);
        let mut manager = crate::AnalysisManager::new(&missing);
        let products = manager
            .require_all(&missing, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerSelfRemainderEliminationRule
                .propose(&missing, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}
