use super::{
    BlockZeroTerminator, assert_deterministic_fixed_point, fold_with, restage_literal,
    staged_saturating_subtract_upper_bound_carrier_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use selected_instructions::{SaturatingCarrier, SelectedInstructionKind};
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

/// Every unsigned carrier binds the three-operand saturating-subtract row
/// — two `Use` operands and a `Def` result, no scratch tail — and every
/// one admits the upper-bound subtrahend fold: `x -| MAX` saturates to
/// the carrier's lower bound, zero, for every `x`.
fn unsigned_carriers() -> [SaturatingCarrier; 4] {
    [
        SaturatingCarrier::U8,
        SaturatingCarrier::U16,
        SaturatingCarrier::U32,
        SaturatingCarrier::U64,
    ]
}

/// Every signed carrier binds the clamped saturating-subtract row — two
/// `Use` operands, an early-clobber `Def` result, and a bound scratch
/// `Def` at operand 3 — and none admits the upper-bound subtrahend fold:
/// `x -| MAX` there is `x - MAX` clamped to the carrier's lower bound for
/// every negative `x`, not a constant.
fn signed_carriers() -> [SaturatingCarrier; 4] {
    [
        SaturatingCarrier::I8,
        SaturatingCarrier::I16,
        SaturatingCarrier::I32,
        SaturatingCarrier::I64,
    ]
}

#[test]
fn saturating_subtract_upper_bound_fold_rejects_every_signed_carrier() {
    // Signed saturation does not make `x -| MAX` a constant: it is
    // `x - MAX` clamped to the carrier's lower bound, so the family
    // declares no signed pair at all. A staged operand-1 maximum literal
    // on a signed carrier names no admitted grammar — under the
    // upper-bound policy alone the consumer *kind* itself is unadmitted
    // (a consumer mismatch on both paths), while under the union with
    // the sibling right-zero identity policy — which covers every
    // signed carrier at operand 1 — the kind is admitted but the
    // maximum literal fails the zero bound that sibling's grammar
    // carries (an immediate mismatch on both paths).
    let both = LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
        .union(LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1);
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in signed_carriers() {
            let inputs = staged_saturating_subtract_upper_bound_carrier_inputs(
                target,
                carrier,
                1,
                BlockZeroTerminator::Jump,
            );
            for (policy, expected) in [
                (
                    LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
                    LiteralFoldError::ConsumerMismatch { function: 0 },
                ),
                (both, LiteralFoldError::UnsupportedImmediate { function: 0 }),
            ] {
                assert_eq!(
                    fold_with(&inputs, &environment, policy).map(|_| ()),
                    Err(expected.clone()),
                    "{carrier:?} `x -| MAX` on {target:?} under {policy:?}"
                );
            }
            // The replay reads the fixture's recorded policy — the
            // upper-bound bit alone — where the signed consumer kind is
            // unadmitted.
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{carrier:?} `x -| MAX` on {target:?} names no admitted grammar, replay"
            );
            // Replayed under the union policy the same staged consumer is
            // an immediate mismatch: the kind is admitted through the
            // sibling identity family but the maximum literal is not its
            // zero.
            let mut plan = inputs.selected.plan().clone();
            plan.policy = both;
            assert_eq!(
                validate(&inputs, &environment, plan).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{carrier:?} `x -| MAX` on {target:?} under both families, replay"
            );
        }
    }
}

#[test]
fn saturating_subtract_upper_bound_fold_rejects_a_sub_maximum_subtrahend() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The subtrahend literal fixes the constant zero only when it is
        // exactly the carrier's maximum: `x -| 0` is the sibling identity
        // fold — a different computation with a different result — and
        // `x -| MAX - 1` is not constant at all. Both the producer's
        // declared bound and the replay's re-derived grammar reject each.
        for subtrahend in [0, SaturatingCarrier::U64.maximum_bits() - 1] {
            let mut inputs = staged_saturating_subtract_upper_bound_carrier_inputs(
                target,
                SaturatingCarrier::U64,
                1,
                BlockZeroTerminator::Jump,
            );
            restage_literal(&mut inputs, subtrahend);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} subtrahend {subtrahend}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} subtrahend {subtrahend} replay"
            );
        }
        // Under the union with the sibling right-zero identity family the
        // same literal-0 fixture folds — through the *other* descriptor
        // set: the rewritten instruction is the surviving-minuend
        // `CopyI64`, not this family's zero materialization. The families
        // partition the literal values the operand-1 position admits.
        let both = LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
            .union(LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1);
        let mut inputs = staged_saturating_subtract_upper_bound_carrier_inputs(
            target,
            SaturatingCarrier::U64,
            1,
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
            "the literal-0 fold is the sibling identity's copy, not a zero materialization"
        );
        validate(&inputs, &environment, result.plan().clone())
            .expect("the sibling fold replays under the union policy");
    }
}

#[test]
fn saturating_subtract_upper_bound_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The right-literal grammar folds operand 1 alone: a fixture
        // with the literal at operand 1 claiming operand 0 or the
        // operand-2 `Def` position names no grammar's victim position,
        // and a fixture with the literal physically at operand 0 — the
        // minuend position this family does not cover — claiming
        // operand 1 finds the surviving register where the victim must
        // sit, a consumer mismatch under the right-literal grammar.
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
            let mut inputs = staged_saturating_subtract_upper_bound_carrier_inputs(
                target,
                SaturatingCarrier::U64,
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
                    LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1
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
fn saturating_subtract_upper_bound_fold_rejects_a_carrier_kind_against_another_row() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A record whose kind names a signed carrier but carries the
    // unsigned row's three operands is refused before the operand
    // grammar is even consulted for admission: no signed carrier names
    // an admitted operand-1 grammar under this family — `x -| MAX`
    // there is `x - MAX` clamped, not a constant — so the consumer kind
    // itself is unadmitted under the upper-bound policy.
    let mut inputs = staged_saturating_subtract_upper_bound_carrier_inputs(
        target,
        SaturatingCarrier::U64,
        1,
        BlockZeroTerminator::Jump,
    );
    let mut plan = inputs.selected.transformed().clone();
    plan.functions[0].blocks[0].instructions[1].kind =
        SelectedInstructionKind::SaturatingSubtract {
            carrier: SaturatingCarrier::I32,
        };
    let mut selected = inputs.selected.clone();
    selected.transformed = Arc::new(plan);
    inputs.selected = selected;

    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    assert_eq!(
        validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );

    // The converse direction: a record whose kind names an unsigned
    // carrier — restaged to that carrier's own maximum so the immediate
    // bound admits it — but carries the clamped row's bound scratch `Def`
    // passes the constant-result grammar's scratch-defs custody — the
    // scratch register occurs nowhere else — then fails the
    // independently re-derived effect declaration, which binds no
    // `SaturatingSubtract(U64)` form under the clamped constraint key.
    let mut inputs = staged_saturating_subtract_upper_bound_carrier_inputs(
        target,
        SaturatingCarrier::I32,
        1,
        BlockZeroTerminator::Jump,
    );
    restage_literal(&mut inputs, SaturatingCarrier::U64.maximum_bits());
    let mut plan = inputs.selected.transformed().clone();
    plan.functions[0].blocks[0].instructions[1].kind =
        SelectedInstructionKind::SaturatingSubtract {
            carrier: SaturatingCarrier::U64,
        };
    let mut selected = inputs.selected.clone();
    selected.transformed = Arc::new(plan);
    inputs.selected = selected;

    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 })
    );
    assert_eq!(
        validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
        Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 })
    );
}

#[test]
fn saturating_subtract_upper_bound_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in unsigned_carriers() {
            let inputs =
                staged_saturating_subtract_upper_bound_carrier_inputs(target, carrier, 1, block0);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
            );
        }
    }
}
