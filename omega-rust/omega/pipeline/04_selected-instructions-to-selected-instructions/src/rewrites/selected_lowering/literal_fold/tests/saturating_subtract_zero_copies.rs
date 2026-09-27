use super::{
    BlockZeroTerminator, fold_with, staged_saturating_subtract_carrier_inputs,
    staged_saturating_subtract_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{SaturatingCarrier, VirtualRegisterId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn saturating_subtract_zero_fold_rejects_while_a_terminator_reads_the_defined_unit() {
    // On aarch64 the conditional branch implicitly uses `nzcv`, the very
    // unit the saturating subtract defines on every carrier: retiring the
    // definition would leave the branch observing stale flags, so the
    // dead-definitions gate — the producer's `admits_dead_consumer_defs`
    // and the replay's independent `dropped_unit_defs_dead` — refuses the
    // fold on both paths. x86-64 keeps folding under the same terminator
    // because `rflags` is only a clobber there: removing a clobber narrows
    // destruction and no reader can go stale.
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let branch = environment.constraint(keys.conditional_branch).unwrap();
    for key in [
        keys.saturating_subtract_unsigned,
        keys.saturating_subtract_clamped,
    ] {
        assert!(
            branch.implicit_uses.iter().any(|unit| environment
                .constraint(key)
                .unwrap()
                .implicit_defs
                .contains(unit)),
            "the aarch64 branch reads the unit each saturating-subtract row defines"
        );
    }
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        let inputs = staged_saturating_subtract_carrier_inputs(
            target,
            carrier,
            1,
            BlockZeroTerminator::ConditionalBranch,
        );
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
            )
            .map(|_| ()),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{carrier:?} with a live nzcv reader"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{carrier:?} with a live nzcv reader, replay"
        );
    }
}

#[test]
fn saturating_subtract_zero_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The right-literal grammar folds operand 1 alone: a fixture with
        // the literal at operand 1 claiming operand 0 or the operand-2
        // `Def` position names no grammar's victim position, and a fixture
        // with the literal physically at operand 0 — the refused `0 -| x`
        // arrangement — claiming operand 1 finds the surviving register
        // where the victim must sit, a consumer mismatch under the
        // right-literal grammar.
        for (literal_operand, claimed_operand, expected) in [
            (
                1u16,
                0u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
            (
                1u16,
                2u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
            (
                0u16,
                1u16,
                LiteralFoldError::ConsumerMismatch { function: 0 },
            ),
        ] {
            let mut inputs = staged_saturating_subtract_inputs(
                target,
                literal_operand,
                BlockZeroTerminator::Jump,
            );
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
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
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
fn saturating_subtract_zero_fold_rejects_a_clamped_consumer_whose_scratch_is_not_custodied() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The scratch-defs grammar drops the bound scratch `Def` under
    // occurrence-free custody: the operand list must keep it a `Def`,
    // and its register must occur nowhere else in the function. A `Use`
    // at the tail position has no droppable role; a tail `Def` naming a
    // register another operand position already carries — here the
    // surviving `Use`'s register — leaves a surviving read of a
    // definition the rewrite would stop making.
    for mutation in 0..2 {
        let mut inputs = staged_saturating_subtract_carrier_inputs(
            target,
            SaturatingCarrier::I32,
            1,
            BlockZeroTerminator::Jump,
        );
        let mut plan = inputs.selected.transformed().clone();
        let consumer = &mut plan.functions[0].blocks[0].instructions[1];
        match mutation {
            0 => consumer.operands[3].access = RegisterOperandAccess::Use,
            _ => consumer.operands[3].virtual_register = VirtualRegisterId(0),
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        inputs.selected = selected;
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
            )
            .map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation} replay"
        );
    }
}
