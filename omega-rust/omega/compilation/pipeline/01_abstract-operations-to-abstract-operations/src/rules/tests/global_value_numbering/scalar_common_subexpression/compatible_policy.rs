//! Compatible-policy keying, evidence, and local/dominating coverage.

use super::super::super::super::SameBlockProofCertifiedCompatiblePolicyScalarCseRule;
use crate::rules::global_value_numbering::{
    compatible_policy_scalar_leader, compatible_policy_scalar_redundant,
};
use crate::rules::tests::fixtures::global_value_numbering::{
    compatible_policy_local_cse_unit, proof_certified_local_cse_unit,
};
use crate::rules::tests::fixtures::id;
use crate::{PsiOptimizationRule, RuleAnalysisView};
use abstract_operations::{
    AbstractOperation as O, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_unit::{
    PsiRewriteCandidate, PsiRewritePatch, recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_local_scalar_common_subexpression_candidate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, ObligationId, OperationId, ScalarType, ValueId,
};

#[test]
fn compatible_policy_key_matrix_binds_sign_width_family_policy_and_operand_order() {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    let operation = id(20_101, OperationId::new);
    let result = id(20_102, ValueId::new);
    let obligation = id(20_103, ObligationId::new);
    let left = id(20_104, ValueId::new);
    let right = id(20_105, ValueId::new);
    let arithmetic = |policy, operator, scalar_type, left, right| O::IntegerArithmetic {
        psi_operation: operation,
        result,
        scalar_type,
        arithmetic: IntegerArithmetic::new(
            policy,
            operator,
            (policy == Policy::Exact).then_some(obligation),
        )
        .expect("admitted integer arithmetic"),
        left,
        right,
    };
    let shift = |policy, direction, value_type, count_type, value, count| O::IntegerShift {
        psi_operation: operation,
        result,
        value_type,
        count_type,
        policy,
        direction,
        value,
        count,
    };
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for bits in [8, 32, 128] {
            let scalar_type = IntegerType::new(sign, bits).unwrap();
            let foreign_domain = IntegerType::new(sign, bits / 2 + 1).unwrap();
            for operator in [Operator::Add, Operator::Subtract, Operator::Multiply] {
                let redundant = arithmetic(Policy::Exact, operator, scalar_type, left, right);
                let redundant_key = compatible_policy_scalar_redundant(&redundant).unwrap().0;
                for policy in [Policy::Wrapping, Policy::Saturating] {
                    let leader = arithmetic(policy, operator, scalar_type, left, right);
                    assert_eq!(
                        compatible_policy_scalar_leader(&leader).unwrap().0,
                        redundant_key
                    );
                }
                let swapped = arithmetic(Policy::Exact, operator, scalar_type, right, left);
                let swapped_key = compatible_policy_scalar_redundant(&swapped).unwrap().0;
                assert_eq!(redundant_key == swapped_key, operator != Operator::Subtract);
                let foreign =
                    arithmetic(Policy::Wrapping, Operator::Add, foreign_domain, left, right);
                assert_ne!(
                    compatible_policy_scalar_leader(&foreign).unwrap().0,
                    redundant_key
                );
            }

            for direction in [IntegerShiftDirection::Left, IntegerShiftDirection::Right] {
                let count_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
                let exact = IntegerShiftPolicy::Exact { obligation };
                let leader = shift(
                    IntegerShiftPolicy::Wrapping,
                    direction,
                    scalar_type,
                    count_type,
                    left,
                    right,
                );
                let redundant = shift(exact, direction, scalar_type, count_type, left, right);
                assert_eq!(
                    compatible_policy_scalar_leader(&leader).unwrap().0,
                    compatible_policy_scalar_redundant(&redundant).unwrap().0
                );
                let reversed = shift(exact, direction, scalar_type, count_type, right, left);
                assert_ne!(
                    compatible_policy_scalar_leader(&leader).unwrap().0,
                    compatible_policy_scalar_redundant(&reversed).unwrap().0
                );
            }
        }
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
