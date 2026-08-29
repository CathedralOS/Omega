//! Same-block, dominating, and phi-translated GVN tests.

use super::*;

#[test]
fn same_block_cse_uses_earliest_typed_leader_and_moves_custody_forward() {
    let unit = local_cse_unit();
    let contract = SameBlockTotalScalarCseRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = SameBlockTotalScalarCseRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("swapped commutative operands have one exact CSE candidate");
    assert!(matches!(
        candidate.patch(),
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_)
    ));
    assert_eq!(
        candidate.substitutions(),
        [ScalarSubstitution {
            from: id(1_306, ValueId::new),
            to: id(1_305, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
        }]
    );
    let accepted = validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    let nodes = &output.functions[0].blocks[0].nodes;
    assert_eq!(nodes.len(), 3);
    assert!(
        matches!(nodes[1].operation, O::IntegerEqual { left, right, .. } if left == id(1_305, ValueId::new) && right == left)
    );
    assert_eq!(
        nodes[1].provenance,
        [
            PsiProvenance::Operation(id(1_310, OperationId::new)),
            PsiProvenance::Operation(id(1_309, OperationId::new))
        ]
    );
    assert_eq!(accepted.provenance().len(), 3);
    assert!(
        output.functions[0].blocks[0]
            .nodes
            .iter()
            .flat_map(|node| &node.uses)
            .all(|row| row.value != id(1_306, ValueId::new))
    );

    let mut manager = crate::AnalysisManager::new(output);
    let products = manager
        .require_all(output, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockTotalScalarCseRule
            .propose(output, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch() else {
        unreachable!()
    };
    let mut provenance = candidate.provenance().to_vec();
    provenance[0].disposition =
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(patch.leader));
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let forged = PsiRewriteCandidate::new_local_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        provenance,
        -1,
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
}

#[test]
fn proof_certified_same_block_cse_consumes_the_redundant_operations_fact() {
    let unit = proof_certified_local_cse_unit();
    let ordinary_contract = SameBlockTotalScalarCseRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, ordinary_contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockTotalScalarCseRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let contract = SameBlockProofCertifiedScalarCseRule::contract();
    assert_eq!(
        contract.safety_class(),
        OptimizationSafetyClass::ProofCertified
    );
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = SameBlockProofCertifiedScalarCseRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("swapped exact-add operands produce one proof-certified CSE candidate");
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == id(1_309, OperationId::new))
        .expect("fixture retains the redundant operation fact")
        .identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    assert_eq!(
        candidate.consumed_facts(),
        [omega_optimization_core::OptimizationFactReference::AcceptedObligation(redundant_fact,)]
    );
    let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch() else {
        unreachable!()
    };
    assert_eq!(patch.leader_operation, id(1_308, OperationId::new));
    assert_eq!(patch.redundant_operation, id(1_309, OperationId::new));
    let accepted = validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 3);
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
        matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_308, OperationId::new)
        )
    }));
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_309, OperationId::new)
        )
    }));

    let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign proof-certified local CSE fact",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut missing_leader = unit.clone();
    missing_leader
        .accepted_obligation_facts
        .retain(|fact| fact.operation != id(1_308, OperationId::new));
    missing_leader.identity = recompute_psi_optimization_unit_identity(&missing_leader);
    let uses = compute_analysis(&missing_leader, AnalysisKind::UseDefinition).unwrap();
    let effects = compute_analysis(&missing_leader, AnalysisKind::EffectSummaries).unwrap();
    assert!(
        SameBlockProofCertifiedScalarCseRule
            .propose(&missing_leader, RuleAnalysisView::new(&[uses, effects]))
            .unwrap()
            .is_empty()
    );
    let forged_without_leader_fact =
        PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
            missing_leader.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            redundant_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(
            &missing_leader,
            &forged_without_leader_fact,
        ),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut missing_redundant = unit.clone();
    missing_redundant
        .accepted_obligation_facts
        .retain(|fact| fact.operation != id(1_309, OperationId::new));
    missing_redundant.identity = recompute_psi_optimization_unit_identity(&missing_redundant);
    let uses = compute_analysis(&missing_redundant, AnalysisKind::UseDefinition).unwrap();
    let effects = compute_analysis(&missing_redundant, AnalysisKind::EffectSummaries).unwrap();
    assert!(
        SameBlockProofCertifiedScalarCseRule
            .propose(&missing_redundant, RuleAnalysisView::new(&[uses, effects]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn proof_certified_dominator_gvn_consumes_cross_block_redundant_evidence() {
    let unit = proof_certified_dominator_gvn_unit();
    let contract = DominatorProofCertifiedScalarGvnRule::contract();
    assert_eq!(
        contract.safety_class(),
        OptimizationSafetyClass::ProofCertified
    );
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = DominatorProofCertifiedScalarGvnRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("the entry exact add dominates one proof-certified duplicate");
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == id(1_351, OperationId::new))
        .expect("fixture retains the dominated operation fact")
        .identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(patch.leader.block, id(1_343, BlockId::new));
    assert_eq!(patch.redundant.block, id(1_342, BlockId::new));
    assert_eq!(patch.leader_operation, id(1_349, OperationId::new));
    assert_eq!(patch.redundant_operation, id(1_351, OperationId::new));
    let accepted =
        validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_351, OperationId::new)
        )
    }));

    let forged = PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign proof-certified dominator GVN fact",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
}

#[test]
fn proof_certified_cse_expression_vocabulary_is_closed_and_exact() {
    let seed = proof_certified_local_cse_unit();
    let O::ExactIntegerAdd {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    } = seed.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    let operations = [
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type: scalar_type,
            target_type: scalar_type,
            operand: left,
        },
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    ];
    for operation in &operations {
        assert!(
            proof_certified_scalar_expression(operation).is_some(),
            "closed proof-bearing shape must have an expression key: {operation:?}"
        );
    }
    assert!(
        proof_certified_scalar_expression(&O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        })
        .is_none()
    );

    let exact_add = proof_certified_scalar_expression(&operations[3]).unwrap().0;
    let swapped_add = proof_certified_scalar_expression(&O::ExactIntegerAdd {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left: right,
        right: left,
    })
    .unwrap()
    .0;
    assert_eq!(exact_add, swapped_add);
    let subtract = proof_certified_scalar_expression(&operations[4]).unwrap().0;
    let swapped_subtract = proof_certified_scalar_expression(&O::ExactIntegerSubtract {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left: right,
        right: left,
    })
    .unwrap()
    .0;
    assert_ne!(subtract, swapped_subtract);
}

#[test]
fn compatible_policy_keys_cover_only_exact_total_counterparts_with_correct_ordering() {
    let value_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let left = id(20_001, ValueId::new);
    let right = id(20_002, ValueId::new);
    let leader_operation = id(20_003, OperationId::new);
    let redundant_operation = id(20_004, OperationId::new);
    let leader_result = id(20_005, ValueId::new);
    let redundant_result = id(20_006, ValueId::new);
    let obligation = id(20_007, ObligationId::new);

    for (leader, redundant) in [
        (
            O::WrappingIntegerAdd {
                psi_operation: leader_operation,
                result: leader_result,
                scalar_type: value_type,
                left,
                right,
            },
            O::ExactIntegerAdd {
                psi_operation: redundant_operation,
                obligation,
                result: redundant_result,
                scalar_type: value_type,
                left: right,
                right: left,
            },
        ),
        (
            O::SaturatingIntegerAdd {
                psi_operation: leader_operation,
                result: leader_result,
                scalar_type: value_type,
                left,
                right,
            },
            O::ExactIntegerAdd {
                psi_operation: redundant_operation,
                obligation,
                result: redundant_result,
                scalar_type: value_type,
                left,
                right,
            },
        ),
        (
            O::WrappingIntegerSubtract {
                psi_operation: leader_operation,
                result: leader_result,
                scalar_type: value_type,
                left,
                right,
            },
            O::ExactIntegerSubtract {
                psi_operation: redundant_operation,
                obligation,
                result: redundant_result,
                scalar_type: value_type,
                left,
                right,
            },
        ),
        (
            O::SaturatingIntegerMultiply {
                psi_operation: leader_operation,
                result: leader_result,
                scalar_type: value_type,
                left,
                right,
            },
            O::ExactIntegerMultiply {
                psi_operation: redundant_operation,
                obligation,
                result: redundant_result,
                scalar_type: value_type,
                left: right,
                right: left,
            },
        ),
        (
            O::WrappingIntegerShiftLeft {
                psi_operation: leader_operation,
                result: leader_result,
                value_type,
                count_type,
                value: left,
                count: right,
            },
            O::ExactIntegerShiftLeft {
                psi_operation: redundant_operation,
                obligation,
                result: redundant_result,
                value_type,
                count_type,
                value: left,
                count: right,
            },
        ),
        (
            O::WrappingIntegerShiftRight {
                psi_operation: leader_operation,
                result: leader_result,
                value_type,
                count_type,
                value: left,
                count: right,
            },
            O::ExactIntegerShiftRight {
                psi_operation: redundant_operation,
                obligation,
                result: redundant_result,
                value_type,
                count_type,
                value: left,
                count: right,
            },
        ),
    ] {
        assert_eq!(
            compatible_policy_scalar_leader(&leader).map(|row| row.0),
            compatible_policy_scalar_redundant(&redundant).map(|row| row.0)
        );
    }

    let reversed_subtract = O::ExactIntegerSubtract {
        psi_operation: redundant_operation,
        obligation,
        result: redundant_result,
        scalar_type: value_type,
        left: right,
        right: left,
    };
    let subtract_leader = O::WrappingIntegerSubtract {
        psi_operation: leader_operation,
        result: leader_result,
        scalar_type: value_type,
        left,
        right,
    };
    assert_ne!(
        compatible_policy_scalar_leader(&subtract_leader).map(|row| row.0),
        compatible_policy_scalar_redundant(&reversed_subtract).map(|row| row.0)
    );
    assert!(
        compatible_policy_scalar_redundant(&O::ExactIntegerDivide {
            psi_operation: redundant_operation,
            obligation,
            result: redundant_result,
            scalar_type: value_type,
            left,
            right,
        })
        .is_none()
    );
}

#[test]
fn compatible_policy_key_matrix_binds_sign_width_family_policy_and_operand_order() {
    #[derive(Clone, Copy)]
    enum ArithmeticFamily {
        Add,
        Subtract,
        Multiply,
    }
    #[derive(Clone, Copy)]
    enum LeaderPolicy {
        Wrapping,
        Saturating,
    }

    let operation = id(20_101, OperationId::new);
    let result = id(20_102, ValueId::new);
    let obligation = id(20_103, ObligationId::new);
    let left = id(20_104, ValueId::new);
    let right = id(20_105, ValueId::new);
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for bits in [8, 32, 128] {
            let scalar_type = IntegerType::new(sign, bits).unwrap();
            let foreign_domain = IntegerType::new(sign, bits / 2 + 1).unwrap();
            for family in [
                ArithmeticFamily::Add,
                ArithmeticFamily::Subtract,
                ArithmeticFamily::Multiply,
            ] {
                let redundant = match family {
                    ArithmeticFamily::Add => O::ExactIntegerAdd {
                        psi_operation: operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    ArithmeticFamily::Subtract => O::ExactIntegerSubtract {
                        psi_operation: operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    ArithmeticFamily::Multiply => O::ExactIntegerMultiply {
                        psi_operation: operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                };
                let redundant_key = compatible_policy_scalar_redundant(&redundant).unwrap().0;
                for policy in [LeaderPolicy::Wrapping, LeaderPolicy::Saturating] {
                    let leader = match (family, policy) {
                        (ArithmeticFamily::Add, LeaderPolicy::Wrapping) => O::WrappingIntegerAdd {
                            psi_operation: operation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        (ArithmeticFamily::Add, LeaderPolicy::Saturating) => {
                            O::SaturatingIntegerAdd {
                                psi_operation: operation,
                                result,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        (ArithmeticFamily::Subtract, LeaderPolicy::Wrapping) => {
                            O::WrappingIntegerSubtract {
                                psi_operation: operation,
                                result,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        (ArithmeticFamily::Subtract, LeaderPolicy::Saturating) => {
                            O::SaturatingIntegerSubtract {
                                psi_operation: operation,
                                result,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        (ArithmeticFamily::Multiply, LeaderPolicy::Wrapping) => {
                            O::WrappingIntegerMultiply {
                                psi_operation: operation,
                                result,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        (ArithmeticFamily::Multiply, LeaderPolicy::Saturating) => {
                            O::SaturatingIntegerMultiply {
                                psi_operation: operation,
                                result,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                    };
                    assert_eq!(
                        compatible_policy_scalar_leader(&leader).unwrap().0,
                        redundant_key
                    );
                }
                let swapped = match family {
                    ArithmeticFamily::Add => O::ExactIntegerAdd {
                        psi_operation: operation,
                        obligation,
                        result,
                        scalar_type,
                        left: right,
                        right: left,
                    },
                    ArithmeticFamily::Subtract => O::ExactIntegerSubtract {
                        psi_operation: operation,
                        obligation,
                        result,
                        scalar_type,
                        left: right,
                        right: left,
                    },
                    ArithmeticFamily::Multiply => O::ExactIntegerMultiply {
                        psi_operation: operation,
                        obligation,
                        result,
                        scalar_type,
                        left: right,
                        right: left,
                    },
                };
                let swapped_key = compatible_policy_scalar_redundant(&swapped).unwrap().0;
                assert_eq!(
                    redundant_key == swapped_key,
                    !matches!(family, ArithmeticFamily::Subtract)
                );
                let foreign = O::WrappingIntegerAdd {
                    psi_operation: operation,
                    result,
                    scalar_type: foreign_domain,
                    left,
                    right,
                };
                assert_ne!(
                    compatible_policy_scalar_leader(&foreign).unwrap().0,
                    redundant_key
                );
            }

            for shift_left in [true, false] {
                let count_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
                let leader = if shift_left {
                    O::WrappingIntegerShiftLeft {
                        psi_operation: operation,
                        result,
                        value_type: scalar_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                } else {
                    O::WrappingIntegerShiftRight {
                        psi_operation: operation,
                        result,
                        value_type: scalar_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                };
                let redundant = if shift_left {
                    O::ExactIntegerShiftLeft {
                        psi_operation: operation,
                        obligation,
                        result,
                        value_type: scalar_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                } else {
                    O::ExactIntegerShiftRight {
                        psi_operation: operation,
                        obligation,
                        result,
                        value_type: scalar_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                };
                assert_eq!(
                    compatible_policy_scalar_leader(&leader).unwrap().0,
                    compatible_policy_scalar_redundant(&redundant).unwrap().0
                );
                let reversed = if shift_left {
                    O::ExactIntegerShiftLeft {
                        psi_operation: operation,
                        obligation,
                        result,
                        value_type: scalar_type,
                        count_type,
                        value: right,
                        count: left,
                    }
                } else {
                    O::ExactIntegerShiftRight {
                        psi_operation: operation,
                        obligation,
                        result,
                        value_type: scalar_type,
                        count_type,
                        value: right,
                        count: left,
                    }
                };
                assert_ne!(
                    compatible_policy_scalar_leader(&leader).unwrap().0,
                    compatible_policy_scalar_redundant(&reversed).unwrap().0
                );
            }
        }
    }
}

#[test]
fn compatible_policy_local_and_dominator_gvn_consume_only_redundant_proof_custody() {
    let cases: [(
        PsiOptimizationUnit,
        OptimizationRuleContract,
        &dyn PsiOptimizationRule,
        bool,
    ); 2] = [
        (
            compatible_policy_local_cse_unit(),
            SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract(),
            &SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
            false,
        ),
        (
            compatible_policy_dominator_gvn_unit(),
            DominatorProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            &DominatorProofCertifiedCompatiblePolicyScalarGvnRule,
            true,
        ),
    ];
    for (unit, contract, rule, dominating) in cases {
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = rule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("one compatible-policy redundant expression");
        assert_eq!(candidate.consumed_facts().len(), 1);
        let accepted = if dominating {
            validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap()
        } else {
            validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap()
        };
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch()
        else {
            if let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) =
                candidate.patch()
            {
                assert!(!accepted.unit().functions[0].facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                            if *support == patch.redundant_operation)
                }));
                continue;
            }
            unreachable!("compatible GVN uses a scalar CSE patch")
        };
        assert!(!accepted.unit().functions[0].facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                    if *support == patch.redundant_operation)
        }));
    }
}

#[test]
fn compatible_policy_gvn_declines_missing_evidence_and_rejects_corruption() {
    let unit = compatible_policy_local_cse_unit();
    let contract = SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = SameBlockProofCertifiedCompatiblePolicyScalarCseRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch() else {
        unreachable!()
    };

    let mut wrong_patch = patch;
    wrong_patch.scalar_type = ScalarType::Boolean;
    let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        candidate.accepted_obligation_witness().unwrap(),
        -1,
        wrong_patch,
    )
    .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let foreign_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == patch.leader_operation)
        .unwrap()
        .identity;
    let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        foreign_fact,
        -1,
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut provenance = candidate.provenance().to_vec();
    provenance[0].fuel[0].units = provenance[0].fuel[0].units.saturating_add(1);
    let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        provenance,
        candidate.accepted_obligation_witness().unwrap(),
        -1,
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_local_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );

    let mut missing = unit.clone();
    missing
        .accepted_obligation_facts
        .retain(|fact| fact.operation != patch.redundant_operation);
    missing.identity = recompute_psi_optimization_unit_identity(&missing);
    let mut manager = crate::AnalysisManager::new(&missing);
    let products = manager
        .require_all(&missing, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockProofCertifiedCompatiblePolicyScalarCseRule
            .propose(&missing, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
    let exact_only = proof_certified_local_cse_unit();
    let mut manager = crate::AnalysisManager::new(&exact_only);
    let products = manager
        .require_all(&exact_only, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockProofCertifiedCompatiblePolicyScalarCseRule
            .propose(&exact_only, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn dominator_gvn_reuses_a_canonical_cross_block_total_scalar_expression() {
    let unit = dominator_gvn_unit();
    let local_contract = SameBlockTotalScalarCseRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let local_products = manager
        .require_all(&unit, local_contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        SameBlockTotalScalarCseRule
            .propose(&unit, RuleAnalysisView::new(&local_products))
            .unwrap()
            .is_empty()
    );

    let contract = DominatorTotalScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = DominatorTotalScalarGvnRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("entry expression strictly dominates one cross-block duplicate");
    let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(patch.leader.block, id(1_343, BlockId::new));
    assert_eq!(patch.redundant.block, id(1_342, BlockId::new));
    let accepted =
        validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 2);
    assert!(
        matches!(output.functions[0].blocks[0].nodes[0].operation, O::IntegerEqual { left, right, .. } if left == id(1_346, ValueId::new) && right == left)
    );
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].provenance,
        [
            PsiProvenance::Operation(id(1_352, OperationId::new)),
            PsiProvenance::Operation(id(1_351, OperationId::new))
        ]
    );
    assert!(
        output.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .all(|row| row.value != id(1_347, ValueId::new))
    );

    let mut forged_patch = patch;
    forged_patch.leader.node = 1;
    forged_patch.leader_operation = id(1_350, OperationId::new);
    let forged = PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        -1,
        forged_patch,
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
}

#[test]
fn dominator_gvn_cascades_through_a_non_topological_diamond_to_fixed_point() {
    let mut unit = diamond_dominator_gvn_unit();
    let contract = DominatorTotalScalarGvnRule::contract();
    for (expected_redundant, expected_leader) in [
        (id(1_410, ValueId::new), id(1_408, ValueId::new)),
        (id(1_411, ValueId::new), id(1_409, ValueId::new)),
    ] {
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("one newly exposed cross-block value number");
        let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.redundant_result, expected_redundant);
        assert_eq!(patch.leader_result, expected_leader);
        assert_eq!(
            candidate.affected_blocks(),
            [
                id(1_402, BlockId::new),
                id(1_403, BlockId::new),
                id(1_404, BlockId::new),
                id(1_405, BlockId::new)
            ]
        );
        unit = validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate)
            .unwrap()
            .into_unit();
    }
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
    let join = &unit.functions[0].blocks[0];
    assert_eq!(join.nodes.len(), 1);
    assert!(
        matches!(join.nodes[0].operation, O::Return { value, .. } if value == id(1_409, ValueId::new))
    );
    assert_eq!(
        join.nodes[0].provenance,
        [
            PsiProvenance::Edge(id(1_414, EdgeId::new)),
            PsiProvenance::Operation(id(1_413, OperationId::new)),
            PsiProvenance::Operation(id(1_412, OperationId::new))
        ]
    );
}

#[test]
fn dominator_gvn_rejects_an_equivalent_sibling_expression_at_a_join() {
    let unit = sibling_only_gvn_unit();
    let contract = DominatorTotalScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let function = &unit.functions[0];
    let leader = NodeLocation {
        machine: function.machine,
        block: id(1_443, BlockId::new),
        node: 0,
    };
    let redundant = NodeLocation {
        machine: function.machine,
        block: id(1_442, BlockId::new),
        node: 0,
    };
    let (affected, provenance) =
        local_cse_accounting(function, redundant, id(1_449, ValueId::new)).unwrap();
    let forged = PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        affected,
        provenance,
        -1,
        DominatingScalarCommonSubexpressionRewrite {
            leader,
            redundant,
            leader_operation: id(1_452, OperationId::new),
            redundant_operation: id(1_450, OperationId::new),
            leader_result: id(1_448, ValueId::new),
            redundant_result: id(1_449, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        },
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
}

fn phi_translated_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let contract = PhiTranslatedObligationFreeScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(unit);
    let products = manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    PhiTranslatedObligationFreeScalarGvnRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

fn proof_certified_phi_translated_candidates(
    unit: &PsiOptimizationUnit,
) -> Vec<PsiRewriteCandidate> {
    let contract = PhiTranslatedProofCertifiedScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(unit);
    let products = manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    PhiTranslatedProofCertifiedScalarGvnRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

fn compatible_policy_phi_translated_candidates(
    unit: &PsiOptimizationUnit,
) -> Vec<PsiRewriteCandidate> {
    let contract = PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(unit);
    let products = manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn phi_translated_gvn_preserves_result_identity_and_reaches_fixed_point() {
    let unit = phi_translated_gvn_unit();
    let [candidate] = phi_translated_candidates(&unit)
        .try_into()
        .expect("both predecessor translations have available leaders");
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(patch.parameter_position, 1);
    assert_eq!(patch.redundant_result, id(1_710, ValueId::new));
    assert_eq!(
        patch
            .incoming
            .iter()
            .map(|row| (row.edge, row.source, row.leader_result))
            .collect::<Vec<_>>(),
        [
            (
                id(1_717, EdgeId::new),
                id(1_705, BlockId::new),
                id(1_712, ValueId::new),
            ),
            (
                id(1_720, EdgeId::new),
                id(1_703, BlockId::new),
                id(1_711, ValueId::new),
            ),
        ]
    );
    assert!(candidate.substitutions().is_empty());
    assert!(candidate.consumed_facts().is_empty());

    let accepted =
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    let join = &output.functions[0].blocks[0];
    assert_eq!(join.parameters.len(), 2);
    assert_eq!(join.parameters[1].value, id(1_710, ValueId::new));
    assert_eq!(join.nodes.len(), 1);
    assert!(
        matches!(join.nodes[0].operation, O::Return { value, .. } if value == id(1_710, ValueId::new))
    );
    for (source, leader) in [
        (id(1_703, BlockId::new), id(1_711, ValueId::new)),
        (id(1_705, BlockId::new), id(1_712, ValueId::new)),
    ] {
        let edge = output.functions[0]
            .blocks
            .iter()
            .find(|block| block.id == source)
            .unwrap()
            .nodes
            .last()
            .unwrap()
            .successors
            .first()
            .unwrap();
        assert_eq!(edge.bindings.len(), 2);
        assert_eq!(edge.bindings[1].parameter, id(1_710, ValueId::new));
        assert_eq!(edge.bindings[1].argument, leader);
    }
    assert!(phi_translated_candidates(output).is_empty());

    let mut corrupted_patch = patch;
    corrupted_patch.incoming[0].leader_result = id(1_711, ValueId::new);
    let corrupted = PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
        unit.identity,
        PhiTranslatedObligationFreeScalarGvnRule::contract(),
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        candidate.predicted_cost_delta(),
        corrupted_patch,
    )
    .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &corrupted),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );
}

#[test]
fn phi_translated_gvn_requires_a_typed_leader_on_every_incoming_arm() {
    for right_arm in [
        PhiTranslatedRightArm::Missing,
        PhiTranslatedRightArm::MismatchedType,
    ] {
        let unit = phi_translated_gvn_fixture(right_arm, false, false);
        assert!(phi_translated_candidates(&unit).is_empty());
    }
}

#[test]
fn phi_translated_gvn_candidate_rejects_noncanonical_incoming_order() {
    let unit = phi_translated_gvn_unit();
    let [candidate] = phi_translated_candidates(&unit).try_into().unwrap();
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(mut patch) =
        candidate.patch()
    else {
        unreachable!()
    };
    patch.incoming.reverse();
    assert_eq!(
        PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
            unit.identity,
            PhiTranslatedObligationFreeScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.predicted_cost_delta(),
            patch,
        ),
        Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
    );
}

#[test]
fn proof_certified_phi_translation_consumes_only_redundant_evidence() {
    let unit = proof_certified_phi_translated_gvn_unit();
    assert!(phi_translated_candidates(&unit).is_empty());
    let [candidate] = proof_certified_phi_translated_candidates(&unit)
        .try_into()
        .expect("all three exact-add operations retain accepted evidence");
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == id(1_713, OperationId::new))
        .unwrap()
        .identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    assert_eq!(
        candidate.consumed_facts(),
        [omega_optimization_core::OptimizationFactReference::AcceptedObligation(redundant_fact,),]
    );
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(
        patch
            .incoming
            .iter()
            .map(|row| row.leader_operation)
            .collect::<Vec<_>>(),
        [id(1_716, OperationId::new), id(1_715, OperationId::new),]
    );
    let accepted =
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1",
        )
    );
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_713, OperationId::new))
    }));
    assert!(proof_certified_phi_translated_candidates(accepted.unit()).is_empty());

    let foreign =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            unit.identity,
            PhiTranslatedProofCertifiedScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign proof phi fact",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &foreign),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
}

#[test]
fn proof_certified_phi_translation_requires_every_leader_fact() {
    let original = proof_certified_phi_translated_gvn_unit();
    let [candidate] = proof_certified_phi_translated_candidates(&original)
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    let redundant_fact = candidate.accepted_obligation_witness().unwrap();
    let mut unit = original;
    unit.accepted_obligation_facts
        .retain(|fact| fact.operation != id(1_716, OperationId::new));
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(proof_certified_phi_translated_candidates(&unit).is_empty());
    let detached_leader =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            unit.identity,
            PhiTranslatedProofCertifiedScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            redundant_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &detached_leader,),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
}

#[test]
fn compatible_policy_phi_translation_preserves_result_and_consumes_only_redundant_evidence() {
    let unit = compatible_policy_phi_translated_gvn_unit();
    let [candidate] = compatible_policy_phi_translated_candidates(&unit)
        .try_into()
        .expect("wrapping and saturating arm leaders are compatible");
    assert!(phi_translated_candidates(&unit).is_empty());
    assert!(proof_certified_phi_translated_candidates(&unit).is_empty());
    let redundant_fact = unit.accepted_obligation_facts[0].identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    assert_eq!(candidate.substitutions(), []);
    assert_eq!(candidate.consumed_facts().len(), 1);
    let accepted =
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
        )
    );
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_713, OperationId::new))
    }));
    let join = &accepted.unit().functions[0].blocks[0];
    assert_eq!(
        join.parameters.last().unwrap().value,
        id(1_710, ValueId::new)
    );
    assert_eq!(join.nodes.len(), 1);
    assert!(compatible_policy_phi_translated_candidates(accepted.unit()).is_empty());
}

#[test]
fn compatible_policy_phi_translation_declines_incomplete_arms_and_rejects_corruption() {
    for right_arm in [
        PhiTranslatedRightArm::Missing,
        PhiTranslatedRightArm::MismatchedType,
    ] {
        let unit = phi_translated_gvn_fixture(right_arm, false, true);
        assert!(compatible_policy_phi_translated_candidates(&unit).is_empty());
    }

    let original = compatible_policy_phi_translated_gvn_unit();
    let [candidate] = compatible_policy_phi_translated_candidates(&original)
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };

    let mut missing_fact = original.clone();
    missing_fact.accepted_obligation_facts.clear();
    missing_fact.identity = recompute_psi_optimization_unit_identity(&missing_fact);
    assert!(compatible_policy_phi_translated_candidates(&missing_fact).is_empty());

    let foreign_fact =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign compatible phi fact",
            ),
            candidate.predicted_cost_delta(),
            patch.clone(),
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&original, &foreign_fact,),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut unavailable_patch = patch.clone();
    unavailable_patch.incoming[0].leader.node = 1;
    let unavailable =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap(),
            candidate.predicted_cost_delta(),
            unavailable_patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&original, &unavailable),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );

    let mut detached_patch = patch.clone();
    detached_patch.incoming[0].leader_operation = id(20_201, OperationId::new);
    detached_patch.incoming[0].leader_result = id(20_202, ValueId::new);
    let detached =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap(),
            candidate.predicted_cost_delta(),
            detached_patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&original, &detached),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );

    let mut reordered_patch = patch;
    reordered_patch.incoming.reverse();
    assert_eq!(
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap(),
            candidate.predicted_cost_delta(),
            reordered_patch,
        ),
        Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
    );
}
