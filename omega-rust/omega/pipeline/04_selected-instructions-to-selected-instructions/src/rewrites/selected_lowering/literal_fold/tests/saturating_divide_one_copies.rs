use super::{
    BlockZeroTerminator, assert_deterministic_fixed_point, fold_with, restage_literal,
    staged_saturating_divide_carrier_inputs, staged_saturating_divide_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{SaturatingCarrier, SelectedInstructionKind, VirtualRegisterId};
use semantic_vocabulary::IntegerValue;
use std::sync::Arc;
use target::NativeTarget;

/// The signed-carrier fixtures. aarch64's clamped signed row *defines*
/// `nzcv`, so its positive cases need the `Jump` terminator — a
/// flag-reading branch would keep the retired definition live — while
/// x86-64's `idiv` only *clobbers* `rdx`/`rflags`, so the flag-reading
/// branch stays legal there.
fn signed_fixtures() -> [(NativeTarget, BlockZeroTerminator); 3] {
    [
        (
            NativeTarget::linux_x64(),
            BlockZeroTerminator::ConditionalBranch,
        ),
        (NativeTarget::linux_x64(), BlockZeroTerminator::Jump),
        (NativeTarget::linux_arm64(), BlockZeroTerminator::Jump),
    ]
}

#[test]
fn saturating_divide_one_fold_rejects_a_non_one_divisor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the right divisor identity only when it is
        // exactly one: `x /| 2` is a different computation both the
        // producer's declared bound and the replay's re-derived grammar
        // reject — and `x /| 0` rejects the same way even though a
        // saturating divide by zero clamps rather than faults.
        for divisor in [0, 2] {
            let mut inputs = staged_saturating_divide_inputs(target, 1, BlockZeroTerminator::Jump);
            restage_literal(&mut inputs, divisor);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} divisor {divisor}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} divisor {divisor} replay"
            );
        }
    }
}

#[test]
fn saturating_divide_one_fold_rejects_an_auxiliary_operand_without_zero_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Both x86-64 divide rows — the unsigned `div` and the signed `idiv`
    // — read the zeroed high-half dividend through the operand-3 `Use`.
    // A `Use` operand past the result is droppable only when its register
    // is defined solely by zero materializations: a nonzero
    // materialization, a non-materialize definition, a `Def` or `UseDef`
    // at the tail position, and a register no instruction defines each
    // reject — the producer and the independent replay alike.
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        for mutation in 0..5 {
            let mut inputs = staged_saturating_divide_carrier_inputs(
                target,
                carrier,
                1,
                BlockZeroTerminator::Jump,
            );
            let mut plan = inputs.selected.transformed().clone();
            let instructions = &mut plan.functions[0].blocks[0].instructions;
            match mutation {
                // The high-half scratch materializes seven, not zero.
                0 => {
                    instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    };
                }
                // The scratch register is defined by a copy, not a zero
                // materialization at all.
                1 => instructions[0].kind = SelectedInstructionKind::CopyI64,
                // The tail operand defines a register rather than reading
                // the proven-zero scratch — and the register it names is
                // the auxiliary's own, which a surviving definition would
                // still observe.
                2 => instructions[2].operands[3].access = RegisterOperandAccess::Def,
                // A `UseDef` tail names neither droppable custody.
                3 => instructions[2].operands[3].access = RegisterOperandAccess::UseDef,
                // The tail operand reads the entry-parameter register no
                // instruction defines — not a zero materialization.
                _ => instructions[2].operands[3].virtual_register = VirtualRegisterId(0),
            }
            let mut selected = inputs.selected.clone();
            selected.transformed = Arc::new(plan);
            inputs.selected = selected;

            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{carrier:?} mutation {mutation}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{carrier:?} mutation {mutation} replay"
            );
        }
    }
}

#[test]
fn saturating_divide_one_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for (target, block0) in signed_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let inputs = staged_saturating_divide_carrier_inputs(target, carrier, 1, block0);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
            );
        }
    }
}
