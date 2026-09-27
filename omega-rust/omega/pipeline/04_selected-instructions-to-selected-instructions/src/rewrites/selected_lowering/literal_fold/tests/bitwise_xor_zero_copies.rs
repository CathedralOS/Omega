use super::{
    assert_deterministic_fixed_point, fold_with, restage_literal, staged_xor_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use target::NativeTarget;

#[test]
fn xor_zero_fold_rejects_a_nonzero_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the identity element only when it is exactly zero:
        // `x ^ 1` is a different computation both the producer's declared
        // bound and the replay's re-derived grammar reject, at either `Use`
        // position.
        for literal_operand in [0u16, 1u16] {
            let mut inputs = staged_xor_inputs(target, literal_operand);
            restage_literal(&mut inputs, 1);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::BITWISE_XOR_ZERO_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "literal at operand {literal_operand} on {target:?}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "literal at operand {literal_operand} on {target:?} replay"
            );
        }
    }
}

#[test]
fn xor_zero_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; claiming the
        // other `Use` position in the recovery classification selects the
        // disjoint grammar, whose operand check finds the surviving register
        // where the victim must sit. Claiming the operand-2 `Def` position
        // names no grammar's victim position at all.
        for (literal_operand, claimed_operand, expected) in [
            (
                0u16,
                1u16,
                LiteralFoldError::ConsumerMismatch { function: 0 },
            ),
            (
                1u16,
                0u16,
                LiteralFoldError::ConsumerMismatch { function: 0 },
            ),
            (
                0u16,
                2u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
            (
                1u16,
                2u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
        ] {
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
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::BITWISE_XOR_ZERO_V1
                )
                .map(|_| ()),
                Err(expected.clone()),
                "literal at operand {literal_operand} claiming {claimed_operand} on {target:?}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(expected),
                "literal at operand {literal_operand} claiming {claimed_operand} on {target:?} \
                 replay"
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
