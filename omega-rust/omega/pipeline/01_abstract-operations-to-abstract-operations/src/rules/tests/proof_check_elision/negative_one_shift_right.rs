//! Exact signed negative-one right-shift tests.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule;
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::dead_scalar_elimination::discard_scalar_function_result;
use crate::rules::tests::fixtures::proof_check_elision::{
    live_exact_signed_negative_one_shift_right_unit, live_proof_binary_identity_unit,
};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationFact, recompute_psi_optimization_unit_identity};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

#[test]
fn proof_certified_exact_signed_negative_one_shift_right_declines_other_shapes() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let contract =
        LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule::contract();

    let mut non_negative_one = live_exact_signed_negative_one_shift_right_unit(signed);
    let O::IntegerConstant { value, .. } =
        &mut non_negative_one.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Signed(-2);
    let OptimizationFact::IntegerConstant { constant, .. } =
        &mut non_negative_one.functions[0].facts[0]
    else {
        unreachable!()
    };
    *constant = IntegerValue::Signed(-2);
    non_negative_one.identity = recompute_psi_optimization_unit_identity(&non_negative_one);

    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned = live_proof_binary_identity_unit(
        unsigned,
        IntegerValue::Unsigned(u8::MAX.into()),
        true,
        |psi_operation, obligation, result, value_type, value, count| O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type: unsigned,
            value,
            count,
        },
    );
    let left_shift = live_proof_binary_identity_unit(
        signed,
        IntegerValue::Signed(-1),
        true,
        |psi_operation, obligation, result, value_type, value, count| O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type: signed,
            value,
            count,
        },
    );
    let wrapping = live_proof_binary_identity_unit(
        signed,
        IntegerValue::Signed(-1),
        true,
        |psi_operation, _obligation, result, value_type, value, count| {
            O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                count_type: signed,
                value,
                count,
            }
        },
    );
    let dead =
        discard_scalar_function_result(live_exact_signed_negative_one_shift_right_unit(signed));
    let mut missing_proof = live_exact_signed_negative_one_shift_right_unit(signed);
    missing_proof.accepted_obligation_facts.clear();
    missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
    let mut missing_active_reference = live_exact_signed_negative_one_shift_right_unit(signed);
    missing_active_reference.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    missing_active_reference.identity =
        recompute_psi_optimization_unit_identity(&missing_active_reference);

    for unit in [
        non_negative_one,
        unsigned,
        left_shift,
        wrapping,
        dead,
        missing_proof,
        missing_active_reference,
    ] {
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}
