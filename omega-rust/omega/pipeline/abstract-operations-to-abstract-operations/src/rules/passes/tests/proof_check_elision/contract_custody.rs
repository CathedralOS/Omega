//! Exact contract, relabelling, cost, and validator custody for all 12 proof-check rows.

use optimization_core::{AnalysisInvalidationSet, AnalysisSet};
use optimization_validation::validate_psi_rewrite_candidate;

use super::*;

struct Case {
    unit: PsiOptimizationUnit,
    candidate: PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    validator: OptimizationValidatorIdentity,
}

fn case(
    unit: PsiOptimizationUnit,
    rule: impl PsiOptimizationRule,
    validator_domain: &[u8],
) -> Case {
    let contract = rule.contract();
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
        .expect("representative proof-check fixture proposes exactly once");
    Case {
        unit,
        candidate,
        contract,
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator_domain),
    }
}

fn cases() -> Vec<Case> {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    vec![
        case(
            dead_exact_add_unit(),
            ProofCertifiedDeadScalarEliminationRule,
            b"omega.validator.dead-unused-proof-certified-scalar-node.v1",
        ),
        case(
            live_exact_add_zero_unit(),
            LiveProofCertifiedIntegerIdentityEliminationRule,
            b"omega.validator.live-proof-certified-integer-identity-elimination.v1",
        ),
        case(
            live_divide_by_one_unit(
                unsigned,
                |psi_operation, obligation, result, scalar_type, left, right| {
                    O::ExactIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                },
            ),
            LiveProofCertifiedIntegerDivideByOneEliminationRule,
            b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1",
        ),
        case(
            live_exact_multiply_by_zero_unit(unsigned, false),
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
            b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
        ),
        case(
            live_zero_dividend_unit(
                unsigned,
                |psi_operation, obligation, result, scalar_type, left, right| {
                    O::ExactIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                },
            ),
            LiveProofCertifiedIntegerZeroDividendEliminationRule,
            b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1",
        ),
        case(
            live_exact_zero_value_shift_unit(unsigned, true),
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
            b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
        ),
        case(
            live_exact_self_subtract_unit(unsigned),
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule,
            b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1",
        ),
        case(
            live_self_remainder_unit(unsigned, SelfRemainderPolicy::Exact),
            LiveProofCertifiedIntegerSelfRemainderEliminationRule,
            b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1",
        ),
        case(
            live_self_divide_unit(unsigned, SelfDividePolicy::Exact),
            LiveProofCertifiedIntegerSelfDivideEliminationRule,
            b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1",
        ),
        case(
            live_remainder_by_one_unit(unsigned, SelfRemainderPolicy::Exact),
            LiveProofCertifiedIntegerRemainderByOneEliminationRule,
            b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1",
        ),
        case(
            live_signed_remainder_by_negative_one_unit(signed, SelfRemainderPolicy::Exact),
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
            b"omega.validator.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
        ),
        case(
            live_exact_signed_negative_one_shift_right_unit(signed),
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
            b"omega.validator.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1",
        ),
    ]
}

fn rebuilt(
    candidate: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    predicted_cost_delta: i64,
) -> Result<PsiRewriteCandidate, PsiRewriteCandidateError> {
    match candidate.patch() {
        PsiRewritePatch::RemoveDeadScalarNode(patch) => {
            if contract.safety_class() == OptimizationSafetyClass::ProofCertified {
                PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    candidate.accepted_obligation_witness().unwrap(),
                    predicted_cost_delta,
                    patch,
                )
            } else {
                PsiRewriteCandidate::new_dead_scalar_node(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    predicted_cost_delta,
                    patch,
                )
            }
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
            let (constant_fact, obligation_fact) =
                candidate.proof_certified_scalar_identity_witness().unwrap();
            PsiRewriteCandidate::new_proof_certified_scalar_identity(
                candidate.input(),
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                constant_fact,
                obligation_fact,
                predicted_cost_delta,
                patch,
            )
        }
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => {
            if let Some((constant_fact, obligation_fact)) =
                candidate.proof_certified_scalar_identity_witness()
            {
                PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    constant_fact,
                    obligation_fact,
                    predicted_cost_delta,
                    patch,
                )
            } else {
                PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    candidate.accepted_obligation_witness().unwrap(),
                    predicted_cost_delta,
                    patch,
                )
            }
        }
        _ => panic!("proof-check roster uses exactly three patch protocols"),
    }
}

fn contract_with(
    base: OptimizationRuleContract,
    identity: OptimizationRuleIdentity,
    required: AnalysisSet,
    invalidated: AnalysisInvalidationSet,
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        identity,
        base.pass(),
        base.version(),
        required,
        invalidated,
        safety,
    )
    .unwrap()
}

fn required_superset(base: OptimizationRuleContract) -> AnalysisSet {
    if base
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
    {
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::ScalarConstants,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ])
    } else if base
        .required_analyses()
        .contains(AnalysisKind::ValueLiveness)
    {
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::ValueLiveness,
            AnalysisKind::EffectSummaries,
        ])
    } else {
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ])
    }
}

fn required_subset(base: OptimizationRuleContract) -> AnalysisSet {
    if base
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
    {
        AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries])
    } else if base
        .required_analyses()
        .contains(AnalysisKind::ValueLiveness)
    {
        AnalysisSet::new([AnalysisKind::ValueLiveness])
    } else {
        AnalysisSet::new([AnalysisKind::UseDefinition])
    }
}

#[test]
fn proof_check_roster_and_candidates_bind_complete_exact_contracts() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let roster = built_in_psi_registry(&selections).unwrap();
    let cases = cases();
    assert_eq!(
        roster.contracts().collect::<Vec<_>>(),
        cases.iter().map(|case| case.contract).collect::<Vec<_>>()
    );

    for case in cases {
        assert_eq!(case.candidate.rule(), case.contract.identity());
        assert_eq!(
            case.candidate.required_analyses(),
            case.contract.required_analyses()
        );
        assert_eq!(
            case.candidate.invalidated_analyses(),
            case.contract.invalidated_analyses()
        );
        assert_eq!(case.candidate.safety_class(), case.contract.safety_class());
        assert_eq!(case.candidate.predicted_cost_delta(), -1);
        assert_eq!(
            validate_psi_rewrite_candidate(&case.unit, &case.candidate)
                .unwrap()
                .validator(),
            case.validator
        );
    }
}

#[test]
fn every_proof_check_rule_rejects_cross_rule_and_contract_corruption() {
    let cases = cases();
    let contracts = cases.iter().map(|case| case.contract).collect::<Vec<_>>();

    for (case_index, case) in cases.iter().enumerate() {
        for (contract_index, contract) in contracts.iter().copied().enumerate() {
            if case_index == contract_index {
                continue;
            }
            let relabelled = rebuilt(&case.candidate, contract, -1).unwrap();
            assert!(validate_psi_rewrite_candidate(&case.unit, &relabelled).is_err());
        }

        let base = case.contract;
        let corrupt_contracts = [
            contract_with(
                base,
                OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.unknown-proof-check-elision.v1",
                ),
                base.required_analyses(),
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                required_superset(base),
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                required_subset(base),
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                base.required_analyses(),
                AnalysisInvalidationSet::new([
                    AnalysisKind::ControlFlowGraph,
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                base.required_analyses(),
                AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
                base.safety_class(),
            ),
        ];
        for contract in corrupt_contracts {
            let corrupted = rebuilt(&case.candidate, contract, -1).unwrap();
            assert!(validate_psi_rewrite_candidate(&case.unit, &corrupted).is_err());
        }

        let wrong_cost = rebuilt(&case.candidate, base, 0).unwrap();
        assert!(validate_psi_rewrite_candidate(&case.unit, &wrong_cost).is_err());

        let wrong_safety = contract_with(
            base,
            base.identity(),
            base.required_analyses(),
            base.invalidated_analyses(),
            OptimizationSafetyClass::ExactOperationSemantics,
        );
        match rebuilt(&case.candidate, wrong_safety, -1) {
            Ok(candidate) => {
                assert!(validate_psi_rewrite_candidate(&case.unit, &candidate).is_err())
            }
            Err(PsiRewriteCandidateError::ProofWitnessSafetyMismatch) => {}
            Err(other) => panic!("wrong safety fails only at its exact custody fence: {other:?}"),
        }
    }
}
