use super::{fold_with, restage_literal, staged_and_inputs, staged_and_ones_inputs, validate};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use selected_instructions::SelectedInstructionKind;
use target::NativeTarget;

#[test]
fn and_ones_fold_rejects_a_literal_that_is_not_all_ones() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the identity element only when it is all ones:
        // `x & 1` is a different computation, and `x & 0` is the sibling
        // family's annihilator — both the producer's declared bound and
        // the replay's re-derived grammar reject either at either `Use`
        // position, keeping the two `BitwiseAndI64` families disjoint on
        // the literal's value.
        for wrong_literal in [1u64, 0] {
            for literal_operand in [0u16, 1u16] {
                let mut inputs = staged_and_ones_inputs(target, literal_operand);
                restage_literal(&mut inputs, wrong_literal);
                assert_eq!(
                    fold_with(
                        &inputs,
                        &environment,
                        LiteralFoldPolicy::BITWISE_AND_ONES_V1
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                    "literal {wrong_literal:#x} at operand {literal_operand} on {target:?}"
                );
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                    "literal {wrong_literal:#x} at operand {literal_operand} on {target:?} replay"
                );
            }
        }
    }
}

#[test]
fn and_ones_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
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
            let mut inputs = staged_and_ones_inputs(target, literal_operand);
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
                    LiteralFoldPolicy::BITWISE_AND_ONES_V1
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
fn and_ones_and_and_zero_families_coexist_under_the_literal_value_key() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    // With both `BitwiseAndI64` families enabled, the literal's value alone
    // selects the grammar at either `Use` position: zero takes the
    // annihilator fold to a materialized zero, all ones takes the identity
    // fold to a copy of the surviving operand. Neither catalog order nor a
    // first-match rule may arbitrate the overlap — the partition must be
    // on the admitted literal.
    let both = LiteralFoldPolicy::BITWISE_AND_ZERO_V1.union(LiteralFoldPolicy::BITWISE_AND_ONES_V1);
    for literal_operand in [0u16, 1u16] {
        let ones = staged_and_ones_inputs(target, literal_operand);
        let ones_result = fold_with(&ones, &environment, both)
            .expect("the and-ones fold applies under both families enabled");
        assert_eq!(ones_result.receipt().applied_count(), 1);
        let action = ones_result.plan().functions[0].action.unwrap();
        assert_eq!(action.immediate, u64::MAX);
        assert_eq!(action.immediate_constraint, keys.copy_i64);
        let rewritten = &ones_result.transformed().functions[0].blocks[0].instructions[0];
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.constraint, keys.copy_i64);

        let zero = staged_and_inputs(target, literal_operand);
        let zero_result = fold_with(&zero, &environment, both)
            .expect("the and-zero fold applies under both families enabled");
        assert_eq!(zero_result.receipt().applied_count(), 1);
        let action = zero_result.plan().functions[0].action.unwrap();
        assert_eq!(action.immediate, 0);
        assert_eq!(action.immediate_constraint, keys.materialize_i64);
        let rewritten = &zero_result.transformed().functions[0].blocks[0].instructions[0];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(0)
            }
        );
        assert_eq!(rewritten.constraint, keys.materialize_i64);
    }
}
