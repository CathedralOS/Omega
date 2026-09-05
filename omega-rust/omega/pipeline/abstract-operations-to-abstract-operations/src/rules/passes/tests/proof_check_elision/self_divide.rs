//! Self-division tests.

use super::*;

#[test]
fn proof_certified_self_divide_materializes_typed_one_for_every_policy_and_sign() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for policy in [
            SelfDividePolicy::Exact,
            SelfDividePolicy::Wrapping,
            SelfDividePolicy::Saturating,
        ] {
            let unit = live_self_divide_unit(integer, policy);
            let contract = LiveProofCertifiedIntegerSelfDivideEliminationRule::contract();
            let original_node = unit.functions[0].blocks[0].nodes[0].clone();
            let accepted_catalog = unit.accepted_obligation_facts.clone();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerSelfDivideEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("one live same-operand division candidate");
            let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(
                patch.location,
                NodeLocation {
                    machine: id(351, MachineId::new),
                    block: id(352, BlockId::new),
                    node: 0,
                }
            );
            assert_eq!(patch.source_operation, id(355, OperationId::new));
            assert_eq!(patch.result, id(354, ValueId::new));
            assert_eq!(patch.scalar_type, integer);
            assert_eq!(patch.constant, integer_one(integer));
            assert_eq!(candidate.predicted_cost_delta(), -1);
            assert!(candidate.substitutions().is_empty());
            assert_eq!(candidate.affected_blocks(), [id(352, BlockId::new)]);
            assert_eq!(candidate.consumed_facts().len(), 1);
            assert!(matches!(
                candidate.consumed_facts()[0],
                OptimizationFactReference::AcceptedObligation(_)
            ));

            let accepted =
                validate_proof_certified_integer_self_divide_candidate(&unit, &candidate).unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1"
                )
            );
            assert_eq!(accepted.unit().accepted_obligation_facts, accepted_catalog);
            let output_node = &accepted.unit().functions[0].blocks[0].nodes[0];
            assert!(matches!(
                output_node.operation,
                O::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type: ScalarType::Integer(output_type),
                    value,
                } if psi_operation == id(355, OperationId::new)
                    && result == id(354, ValueId::new)
                    && output_type == integer
                    && value == integer_one(integer)
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
                        if *value == id(354, ValueId::new)
                            && *constant == integer_one(integer)
                            && *support == id(355, OperationId::new)
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
                LiveProofCertifiedIntegerSelfDivideEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn proof_certified_self_divide_declines_ineligible_shapes_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = live_self_divide_unit(integer, SelfDividePolicy::Exact);
    let contract = LiveProofCertifiedIntegerSelfDivideEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedIntegerSelfDivideEliminationRule
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
            constant: IntegerValue::Unsigned(0),
            ..patch
        },
        IntegerConstantRewrite {
            source_operation: id(358, OperationId::new),
            ..patch
        },
        IntegerConstantRewrite {
            result: id(359, ValueId::new),
            ..patch
        },
        IntegerConstantRewrite {
            scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            constant: IntegerValue::Signed(1),
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
        assert!(validate_proof_certified_integer_self_divide_candidate(&unit, &forged).is_err());
    }

    let foreign_fact = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign self-divide proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_integer_self_divide_candidate(&unit, &foreign_fact),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let forged_cost = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        obligation_fact,
        0,
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_integer_self_divide_candidate(&unit, &forged_cost),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );

    let mut corrupt_provenance = candidate.provenance().to_vec();
    corrupt_provenance[0].fuel[0].units += 1;
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
        validate_proof_certified_integer_self_divide_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );

    let unequal = live_proof_binary_identity_unit(
        integer,
        IntegerValue::Unsigned(1),
        false,
        |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    );
    let signed_one_bit = live_self_divide_unit(
        IntegerType::new(IntegerSign::Signed, 1).unwrap(),
        SelfDividePolicy::Exact,
    );
    let address = live_self_divide_unit(IntegerType::address(64).unwrap(), SelfDividePolicy::Exact);
    for ineligible in [
        unequal,
        signed_one_bit,
        address,
        discard_scalar_function_result(live_self_divide_unit(integer, SelfDividePolicy::Exact)),
    ] {
        let mut manager = crate::AnalysisManager::new(&ineligible);
        let products = manager
            .require_all(&ineligible, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerSelfDivideEliminationRule
                .propose(&ineligible, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    for (remove_catalog, mut missing) in [
        (
            true,
            live_self_divide_unit(integer, SelfDividePolicy::Exact),
        ),
        (
            false,
            live_self_divide_unit(integer, SelfDividePolicy::Exact),
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
            LiveProofCertifiedIntegerSelfDivideEliminationRule
                .propose(&missing, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    let signed_one_bit = live_self_divide_unit(
        IntegerType::new(IntegerSign::Signed, 1).unwrap(),
        SelfDividePolicy::Exact,
    );
    let node = &signed_one_bit.functions[0].blocks[0].nodes[0];
    let signed_one_bit_patch = IntegerConstantRewrite {
        location: NodeLocation {
            machine: signed_one_bit.functions[0].machine,
            block: signed_one_bit.functions[0].blocks[0].id,
            node: 0,
        },
        source_operation: id(355, OperationId::new),
        result: id(354, ValueId::new),
        scalar_type: IntegerType::new(IntegerSign::Signed, 1).unwrap(),
        constant: IntegerValue::Signed(1),
    };
    let site = PsiRealizationSite::Node(signed_one_bit_patch.location);
    let forged_signed_one_bit =
        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            signed_one_bit.identity,
            contract,
            vec![signed_one_bit_patch.location.block],
            vec![ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            }],
            signed_one_bit.accepted_obligation_facts[0].identity,
            -1,
            signed_one_bit_patch,
        )
        .unwrap();
    assert_eq!(
        validate_proof_certified_integer_self_divide_candidate(
            &signed_one_bit,
            &forged_signed_one_bit,
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
}
