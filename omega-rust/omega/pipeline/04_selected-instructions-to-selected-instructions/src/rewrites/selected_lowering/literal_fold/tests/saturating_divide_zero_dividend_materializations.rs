use super::{
    BlockZeroTerminator, fold_with, restage_literal,
    staged_saturating_divide_zero_dividend_carrier_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SaturatingCarrier, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedOperand, VirtualRegisterId,
};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn saturating_divide_zero_dividend_fold_rejects_a_non_zero_dividend() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the zero dividend only under the constant fold:
        // a dividend of one or seven is a different computation both the
        // producer's declared bound and the replay's re-derived grammar
        // reject — and a dividend of one does not become the sibling
        // divisor-one fold by value, which binds the divisor position.
        for dividend in [1, 7] {
            let mut inputs = staged_saturating_divide_zero_dividend_carrier_inputs(
                target,
                SaturatingCarrier::U64,
                0,
                BlockZeroTerminator::Jump,
            );
            restage_literal(&mut inputs, dividend);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} dividend {dividend}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} dividend {dividend} replay"
            );
        }
    }
}

#[test]
fn saturating_divide_zero_dividend_fold_rejects_a_scratch_def_without_dead_custody() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    // aarch64's clamped signed row writes the bound scratch `Def` at
    // operand 3, droppable only under occurrence-free custody: the
    // operand list must keep it a `Def`, and its register must occur
    // nowhere else in the function. A `Use` or `UseDef` at the tail
    // position has no droppable role — the `Use` custody asks for a
    // zero-materializing definition the consumer's own scratch register
    // does not have — and a tail `Def` naming a register another operand
    // position already carries leaves a surviving read of a definition
    // the rewrite would stop making.
    for mutation in 0..3 {
        let mut inputs = staged_saturating_divide_zero_dividend_carrier_inputs(
            target,
            SaturatingCarrier::I32,
            0,
            BlockZeroTerminator::Jump,
        );
        let mut plan = inputs.selected.transformed().clone();
        let consumer = &mut plan.functions[0].blocks[0].instructions[1];
        match mutation {
            0 => consumer.operands[3].access = RegisterOperandAccess::Use,
            1 => consumer.operands[3].access = RegisterOperandAccess::UseDef,
            _ => consumer.operands[3].virtual_register = VirtualRegisterId(0),
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        inputs.selected = selected;
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1
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
fn saturating_divide_zero_dividend_fold_rejects_while_another_instruction_reads_the_defined_unit() {
    // The deadness scan is whole-function and order-insensitive: a reader
    // in a different block — here a block-1 `MaterializeBooleanEqual`,
    // which implicitly uses `nzcv` on aarch64 and `rflags` on x86-64 —
    // keeps the retired unit live just as surely as the terminator does.
    // The same forged reader is legal wherever the consumer merely
    // clobbers the unit — x86-64 on either carrier — or defines none at
    // all — aarch64's unsigned `udiv`.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let materialize_boolean = environment.constraint(keys.materialize_boolean).unwrap();
        assert!(
            !materialize_boolean.implicit_uses.is_empty(),
            "the {target:?} materialize-boolean row reads condition state"
        );
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let mut inputs = staged_saturating_divide_zero_dividend_carrier_inputs(
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
            // the three-register aarch64 unsigned row and 4 on the
            // four-register rows, and the staged instruction identifiers
            // are dense — every block contributes its instructions plus
            // one terminator instruction.
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

            if target == NativeTarget::linux_arm64() && carrier.is_signed() {
                assert_eq!(
                    fold_with(
                        &inputs,
                        &environment,
                        LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1
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
                // x86-64's consumer defines no unit — its `rflags`
                // surface is a clobber — and aarch64's unsigned `udiv`
                // defines none either, so a condition-state reader
                // elsewhere in the function does not keep a dropped
                // definition live.
                let result = fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1,
                )
                .expect("a reader of a unit the consumer does not define cannot block the fold");
                assert_eq!(result.receipt().applied_count(), 1);
            }
        }
    }
}

#[test]
fn saturating_divide_zero_dividend_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The left-literal grammar folds operand 0 alone: a fixture with
        // the literal at operand 0 claiming operand 1 or the operand-2
        // `Def` position names no grammar's victim position, and a
        // fixture with the literal physically at operand 1 — the refused
        // `x /| 0` arrangement — claiming operand 0 finds the surviving
        // register where the victim must sit, a consumer mismatch under
        // the left-literal grammar.
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
            let mut inputs = staged_saturating_divide_zero_dividend_carrier_inputs(
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
                    LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1
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
