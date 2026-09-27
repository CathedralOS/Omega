use super::{
    BlockZeroTerminator, fold_with, restage_literal,
    staged_saturating_subtract_zero_minuend_carrier_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SaturatingCarrier, SelectedInstructionId,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_a_nonzero_minuend() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The minuend literal fixes the constant zero only when it is
        // exactly zero: a minuend of one or seven is a different
        // computation — `1 -| x` and `7 -| x` do not saturate to a
        // constant — and both the producer's declared bound and the
        // replay's re-derived grammar reject.
        for minuend in [1, 7] {
            let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
                target,
                SaturatingCarrier::U64,
                0,
                BlockZeroTerminator::Jump,
            );
            restage_literal(&mut inputs, minuend);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} minuend {minuend}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} minuend {minuend} replay"
            );
        }
    }
}

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_malformed_operand_arrangements() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The constant-result grammar admits exactly the two `Use` operands
    // and the `Def` result on the three-operand unsigned row — every
    // operand past the result must be a droppable scratch `Def` whose
    // register occurs nowhere else in the function.
    for mutation in 0..4 {
        let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
            target,
            SaturatingCarrier::U64,
            0,
            BlockZeroTerminator::Jump,
        );
        let mut plan = inputs.selected.transformed().clone();
        let consumer = &mut plan.functions[0].blocks[0].instructions[1];
        match mutation {
            // The folded minuend operand is a `Def`, not a `Use`.
            0 => consumer.operands[0].access = RegisterOperandAccess::Def,
            // The dropped subtrahend position is not a `Use`.
            1 => consumer.operands[1].access = RegisterOperandAccess::Def,
            // The result `Def` is missing from the grammar.
            2 => {
                consumer.operands.pop();
            }
            // An operand past the result is a droppable scratch only
            // under occurrence-free custody — a `Def` naming the
            // register operand 1 still reads leaves a surviving use of
            // a definition the rewrite would stop making.
            _ => {
                let mut extra = consumer.operands[0];
                extra.operand = 3;
                extra.access = RegisterOperandAccess::Def;
                extra.virtual_register = VirtualRegisterId(0);
                consumer.operands.push(extra);
            }
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        inputs.selected = selected;
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
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

#[test]
fn saturating_subtract_zero_minuend_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The left-literal grammar exercises the operand-0 fold on the
    // three-operand unsigned row; every recorded decision field must
    // agree with the action the validator independently reconstructs.
    let inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
        target,
        SaturatingCarrier::U64,
        0,
        BlockZeroTerminator::Jump,
    );
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
    )
    .expect("the staged saturating-subtract-zero-minuend fold should validate");

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the subtract's own `Def`,
            // not the dropped subtrahend `Use`.
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            1 => plan.functions[0].action.as_mut().unwrap().result = None,
            // The recorded immediate is the materialized constant zero;
            // any substitution replays differently.
            2 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            // The recorded constraint is the `MaterializeI64` row the
            // zero-minuend policy gate binds; any other key fails the
            // rebuild's binding.
            4 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .immediate_constraint
                    .variant += 1
            }
            5 => plan.functions[0].action = None,
            6 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            7 => plan.usage.candidates += 1,
            // The sibling right-zero policy alone cannot replay the
            // fold: its grammar covers only the operand-1 subtrahend
            // position, so the action reconstructs nothing.
            8 => plan.policy = LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            // The surviving register is the dropped subtrahend:
            // recording the result `Def` register instead fails the
            // re-derived action.
            _ => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(2),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}
