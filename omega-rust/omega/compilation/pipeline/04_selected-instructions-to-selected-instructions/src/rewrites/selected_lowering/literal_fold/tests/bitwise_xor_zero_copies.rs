use super::{
    assert_classification_does_not_steer, assert_declines, assert_deterministic_fixed_point,
    restage_literal, staged_xor_inputs,
};
use crate::LiteralFoldPolicy;
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use target::NativeTarget;

#[test]
fn xor_zero_fold_declines_a_nonzero_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the identity element only when it is exactly zero:
        // `x ^ 1` is a different computation both the producer's declared
        // bound and the replay's re-derived grammar reject, at either `Use`
        // position.
        for literal_operand in [0u16, 1u16] {
            let mut inputs = staged_xor_inputs(target, literal_operand);
            restage_literal(&mut inputs, 1);
            assert_declines(
                &inputs,
                &environment,
                LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
                &format!("literal at operand {literal_operand} on {target:?}"),
            );
        }
    }
}

#[test]
fn xor_zero_fold_ignores_a_classification_claiming_another_operand() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; the
        // recovery classification claims `claimed_operand` instead. The
        // classification no longer nominates, so the claim neither
        // redirects nor suppresses the fold the stream itself admits.
        for (literal_operand, claimed_operand) in
            [(0u16, 1u16), (1u16, 0u16), (0u16, 2u16), (1u16, 2u16)]
        {
            let mut inputs = staged_xor_inputs(target, literal_operand);
            let RecoveryClassification::ImmediateU64RematerializationCandidate {
                future_uses, ..
            } = &mut inputs.recovery.plan.functions[0]
                .classification
                .as_mut()
                .unwrap()
                .classification
            else {
                unreachable!()
            };
            future_uses[0].operand = claimed_operand;
            assert_classification_does_not_steer(
                &inputs,
                &environment,
                LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
                &format!(
                    "literal at operand {literal_operand} claiming {claimed_operand} on {target:?}"
                ),
            );
        }
    }
}

#[test]
fn xor_zero_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_xor_inputs(target, literal_operand);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
            );
        }
    }
}
