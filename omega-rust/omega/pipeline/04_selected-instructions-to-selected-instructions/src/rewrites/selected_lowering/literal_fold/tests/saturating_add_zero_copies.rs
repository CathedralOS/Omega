use super::{
    BlockZeroTerminator, fold_with, staged_saturating_add_carrier_inputs,
    staged_saturating_add_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{SaturatingCarrier, VirtualRegisterId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn saturating_add_zero_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; claiming
        // the other `Use` position in the recovery classification selects
        // the disjoint grammar, whose operand check finds the surviving
        // register where the victim must sit. Claiming the operand-2 `Def`
        // position names no grammar's victim position at all.
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
            let mut inputs =
                staged_saturating_add_inputs(target, literal_operand, BlockZeroTerminator::Jump);
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
                    LiteralFoldPolicy::SATURATING_ADD_ZERO_V1
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
fn saturating_add_zero_fold_rejects_tied_consumer_operands_but_keeps_the_marks_it_allows() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS` admits `fixed_view` pins and
    // `early_clobber` marks — the bindings constrain only the dropped
    // operand list — while `tied_to` still rejects: a tied register would
    // be a co-allocation the rewrite silently dissolves. The clamped row
    // already marks both `Def`s early-clobber; the same gate applies on
    // its surviving `Use`.
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        for literal_operand in [0u16, 1u16] {
            let surviving_position = usize::from(1 - literal_operand);
            for mutation in 0..3 {
                let mut inputs = staged_saturating_add_carrier_inputs(
                    target,
                    carrier,
                    literal_operand,
                    BlockZeroTerminator::Jump,
                );
                let mut plan = inputs.selected.transformed().clone();
                let operand =
                    &mut plan.functions[0].blocks[0].instructions[1].operands[surviving_position];
                match mutation {
                    0 => operand.fixed_view = Some(register_model::RegisterViewId(0)),
                    1 => operand.tied_to = Some(0),
                    _ => operand.early_clobber = true,
                }
                let mut selected = inputs.selected.clone();
                selected.transformed = Arc::new(plan);
                inputs.selected = selected;

                if mutation == 1 {
                    assert_eq!(
                        fold_with(
                            &inputs,
                            &environment,
                            LiteralFoldPolicy::SATURATING_ADD_ZERO_V1
                        )
                        .map(|_| ()),
                        Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                        "{carrier:?} literal at operand {literal_operand} tied surviving operand"
                    );
                    assert_eq!(
                        validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                        Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                        "{carrier:?} literal at operand {literal_operand} tied surviving \
                         operand, replay"
                    );
                } else {
                    let result = fold_with(
                        &inputs,
                        &environment,
                        LiteralFoldPolicy::SATURATING_ADD_ZERO_V1,
                    )
                    .unwrap_or_else(|error| {
                        panic!(
                            "{carrier:?} literal at operand {literal_operand} mutation \
                             {mutation} is admitted: {error:?}"
                        )
                    });
                    assert_eq!(result.receipt().applied_count(), 1);
                    validate(&inputs, &environment, result.plan().clone())
                        .expect("the admitted fold replays");
                }
            }
        }
    }
}

#[test]
fn saturating_add_zero_fold_rejects_a_clamped_consumer_whose_scratch_is_not_custodied() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The scratch-defs grammar drops the bound scratch `Def` under
    // occurrence-free custody: the operand list must keep it a `Def`,
    // and its register must occur nowhere else in the function. A `Use`
    // at the tail position has no droppable role; a tail `Def` naming a
    // register another operand position already carries — here the
    // surviving `Use`'s register — leaves a surviving read of a
    // definition the rewrite would stop making.
    for literal_operand in [0u16, 1u16] {
        for mutation in 0..2 {
            let mut inputs = staged_saturating_add_carrier_inputs(
                target,
                SaturatingCarrier::I32,
                literal_operand,
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
                    LiteralFoldPolicy::SATURATING_ADD_ZERO_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} mutation {mutation}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} mutation {mutation} replay"
            );
        }
    }
}
