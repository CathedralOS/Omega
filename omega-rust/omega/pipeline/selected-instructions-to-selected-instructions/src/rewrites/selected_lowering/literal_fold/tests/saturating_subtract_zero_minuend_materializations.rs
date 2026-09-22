use super::{
    BlockZeroTerminator, assert_budget_is_enforced, assert_deterministic_fixed_point, fold_with,
    policy_without, policy_without_all, restage_literal,
    staged_saturating_subtract_zero_minuend_carrier_inputs, staged_wrapping_add_inputs,
    staged_xor_inputs, validate,
};
use crate::analyses::validated_machine_effect_catalog;
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SaturatingCarrier, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlanIdentity, SelectedOperand, SelectedTerminator,
    VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;
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
/// one admits the zero-minuend fold: `0 -| x` saturates to the carrier's
/// lower bound, zero, for every `x`.
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
/// `Def` at operand 3 — and none admits the zero-minuend fold: `0 -| x`
/// there is `-x` clamped to the carrier's bounds, not a constant.
fn signed_carriers() -> [SaturatingCarrier; 4] {
    [
        SaturatingCarrier::I8,
        SaturatingCarrier::I16,
        SaturatingCarrier::I32,
        SaturatingCarrier::I64,
    ]
}

#[test]
fn saturating_subtract_zero_minuend_fold_rewrites_every_unsigned_carrier_consumer() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        // The unsigned saturating-subtract row carries the unit effects
        // this family exists to retire: aarch64's flag-setting `subs`
        // realization defines `nzcv` with no clobber or operand mark,
        // while x86-64's form clobbers `rflags` and marks its result
        // early-clobber. Neither declares an implicit use.
        let consumer_row = environment
            .constraint(keys.saturating_subtract_unsigned)
            .unwrap();
        assert_eq!(consumer_row.operands.len(), 3);
        assert!(consumer_row.implicit_uses.is_empty());
        if target == NativeTarget::linux_x64() {
            assert!(
                consumer_row.implicit_defs.is_empty(),
                "x86-64 defines no unit — its rflags surface is a clobber"
            );
            assert_eq!(consumer_row.clobbers.len(), 1);
            assert!(consumer_row.operands[2].early_clobber);
        } else {
            assert_eq!(
                consumer_row.implicit_defs.len(),
                1,
                "aarch64 defines nzcv — the unit the fold must prove dead"
            );
            assert!(consumer_row.clobbers.is_empty());
            assert!(!consumer_row.operands[2].early_clobber);
        }
        for carrier in unsigned_carriers() {
            let inputs =
                staged_saturating_subtract_zero_minuend_carrier_inputs(target, carrier, 0, block0);
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "saturating-subtract-zero-minuend fold on {carrier:?} with the literal at \
                     operand 0 on {target:?} should validate: {error:?}"
                )
            });

            assert_eq!(
                result.plan().machine_effect_catalog,
                effect_catalog.identity()
            );
            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            // The recorded result register is the subtract's own `Def`:
            // the rewritten materialization binds it, not the dropped
            // subtrahend `Use`.
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            // The recorded immediate is the materialized constant zero —
            // `0 -| x` is `0` for every `x` an unsigned carrier admits.
            assert_eq!(action.immediate, 0);
            // The surviving register records the dropped operand-1
            // subtrahend `Use` for custody; the rewritten row binds no
            // `Use` at all.
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, keys.materialize_i64);

            let function = &result.transformed().functions[0];
            // The fold removes only the zero literal and its register:
            // the dropped subtrahend operand and the result stay
            // declared, redensified past the removed victim.
            assert_eq!(function.virtual_registers.len(), 2);
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1);
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0));
            assert_eq!(
                rewritten.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                }
            );
            assert_eq!(rewritten.constraint, keys.materialize_i64);
            // The rebuilt operand list binds only the result `Def` —
            // redensified to `VirtualRegisterId(1)` — from the clean
            // materialize row: the dropped subtrahend `Use` and the
            // consumer's early-clobber mark are gone, and so is every
            // unit effect the saturating form carried.
            // `RetiredWhenDead` retires the aarch64 `nzcv`
            // definition — dead in these fixtures — and the x86-64
            // `rflags` clobber unconditionally, because dropping a
            // clobber only narrows destruction.
            assert_eq!(rewritten.operands.len(), 1);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
            assert_eq!(rewritten.operands[0].fixed_view, None);
            assert!(!rewritten.operands[0].early_clobber);
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            assert!(rewritten.clobbers.is_empty());
            // The folded literal's provenance joins the consumer's; the
            // fieldless saturating-subtract kind carries no obligation
            // custody.
            assert_eq!(rewritten.provenance.operations.len(), 2);
            assert!(rewritten.provenance.obligations.is_empty());

            match &function.blocks[0].terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. } => {
                    assert_eq!(instruction.id, SelectedInstructionId(1));
                }
                SelectedTerminator::Jump { instruction, .. } => {
                    assert_eq!(instruction.id, SelectedInstructionId(1));
                }
                _ => panic!("the staged block-0 terminator is retained"),
            }
            let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator
            else {
                panic!("return terminator retained");
            };
            assert_eq!(instruction.id, SelectedInstructionId(2));

            // The recorded action replays under the independent
            // validator: the same source, the same independently
            // reconstructed decision.
            validate(&inputs, &environment, result.plan().clone())
                .expect("the recorded zero-minuend fold replays");
        }
    }
}

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_every_signed_carrier() {
    // Signed saturation does not make `0 -| x` a constant: it is `-x`
    // clamped to the carrier's bounds, so the family declares no signed
    // pair at all. A staged operand-0 zero literal on a signed carrier
    // names no admitted grammar — under the zero-minuend policy alone
    // the consumer *kind* itself is unadmitted (a consumer mismatch on
    // both paths), while under the union with the sibling identity
    // policy the kind is admitted but no enabled grammar covers the
    // operand-0 position (a future-use mismatch on both paths).
    let both = LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
        .union(LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1);
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in signed_carriers() {
            let inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
                target,
                carrier,
                0,
                BlockZeroTerminator::Jump,
            );
            for (policy, expected) in [
                (
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
                    LiteralFoldError::ConsumerMismatch { function: 0 },
                ),
                (both, LiteralFoldError::FutureUseMismatch { function: 0 }),
            ] {
                assert_eq!(
                    fold_with(&inputs, &environment, policy).map(|_| ()),
                    Err(expected.clone()),
                    "{carrier:?} `0 -| x` on {target:?} under {policy:?}"
                );
            }
            // The replay reads the fixture's recorded policy — the
            // zero-minuend bit alone — where the signed consumer kind is
            // unadmitted.
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{carrier:?} `0 -| x` on {target:?} names no admitted grammar, replay"
            );
            // Replayed under the union policy the same staged consumer is
            // a future-use mismatch: the kind is admitted through the
            // sibling identity family but no enabled grammar covers
            // operand 0.
            let mut plan = inputs.selected.plan().clone();
            plan.policy = both;
            assert_eq!(
                validate(&inputs, &environment, plan).map(|_| ()),
                Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
                "{carrier:?} `0 -| x` on {target:?} under both families, replay"
            );
        }
    }
}

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_the_right_literal_form() {
    // The family folds operand 0 alone: staging the zero literal at
    // operand 1 — the subtrahend position — records a future use at a
    // position the family's grammar does not cover. On an unsigned
    // carrier the kind is admitted and the position names no enabled
    // grammar — a future-use mismatch on both paths. On a signed carrier
    // the kind itself is unadmitted under the zero-minuend policy — a
    // consumer mismatch on both paths, even though operand 1 is the
    // sibling identity family's victim position: that family is not
    // enabled here.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (carrier, expected) in [
            (
                SaturatingCarrier::U64,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
            (
                SaturatingCarrier::I32,
                LiteralFoldError::ConsumerMismatch { function: 0 },
            ),
        ] {
            let inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
                target,
                carrier,
                1,
                BlockZeroTerminator::Jump,
            );
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
                )
                .map(|_| ()),
                Err(expected.clone()),
                "{carrier:?} `x -| 0` staged under the zero-minuend family on {target:?}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(expected),
                "{carrier:?} `x -| 0` staged under the zero-minuend family on {target:?}, replay"
            );
        }
    }
}

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
fn saturating_subtract_zero_minuend_fold_rejects_while_a_terminator_reads_the_defined_unit() {
    // On aarch64 the conditional branch implicitly uses `nzcv`, the very
    // unit the unsigned saturating-subtract row defines: retiring the
    // definition would leave the branch observing stale flags, so the
    // dead-definitions gate — the producer's `admits_dead_consumer_defs`
    // and the replay's independent `dropped_unit_defs_dead` — refuses the
    // fold on both paths. x86-64 keeps folding under the same terminator
    // because `rflags` is only a clobber there: removing a clobber
    // narrows destruction and no reader can go stale.
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let branch = environment.constraint(keys.conditional_branch).unwrap();
    assert!(
        branch.implicit_uses.iter().any(|unit| environment
            .constraint(keys.saturating_subtract_unsigned)
            .unwrap()
            .implicit_defs
            .contains(unit)),
        "the aarch64 branch reads the unit the unsigned subtract row defines"
    );
    for carrier in unsigned_carriers() {
        let inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
            target,
            carrier,
            0,
            BlockZeroTerminator::ConditionalBranch,
        );
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
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
fn saturating_subtract_zero_minuend_fold_rejects_while_another_instruction_reads_the_defined_unit()
{
    // The deadness scan is whole-function and order-insensitive: a reader
    // in a different block — here a block-1 `MaterializeBooleanEqual`,
    // which implicitly uses `nzcv` on aarch64 and `rflags` on x86-64 —
    // keeps the retired unit live just as surely as the terminator does.
    // The same forged reader is legal on x86-64, where the unit the
    // consumer touches is a clobber rather than a definition.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let materialize_boolean = environment.constraint(keys.materialize_boolean).unwrap();
        assert!(
            !materialize_boolean.implicit_uses.is_empty(),
            "the {target:?} materialize-boolean row reads condition state"
        );
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::U32] {
            let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
                target,
                carrier,
                0,
                BlockZeroTerminator::Jump,
            );
            let mut plan = inputs.selected.transformed().clone();
            let function = &mut plan.functions[0];
            let scalar = function.virtual_registers[0].scalar_type;
            let class = function.virtual_registers[0].class;
            // The forged reader's register and instruction must keep the
            // dense identifier domains: the next free register is 3 on
            // the three-register unsigned row, and the staged
            // instruction identifiers are dense — every block
            // contributes its instructions plus one terminator
            // instruction.
            let forged_register =
                VirtualRegisterId(u32::try_from(function.virtual_registers.len()).unwrap());
            let forged_instruction = SelectedInstructionId(
                u32::try_from(
                    function
                        .blocks
                        .iter()
                        .map(|block| block.instructions.len() + 1)
                        .sum::<usize>(),
                )
                .unwrap(),
            );
            function
                .virtual_registers
                .push(selected_instructions::VirtualRegister {
                    id: forged_register,
                    scalar_type: scalar,
                    class,
                    origin: selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: forged_instruction,
                        source_value: semantic_vocabulary::ValueId::new(5).unwrap(),
                    },
                    definition_site: Some(optimization_unit::ValueDefinitionSite::Node {
                        block: semantic_vocabulary::BlockId::new(2).unwrap(),
                        node: 0,
                    }),
                    entry_fixed_view: None,
                });
            function.blocks[1].instructions.push(SelectedInstruction {
                id: forged_instruction,
                kind: SelectedInstructionKind::MaterializeBooleanEqual,
                constraint: materialize_boolean.key,
                operands: vec![SelectedOperand {
                    operand: materialize_boolean.operands[0].operand,
                    virtual_register: forged_register,
                    access: materialize_boolean.operands[0].access,
                    class,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }],
                implicit_uses: materialize_boolean.implicit_uses.clone(),
                implicit_defs: materialize_boolean.implicit_defs.clone(),
                clobbers: materialize_boolean.clobbers.clone(),
                provenance: Default::default(),
            });
            let mut selected = inputs.selected.clone();
            selected.transformed = Arc::new(plan);
            inputs.selected = selected;

            if target == NativeTarget::linux_arm64() {
                assert_eq!(
                    fold_with(
                        &inputs,
                        &environment,
                        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                    "{carrier:?} with a block-1 nzcv reader"
                );
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                    "{carrier:?} with a block-1 nzcv reader, replay"
                );
            } else {
                // The x86-64 consumer defines no unit — its `rflags`
                // surface is a clobber — so an `rflags` reader elsewhere
                // in the function does not keep a dropped definition
                // live.
                let result = fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
                )
                .expect("an rflags reader does not block retiring a clobber");
                assert_eq!(result.receipt().applied_count(), 1);
            }
        }
    }
}

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_a_consumer_implicitly_using_a_unit() {
    // `RetiredWhenDead` forbids the consumer's own implicit uses
    // too: a use the `MaterializeI64` does not carry would silently stop
    // being observed. Forging the branch row's condition-state use onto
    // the consumer — `nzcv` on aarch64, `rflags` on x86-64 — fails the
    // gate on both targets, however dead the definition itself is.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let condition_state = environment
            .constraint(keys.conditional_branch)
            .unwrap()
            .implicit_uses
            .clone();
        assert!(!condition_state.is_empty());
        let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
            target,
            SaturatingCarrier::U64,
            0,
            BlockZeroTerminator::Jump,
        );
        let mut plan = inputs.selected.transformed().clone();
        plan.functions[0].blocks[0].instructions[1]
            .implicit_uses
            .clone_from(&condition_state);
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
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{target:?}"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The left-literal grammar folds operand 0 alone: a fixture with
        // the literal at operand 0 claiming operand 1 or the operand-2
        // `Def` position names no grammar's victim position, and a
        // fixture with the literal physically at operand 1 — the
        // subtrahend position this family does not cover — claiming
        // operand 0 finds the surviving register where the victim must
        // sit, a consumer mismatch under the left-literal grammar.
        for (literal_operand, claimed_operand, expected) in [
            (
                0u16,
                1u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
            (
                0u16,
                2u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
            (
                1u16,
                0u16,
                LiteralFoldError::ConsumerMismatch { function: 0 },
            ),
        ] {
            let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
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
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
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
fn saturating_subtract_zero_minuend_fold_rejects_tied_consumer_operands_but_keeps_the_marks_it_allows()
 {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS` admits `fixed_view` pins and
    // `early_clobber` marks — the bindings constrain only the dropped
    // operand list — while `tied_to` still rejects: a tied register
    // would be a co-allocation the rewrite silently dissolves. The
    // x86-64 row already marks its result early-clobber; the same gate
    // applies on the dropped subtrahend `Use`.
    for mutation in 0..3 {
        let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
            target,
            SaturatingCarrier::U64,
            0,
            BlockZeroTerminator::Jump,
        );
        let mut plan = inputs.selected.transformed().clone();
        let operand = &mut plan.functions[0].blocks[0].instructions[1].operands[1];
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
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "tied dropped operand"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "tied dropped operand, replay"
            );
        } else {
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
            )
            .unwrap_or_else(|error| panic!("mutation {mutation} is admitted: {error:?}"));
            assert_eq!(result.receipt().applied_count(), 1);
            validate(&inputs, &environment, result.plan().clone())
                .expect("the admitted fold replays");
        }
    }
}

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The zero-minuend operand grammar admits the literal only under the
    // saturating-subtract-zero-minuend policy: every selection naming no
    // saturating-subtract family sees no admitted consumer kind —
    // including the strongest posture, every other rule enabled at once
    // with all three saturating-subtract bits closed.
    let inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
        target,
        SaturatingCarrier::U64,
        0,
        BlockZeroTerminator::Jump,
    );
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
        LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
        LiteralFoldPolicy::SATURATING_ADD_ZERO_V1,
        policy_without_all(&[
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
            LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
        ]),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The sibling right-zero identity family admits the same consumer
    // kind at the operand-1 subtrahend position: with only the
    // right-zero bit set — or with every family enabled except the
    // zero-minuend bit — the kind is admitted but no enabled grammar
    // covers the operand-0 minuend position the staged literal occupies:
    // a future-use mismatch, not an unadmitted consumer.
    for policy in [
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
        policy_without(LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
            "{policy:?}"
        );
        assert_eq!(
            validate(&inputs, &environment, {
                let mut plan = inputs.selected.plan().clone();
                plan.policy = policy;
                plan
            })
            .map(|_| ()),
            Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
            "{policy:?}, replay"
        );
    }
    // And the zero-minuend policy admits no other consumer: the
    // wrapping-add and xor fixtures' consumer kinds are no admitted kind.
    let wrapping = staged_wrapping_add_inputs(target, 1);
    assert_eq!(
        fold_with(
            &wrapping,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    let xor = staged_xor_inputs(target, 1);
    assert_eq!(
        fold_with(
            &xor,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn saturating_subtract_zero_minuend_fold_rejects_a_carrier_kind_against_another_row() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A record whose kind names a signed carrier but carries the
    // unsigned row's three operands is refused before the operand
    // grammar is even consulted for admission: no signed carrier names
    // an admitted operand-0 grammar under this family — `0 -| x` there
    // is `-x` clamped, not a constant — so the consumer kind itself is
    // unadmitted under the zero-minuend policy.
    let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
        target,
        SaturatingCarrier::U64,
        0,
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
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    assert_eq!(
        validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );

    // The converse direction: a record whose kind names an unsigned
    // carrier but carries the clamped row's bound scratch `Def` passes
    // the constant-result grammar's scratch-defs custody — the scratch
    // register occurs nowhere else — then fails the independently
    // re-derived effect declaration, which binds no
    // `SaturatingSubtract(U64)` form under the clamped constraint key.
    let mut inputs = staged_saturating_subtract_zero_minuend_carrier_inputs(
        target,
        SaturatingCarrier::I32,
        0,
        BlockZeroTerminator::Jump,
    );
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
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1
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

#[test]
fn saturating_subtract_zero_minuend_fold_reports_and_enforces_its_measured_work() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in unsigned_carriers() {
            let inputs =
                staged_saturating_subtract_zero_minuend_carrier_inputs(target, carrier, 0, block0);
            assert_budget_is_enforced(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
            );
        }
    }
}

#[test]
fn saturating_subtract_zero_minuend_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in unsigned_carriers() {
            let inputs =
                staged_saturating_subtract_zero_minuend_carrier_inputs(target, carrier, 0, block0);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
            );
        }
    }
}
