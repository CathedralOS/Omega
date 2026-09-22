use super::{
    BlockZeroTerminator, assert_budget_is_enforced, assert_deterministic_fixed_point, fold_with,
    policy_without_all, restage_literal, staged_saturating_add_inputs,
    staged_saturating_subtract_carrier_inputs, staged_saturating_subtract_inputs,
    staged_wrapping_add_inputs, staged_xor_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy, validated_machine_effect_catalog};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SaturatingCarrier, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlanIdentity, SelectedOperand, SelectedTerminator,
    VirtualRegisterId,
};
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
/// — two `Use` operands and a `Def` result, no scratch tail.
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
/// `Def` at operand 3 the fold drops under occurrence-free custody.
fn signed_carriers() -> [SaturatingCarrier; 4] {
    [
        SaturatingCarrier::I8,
        SaturatingCarrier::I16,
        SaturatingCarrier::I32,
        SaturatingCarrier::I64,
    ]
}

#[test]
fn saturating_subtract_zero_fold_rewrites_the_consumer_to_a_surviving_operand_copy() {
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
            let inputs = staged_saturating_subtract_carrier_inputs(target, carrier, 1, block0);
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "saturating-subtract-zero fold on {carrier:?} with the literal at operand 1 on \
                     {target:?} should validate: {error:?}"
                )
            });

            assert_eq!(
                result.plan().machine_effect_catalog,
                effect_catalog.identity()
            );
            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            // The recorded immediate is the folded literal itself — zero
            // — under the right-literal grammar; the `CopyI64` rewrite
            // binds the surviving register rather than embedding a
            // constant.
            assert_eq!(action.immediate, 0);
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, keys.copy_i64);

            let function = &result.transformed().functions[0];
            // The fold removes only the zero literal and its register:
            // the surviving operand and the result stay declared,
            // redensified past the removed victim.
            assert_eq!(function.virtual_registers.len(), 2);
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1);
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0));
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys.copy_i64);
            // The rebuilt operand list binds the surviving `Use` and the
            // result `Def` — redensified to `VirtualRegisterId(1)` — from
            // the clean copy row: the consumer's early-clobber mark is
            // gone, and so is every unit effect the saturating form
            // carried. `RetiredWhenDead` retires the aarch64 `nzcv`
            // definition — dead in these fixtures — and the x86-64
            // `rflags` clobber unconditionally, because dropping a clobber
            // only narrows destruction.
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert!(!rewritten.operands[1].early_clobber);
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
        }
    }
}

#[test]
fn saturating_subtract_zero_fold_rewrites_every_signed_carrier_consumer() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        // The clamped row continues past the `Def` result with the bound
        // scratch `Def` the realization computes its saturation bound
        // through — operand 3, early-clobber on both targets — and its
        // unit surface is the family's own: aarch64 defines `nzcv`,
        // x86-64 clobbers `rflags`, and neither declares an implicit use.
        let consumer_row = environment
            .constraint(keys.saturating_subtract_clamped)
            .unwrap();
        assert_eq!(consumer_row.operands.len(), 4);
        assert_eq!(consumer_row.operands[3].access, RegisterOperandAccess::Def);
        assert!(consumer_row.operands[2].early_clobber);
        assert!(consumer_row.operands[3].early_clobber);
        assert!(consumer_row.implicit_uses.is_empty());
        if target == NativeTarget::linux_x64() {
            assert!(consumer_row.implicit_defs.is_empty());
            assert_eq!(consumer_row.clobbers.len(), 1);
        } else {
            assert_eq!(consumer_row.implicit_defs.len(), 1);
            assert!(consumer_row.clobbers.is_empty());
        }
        for carrier in signed_carriers() {
            let inputs = staged_saturating_subtract_carrier_inputs(target, carrier, 1, block0);
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "saturating-subtract-zero fold on {carrier:?} with the literal at operand 1 on \
                     {target:?} should validate: {error:?}"
                )
            });

            assert_eq!(
                result.plan().machine_effect_catalog,
                effect_catalog.identity()
            );
            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            assert_eq!(action.immediate, 0);
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, keys.copy_i64);

            let function = &result.transformed().functions[0];
            // The fold removes only the zero literal and its register:
            // the surviving operand, the result, and the dropped
            // scratch output stay declared, redensified past the
            // removed victim — the scratch entry carries no operand
            // occurrence past the rewrite.
            assert_eq!(function.virtual_registers.len(), 3);
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1);
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0));
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys.copy_i64);
            // The rebuilt operand list binds only the surviving `Use`
            // and the result `Def` — the clamped row's bound scratch
            // is gone with its early-clobber mark, and so is every
            // unit effect the saturating form carried.
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert!(!rewritten.operands[1].early_clobber);
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            assert!(rewritten.clobbers.is_empty());
            assert_eq!(rewritten.provenance.operations.len(), 2);
            assert!(rewritten.provenance.obligations.is_empty());
        }
    }
}

#[test]
fn saturating_subtract_zero_fold_rejects_the_left_literal_form() {
    // Saturating subtraction does not commute: `0 -| x` is `-x` clamped
    // to the carrier's bounds, not `x`, so the family declares no
    // left-literal grammar at all. Staging the zero literal at operand 0
    // — the consumer's own `Use` position for its left input — records a
    // future use at a position no admitted shape folds: both the
    // producer's descriptor selection and the replay's independently
    // re-derived operand-1 victim position refuse it as a future-use
    // mismatch, on the plain unsigned row and the signed scratch-defs
    // row alike.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let inputs = staged_saturating_subtract_carrier_inputs(
                target,
                carrier,
                0,
                BlockZeroTerminator::Jump,
            );
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
                "{carrier:?} `0 -| x` on {target:?} names no admitted grammar"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
                "{carrier:?} `0 -| x` on {target:?} names no admitted grammar, replay"
            );
        }
    }
}

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
fn saturating_subtract_zero_fold_rejects_while_another_instruction_reads_the_defined_unit() {
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
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let mut inputs = staged_saturating_subtract_carrier_inputs(
                target,
                carrier,
                1,
                BlockZeroTerminator::Jump,
            );
            let mut plan = inputs.selected.transformed().clone();
            let function = &mut plan.functions[0];
            let scalar = function.virtual_registers[0].scalar_type;
            let class = function.virtual_registers[0].class;
            // The forged reader's register must keep the dense identifier
            // domain: the next free register is 3 on the three-register
            // unsigned row and 4 on the clamped row's extra scratch entry.
            let forged_register =
                VirtualRegisterId(u32::try_from(function.virtual_registers.len()).unwrap());
            function
                .virtual_registers
                .push(selected_instructions::VirtualRegister {
                    id: forged_register,
                    scalar_type: scalar,
                    class,
                    origin: selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(4),
                        source_value: semantic_vocabulary::ValueId::new(4).unwrap(),
                    },
                    definition_site: Some(optimization_unit::ValueDefinitionSite::Node {
                        block: semantic_vocabulary::BlockId::new(2).unwrap(),
                        node: 0,
                    }),
                    entry_fixed_view: None,
                });
            function.blocks[1].instructions.push(SelectedInstruction {
                id: SelectedInstructionId(4),
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
                        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
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
                // in the function does not keep a dropped definition live.
                let result = fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
                )
                .expect("an rflags reader does not block retiring a clobber");
                assert_eq!(result.receipt().applied_count(), 1);
            }
        }
    }
}

#[test]
fn saturating_subtract_zero_fold_rejects_a_consumer_implicitly_using_a_unit() {
    // `RetiredWhenDead` forbids the consumer's own implicit uses too:
    // a use the `CopyI64` does not carry would silently stop being
    // observed. Forging the branch row's condition-state use onto the
    // consumer — `nzcv` on aarch64, `rflags` on x86-64 — fails the gate on
    // both targets, however dead the definition itself is.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let condition_state = environment
            .constraint(keys.conditional_branch)
            .unwrap()
            .implicit_uses
            .clone();
        assert!(!condition_state.is_empty());
        let mut inputs = staged_saturating_subtract_inputs(target, 1, BlockZeroTerminator::Jump);
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
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
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
fn saturating_subtract_zero_fold_rejects_a_nonzero_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the identity element only when it is exactly
        // zero: `x -| 1` is a different computation both the producer's
        // declared bound and the replay's re-derived grammar reject.
        let mut inputs = staged_saturating_subtract_inputs(target, 1, BlockZeroTerminator::Jump);
        restage_literal(&mut inputs, 1);
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
            )
            .map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?} replay"
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
fn saturating_subtract_zero_fold_rejects_malformed_operand_arrangements() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The copy grammar admits exactly the two `Use` operands and the `Def`
    // result — unlike the constant-result grammars it binds the surviving
    // operand, so no operand past the result has a droppable role on the
    // three-operand unsigned row.
    for mutation in 0..4 {
        let mut inputs = staged_saturating_subtract_inputs(target, 1, BlockZeroTerminator::Jump);
        let mut plan = inputs.selected.transformed().clone();
        let consumer = &mut plan.functions[0].blocks[0].instructions[1];
        match mutation {
            // The folded operand is a `Def`, not a `Use`.
            0 => consumer.operands[1].access = RegisterOperandAccess::Def,
            // The surviving position is not a `Use`.
            1 => consumer.operands[0].access = RegisterOperandAccess::Def,
            // The result `Def` is missing from the grammar.
            2 => {
                consumer.operands.pop();
            }
            // An operand past the result is no droppable scratch under
            // the copy grammar — the operand list must be exactly the
            // two `Use`s and the `Def`.
            _ => {
                let mut extra = consumer.operands[0];
                extra.operand = 3;
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

#[test]
fn saturating_subtract_zero_fold_rejects_tied_consumer_operands_but_keeps_the_marks_it_allows() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS` admits `fixed_view` pins and
    // `early_clobber` marks — the bindings constrain only the dropped
    // operand list — while `tied_to` still rejects: a tied register would
    // be a co-allocation the rewrite silently dissolves. The clamped row
    // already marks both `Def`s early-clobber; the same gate applies on
    // its surviving `Use`.
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        for mutation in 0..3 {
            let mut inputs = staged_saturating_subtract_carrier_inputs(
                target,
                carrier,
                1,
                BlockZeroTerminator::Jump,
            );
            let mut plan = inputs.selected.transformed().clone();
            let operand = &mut plan.functions[0].blocks[0].instructions[1].operands[0];
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
                        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "{carrier:?} tied surviving operand"
                );
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "{carrier:?} tied surviving operand, replay"
                );
            } else {
                let result = fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
                )
                .unwrap_or_else(|error| {
                    panic!("{carrier:?} mutation {mutation} is admitted: {error:?}")
                });
                assert_eq!(result.receipt().applied_count(), 1);
                validate(&inputs, &environment, result.plan().clone())
                    .expect("the admitted fold replays");
            }
        }
    }
}

#[test]
fn saturating_subtract_zero_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The saturating-subtract operand grammar admits the literal only
    // under a saturating-subtract policy: every selection naming no
    // saturating-subtract family sees no admitted consumer kind —
    // including its sibling saturating-add family and the strongest
    // posture, every other rule enabled at once with all three
    // saturating-subtract bits closed.
    let inputs = staged_saturating_subtract_inputs(target, 1, BlockZeroTerminator::Jump);
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
    // The sibling zero-minuend family admits the same consumer kind at
    // the operand-0 minuend position — on the unsigned carriers only:
    // with every family enabled except the two operand-1 bits — the
    // right-zero identity and the upper-bound subtrahend — the kind is
    // admitted but no enabled grammar covers the operand-1 subtrahend
    // position the staged literal occupies: a future-use mismatch, not
    // an unadmitted consumer.
    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            policy_without_all(&[
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
                LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
            ])
        )
        .map(|_| ()),
        Err(LiteralFoldError::FutureUseMismatch { function: 0 })
    );
    // And the saturating-subtract-zero policy admits no other consumer:
    // the saturating-add, wrapping-add, and xor fixtures' consumer kinds
    // are no admitted kind.
    let saturating_add = staged_saturating_add_inputs(target, 1, BlockZeroTerminator::Jump);
    assert_eq!(
        fold_with(
            &saturating_add,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    let wrapping = staged_wrapping_add_inputs(target, 1);
    assert_eq!(
        fold_with(
            &wrapping,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    let xor = staged_xor_inputs(target, 1);
    assert_eq!(
        fold_with(
            &xor,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn saturating_subtract_zero_fold_rejects_a_carrier_kind_against_another_row() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Each carrier's kind admits only the operand grammar its own row
    // declares. A record whose kind names a signed carrier but carries
    // the unsigned row's three operands passes the scratch-defs
    // grammar's empty tail — then fails the independently re-derived
    // effect declaration, which binds no `SaturatingSubtract(I32)` form
    // under the unsigned constraint key.
    let mut inputs = staged_saturating_subtract_inputs(target, 1, BlockZeroTerminator::Jump);
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
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 })
    );
    assert_eq!(
        validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
        Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 })
    );

    // The converse direction: a record whose kind names an unsigned
    // carrier but carries the clamped row's bound scratch `Def` fails the
    // exact three-operand grammar both sides derive for
    // `SaturatingSubtract(U64)` — the descriptor's scratch-defs grammar
    // belongs to the signed carriers alone.
    let mut inputs = staged_saturating_subtract_carrier_inputs(
        target,
        SaturatingCarrier::I32,
        1,
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
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    assert_eq!(
        validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
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

#[test]
fn saturating_subtract_zero_fold_rejects_a_clamped_consumer_whose_scratch_is_read_elsewhere() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    // The custody gate is whole-function: a block-1 `CopyI64` reading the
    // bound scratch register keeps a surviving use of a definition the
    // rewrite would stop making, so the fold refuses on both paths.
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let mut inputs = staged_saturating_subtract_carrier_inputs(
        target,
        SaturatingCarrier::I32,
        1,
        BlockZeroTerminator::Jump,
    );
    let mut plan = inputs.selected.transformed().clone();
    let function = &mut plan.functions[0];
    let scalar = function.virtual_registers[0].scalar_type;
    let class = function.virtual_registers[0].class;
    function
        .virtual_registers
        .push(selected_instructions::VirtualRegister {
            id: VirtualRegisterId(4),
            scalar_type: scalar,
            class,
            origin: selected_instructions::VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(4),
                source_value: semantic_vocabulary::ValueId::new(4).unwrap(),
            },
            definition_site: Some(optimization_unit::ValueDefinitionSite::Node {
                block: semantic_vocabulary::BlockId::new(2).unwrap(),
                node: 0,
            }),
            entry_fixed_view: None,
        });
    function.blocks[1].instructions.push(SelectedInstruction {
        id: SelectedInstructionId(4),
        kind: SelectedInstructionKind::CopyI64,
        constraint: copy.key,
        operands: vec![
            SelectedOperand {
                operand: copy.operands[0].operand,
                virtual_register: VirtualRegisterId(3),
                access: copy.operands[0].access,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            SelectedOperand {
                operand: copy.operands[1].operand,
                virtual_register: VirtualRegisterId(4),
                access: copy.operands[1].access,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: copy.implicit_uses.clone(),
        implicit_defs: copy.implicit_defs.clone(),
        clobbers: copy.clobbers.clone(),
        provenance: Default::default(),
    });
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
        "a live scratch reader"
    );
    assert_eq!(
        validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
        "a live scratch reader, replay"
    );
}

#[test]
fn saturating_subtract_zero_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The right-literal grammar exercises the operand-1 fold on both the
    // exact three-operand unsigned row and a signed carrier's
    // scratch-defs row; every recorded decision field must agree with the
    // action the validator independently reconstructs.
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        let inputs = staged_saturating_subtract_carrier_inputs(
            target,
            carrier,
            1,
            BlockZeroTerminator::Jump,
        );
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
        )
        .expect("the staged saturating-subtract-zero fold should validate");

        for mutation in 0..11 {
            let mut plan = result.plan().clone();
            match mutation {
                // The recorded result register is the subtract's own
                // `Def`, not the surviving `Use`.
                0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
                1 => plan.functions[0].action.as_mut().unwrap().result = None,
                // The recorded immediate is the folded literal zero; any
                // substitution replays differently.
                2 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
                3 => {
                    plan.functions[0]
                        .action
                        .as_mut()
                        .unwrap()
                        .consumer_instruction = SelectedInstructionId(9)
                }
                // The recorded constraint is the `CopyI64` row the
                // saturating-subtract-zero policy gate binds; any other
                // key fails the rebuild's binding.
                4 => {
                    plan.functions[0]
                        .action
                        .as_mut()
                        .unwrap()
                        .immediate_constraint
                        .variant += 1
                }
                5 => plan.functions[0].action = None,
                6 => {
                    plan.transformed_selected =
                        SelectedInstructionPlanIdentity::from_bytes([99; 32])
                }
                7 => plan.usage.candidates += 1,
                // A policy without the saturating-subtract-zero bit cannot
                // replay the fold: no `CopyI64` row binds for this
                // consumer and the action reconstructs nothing.
                8 => plan.policy = LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
                9 => {
                    plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32])
                }
                // The surviving register is the operand-0 `Use` the
                // rewritten copy binds: recording the result `Def`
                // register instead fails the re-derived action.
                _ => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(2),
            }
            assert!(
                validate(&inputs, &environment, plan).is_err(),
                "{carrier:?} mutation {mutation}"
            );
        }
    }
}

#[test]
fn saturating_subtract_zero_fold_reports_and_enforces_its_measured_work() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let inputs = staged_saturating_subtract_carrier_inputs(target, carrier, 1, block0);
            assert_budget_is_enforced(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            );
        }
    }
}

#[test]
fn saturating_subtract_zero_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for (target, block0) in positive_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let inputs = staged_saturating_subtract_carrier_inputs(target, carrier, 1, block0);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            );
        }
    }
}
