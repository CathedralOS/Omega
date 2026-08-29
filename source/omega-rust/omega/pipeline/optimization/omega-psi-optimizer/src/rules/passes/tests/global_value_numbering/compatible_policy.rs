//! Compatible-policy keying, evidence, and local/dominating coverage.

use super::*;

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
