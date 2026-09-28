//! Exact contract, relabelling, cost, and validator custody for all 12 proof-check rows.

use super::super::super::{
    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
    LiveProofCertifiedExactIntegerSelfSubtractEliminationRule,
    LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
    LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
    LiveProofCertifiedIntegerDivideByOneEliminationRule,
    LiveProofCertifiedIntegerIdentityEliminationRule,
    LiveProofCertifiedIntegerRemainderByOneEliminationRule,
    LiveProofCertifiedIntegerSelfDivideEliminationRule,
    LiveProofCertifiedIntegerSelfRemainderEliminationRule,
    LiveProofCertifiedIntegerZeroDividendEliminationRule,
    LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
    ProofCertifiedDeadScalarEliminationRule,
};
use crate::rules::tests::fixtures::dead_scalar_elimination::dead_exact_add_unit;
use crate::rules::tests::fixtures::proof_check_elision::{
    SelfDividePolicy, SelfRemainderPolicy, live_divide_by_one_unit, live_exact_add_zero_unit,
    live_exact_multiply_by_zero_unit, live_exact_self_subtract_unit,
    live_exact_signed_negative_one_shift_right_unit, live_exact_zero_value_shift_unit,
    live_remainder_by_one_unit, live_self_divide_unit, live_self_remainder_unit,
    live_signed_remainder_by_negative_one_unit, live_zero_dividend_unit,
};
use crate::{PsiOptimizationRule, RuleAnalysisView};
use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisKind, OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    PsiOptimizationUnit, PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewritePatch,
};
use semantic_vocabulary::{IntegerSign, IntegerType};

use optimization_core::{AnalysisInvalidationSet, AnalysisSet};
use optimization_unit_semantics::validate_psi_rewrite_candidate;

struct Case {
    unit: PsiOptimizationUnit,
    candidate: PsiRewriteCandidate,
    contract: OptimizationRuleContract,
}

fn case(unit: PsiOptimizationUnit, rule: impl PsiOptimizationRule) -> Case {
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
    }
}

fn cases() -> Vec<Case> {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    vec![
        case(
            dead_exact_add_unit(),
            ProofCertifiedDeadScalarEliminationRule,
        ),
        case(
            live_exact_add_zero_unit(),
            LiveProofCertifiedIntegerIdentityEliminationRule,
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
        ),
        case(
            live_exact_multiply_by_zero_unit(unsigned, false),
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
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
        ),
        case(
            live_exact_zero_value_shift_unit(unsigned, true),
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
        ),
        case(
            live_exact_self_subtract_unit(unsigned),
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule,
        ),
        case(
            live_self_remainder_unit(unsigned, SelfRemainderPolicy::Exact),
            LiveProofCertifiedIntegerSelfRemainderEliminationRule,
        ),
        case(
            live_self_divide_unit(unsigned, SelfDividePolicy::Exact),
            LiveProofCertifiedIntegerSelfDivideEliminationRule,
        ),
        case(
            live_remainder_by_one_unit(unsigned, SelfRemainderPolicy::Exact),
            LiveProofCertifiedIntegerRemainderByOneEliminationRule,
        ),
        case(
            live_signed_remainder_by_negative_one_unit(signed, SelfRemainderPolicy::Exact),
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
        ),
        case(
            live_exact_signed_negative_one_shift_right_unit(signed),
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
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
