use super::{
    BlockZeroTerminator, assert_deterministic_fixed_point, fold_with, restage_literal,
    staged_saturating_add_upper_bound_carrier_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{SaturatingCarrier, SelectedInstructionKind, VirtualRegisterId};
use std::sync::Arc;
use target::NativeTarget;

/// The targets and block-0 terminators the fold must hold under. x86-64
/// admits either terminator — its consumer only *clobbers* `rflags`, so a
/// flag-reading branch keeps the fold legal — while aarch64 needs the
/// `Jump` fixture: its consumer *defines* `nzcv`, and the default
/// conditional branch would keep that definition live.
fn positive_fixtures() -> [(NativeTarget, BlockZeroTerminator); 3] {
    [
        (
            NativeTarget::linux_x64(),
            BlockZeroTerminator::ConditionalBranch,
        ),
        (NativeTarget::linux_x64(), BlockZeroTerminator::Jump),
        (NativeTarget::linux_arm64(), BlockZeroTerminator::Jump),
    ]
}

/// The signed carriers bind no pair: `x +| MAX` on a signed carrier is
/// not constant — `MIN +| MAX` is `-1`, not `MAX`.
fn signed_carriers() -> [SaturatingCarrier; 4] {
    [
        SaturatingCarrier::I8,
        SaturatingCarrier::I16,
        SaturatingCarrier::I32,
        SaturatingCarrier::I64,
    ]
}

/// Every unsigned carrier but u64 binds the clamped saturating-add row —
/// two `Use` operands, an early-clobber `Def` result, and a bound scratch
/// `Def` at operand 3 the fold drops under occurrence-free custody.
fn clamped_unsigned_carriers() -> [SaturatingCarrier; 3] {
    [
        SaturatingCarrier::U8,
        SaturatingCarrier::U16,
        SaturatingCarrier::U32,
    ]
}

#[test]
fn saturating_add_upper_bound_fold_rejects_every_signed_carrier() {
    // Signed saturation does not make `x +| MAX` a constant: `MIN +|
    // MAX` is `-1`, so the family declares no signed pair at all. A
    // staged maximum literal on a signed carrier names no admitted
    // grammar — under the upper-bound policy alone the consumer *kind*
    // itself is unadmitted (a consumer mismatch on both paths), while
    // under the union with the sibling identity policy the kind is
    // admitted but the maximum literal fails the zero bound that
    // sibling's grammar carries (an immediate mismatch on both paths).
    let both = LiteralFoldPolicy::SATURATING_ADD_ZERO_V1
        .union(LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1);
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in signed_carriers() {
            for literal_operand in [0u16, 1u16] {
                let inputs = staged_saturating_add_upper_bound_carrier_inputs(
                    target,
                    carrier,
                    literal_operand,
                    BlockZeroTerminator::Jump,
                );
                for (policy, expected) in [
                    (
                        LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1,
                        LiteralFoldError::ConsumerMismatch { function: 0 },
                    ),
                    (both, LiteralFoldError::UnsupportedImmediate { function: 0 }),
                ] {
                    assert_eq!(
                        fold_with(&inputs, &environment, policy).map(|_| ()),
                        Err(expected.clone()),
                        "{carrier:?} `x +| MAX` at operand {literal_operand} on {target:?} under \
                         {policy:?}"
                    );
                }
                // The replay reads the fixture's recorded policy — the
                // upper-bound bit alone — where the signed consumer kind
                // is unadmitted.
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "{carrier:?} `x +| MAX` at operand {literal_operand} names no admitted \
                     grammar, replay"
                );
                // Replayed under the union policy the same staged
                // consumer is an immediate mismatch: the kind is
                // admitted through the sibling identity family but the
                // maximum literal is not its zero.
                let mut plan = inputs.selected.plan().clone();
                plan.policy = both;
                assert_eq!(
                    validate(&inputs, &environment, plan).map(|_| ()),
                    Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                    "{carrier:?} `x +| MAX` at operand {literal_operand} under both families, \
                     replay"
                );
            }
        }
    }
}

#[test]
fn saturating_add_upper_bound_fold_rejects_a_sub_maximum_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal fixes the constant maximum only when it is exactly
        // the carrier's maximum: `x +| 0` is the sibling identity fold —
        // a different computation with a different result — and `x +|
        // MAX - 1` is not constant at all. Both the producer's declared
        // bound and the replay's re-derived grammar reject each, at
        // either `Use` position.
        for literal in [0, SaturatingCarrier::U64.maximum_bits() - 1] {
            for literal_operand in [0u16, 1u16] {
                let mut inputs = staged_saturating_add_upper_bound_carrier_inputs(
                    target,
                    SaturatingCarrier::U64,
                    literal_operand,
                    BlockZeroTerminator::Jump,
                );
                restage_literal(&mut inputs, literal);
                assert_eq!(
                    fold_with(
                        &inputs,
                        &environment,
                        LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                    "{target:?} literal {literal} at operand {literal_operand}"
                );
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                    "{target:?} literal {literal} at operand {literal_operand} replay"
                );
            }
        }
        // Under the union with the sibling identity family the same
        // literal-0 fixture folds — through the *other* descriptor set:
        // the rewritten instruction is the surviving-operand `CopyI64`,
        // not this family's maximum materialization. The families
        // partition the literal values the consumer kind admits.
        let both = LiteralFoldPolicy::SATURATING_ADD_ZERO_V1
            .union(LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1);
        for literal_operand in [0u16, 1u16] {
            let mut inputs = staged_saturating_add_upper_bound_carrier_inputs(
                target,
                SaturatingCarrier::U64,
                literal_operand,
                BlockZeroTerminator::Jump,
            );
            restage_literal(&mut inputs, 0);
            let result = fold_with(&inputs, &environment, both)
                .expect("the zero literal folds through the sibling identity family");
            assert_eq!(result.receipt().applied_count(), 1);
            let function = &result.transformed().functions[0];
            assert_eq!(
                function.blocks[0].instructions[0].kind,
                SelectedInstructionKind::CopyI64,
                "the literal-0 fold is the sibling identity's copy, not a maximum \
                 materialization"
            );
            validate(&inputs, &environment, result.plan().clone())
                .expect("the sibling fold replays under the union policy");
        }
    }
}

#[test]
fn saturating_add_upper_bound_fold_rejects_a_clamped_consumer_whose_scratch_is_not_custodied() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The scratch-defs custody drops the bound scratch `Def` only under
    // occurrence-free custody: the operand list must keep it a `Def`,
    // and its register must occur nowhere else in the function. A `Use`
    // at the tail position has no droppable role; a tail `Def` naming a
    // register another operand position already carries — here the
    // surviving `Use`'s register — leaves a surviving read of a
    // definition the rewrite would stop making.
    for carrier in clamped_unsigned_carriers() {
        for literal_operand in [0u16, 1u16] {
            for mutation in 0..2 {
                let mut inputs = staged_saturating_add_upper_bound_carrier_inputs(
                    target,
                    carrier,
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
                        LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "{carrier:?} literal at operand {literal_operand} mutation {mutation}"
                );
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "{carrier:?} literal at operand {literal_operand} mutation {mutation} replay"
                );
            }
        }
    }
}

#[test]
fn saturating_add_upper_bound_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::U32] {
            for literal_operand in [0u16, 1u16] {
                let inputs = staged_saturating_add_upper_bound_carrier_inputs(
                    target,
                    carrier,
                    literal_operand,
                    block0,
                );
                assert_deterministic_fixed_point(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1,
                );
            }
        }
    }
}
