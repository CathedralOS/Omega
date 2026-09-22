use super::{
    BlockZeroTerminator, assert_budget_is_enforced, assert_deterministic_fixed_point, fold_with,
    policy_without, policy_without_all, restage_literal, staged_saturating_divide_carrier_inputs,
    staged_saturating_divide_inputs, staged_saturating_subtract_inputs, staged_wrapping_add_inputs,
    validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy, validated_machine_effect_catalog};
use optimization_core::AcceptedObligationFactIdentity;
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SaturatingCarrier, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlanIdentity, SelectedOperand, SelectedTerminator,
    VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, ObligationId};
use std::sync::Arc;
use target::NativeTarget;

/// The targets and block-0 terminators the unsigned-carrier fold must
/// hold under. Every unsigned carrier binds the `divide_u64` row —
/// x86-64's pinned `div` and aarch64's `udiv` — which *defines* no unit
/// on either target, so even the flag-reading conditional branch keeps
/// the fold legal: x86-64's `rdx`/`rflags` surface is a clobber, and a
/// clobber's retirement cannot strand a reader.
fn unsigned_fixtures() -> [(NativeTarget, BlockZeroTerminator); 4] {
    [
        (
            NativeTarget::linux_x64(),
            BlockZeroTerminator::ConditionalBranch,
        ),
        (NativeTarget::linux_x64(), BlockZeroTerminator::Jump),
        (
            NativeTarget::linux_arm64(),
            BlockZeroTerminator::ConditionalBranch,
        ),
        (NativeTarget::linux_arm64(), BlockZeroTerminator::Jump),
    ]
}

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

/// Every unsigned carrier binds the `divide_u64` row — aarch64's bare
/// three-operand `udiv`; x86-64's pinned `div` continues past the `Def`
/// result with the operand-3 `Use` reading the zeroed high-half dividend.
fn unsigned_carriers() -> [SaturatingCarrier; 4] {
    [
        SaturatingCarrier::U8,
        SaturatingCarrier::U16,
        SaturatingCarrier::U32,
        SaturatingCarrier::U64,
    ]
}

/// Every signed carrier binds the `saturating_divide_signed` row —
/// x86-64's pinned `idiv` carries the same operand-3 zeroed-rdx `Use`
/// tail the unsigned row does; aarch64's clamped form writes a bound
/// scratch `Def` at operand 3 the fold drops under occurrence-free
/// custody.
fn signed_carriers() -> [SaturatingCarrier; 4] {
    [
        SaturatingCarrier::I8,
        SaturatingCarrier::I16,
        SaturatingCarrier::I32,
        SaturatingCarrier::I64,
    ]
}

/// The tail operand count and the number of tail operands staged as
/// auxiliary `Use`s — the operand-3 `Use` an x86-64 pinned divide row
/// reads — for the constraint row `key` names.
fn tail_shape(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    key: register_model::RegisterConstraintKey,
) -> (usize, usize) {
    let row = environment.constraint(key).unwrap();
    let tail = &row.operands[3..];
    (
        tail.len(),
        tail.iter()
            .filter(|operand| operand.access == RegisterOperandAccess::Use)
            .count(),
    )
}

#[test]
fn saturating_divide_one_fold_rewrites_every_unsigned_carrier_consumer() {
    for (target, block0) in unsigned_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        // The unsigned divide row carries the unit surface this family
        // exists to retire: x86-64's `div` clobbers `rdx` and `rflags`
        // and pins operand 3's `Use` on `rdx`, while aarch64's `udiv`
        // touches no unit at all. Neither declares an implicit use or
        // definition.
        let consumer_row = environment.constraint(keys.divide_u64).unwrap();
        assert!(consumer_row.implicit_uses.is_empty());
        assert!(consumer_row.implicit_defs.is_empty());
        let (tail, auxiliaries) = tail_shape(&environment, keys.divide_u64);
        if target == NativeTarget::linux_x64() {
            assert_eq!(consumer_row.operands.len(), 4);
            assert_eq!(consumer_row.operands[3].access, RegisterOperandAccess::Use);
            assert_eq!(auxiliaries, 1, "the x86-64 div reads a zeroed rdx");
            // The clobber surface is `rdx` — four lane units under the
            // lane-split physical model — plus `rflags`.
            assert_eq!(consumer_row.clobbers.len(), 5);
        } else {
            assert_eq!(consumer_row.operands.len(), 3);
            assert_eq!(tail, 0, "the aarch64 udiv carries no tail");
            assert!(consumer_row.clobbers.is_empty());
        }
        for carrier in unsigned_carriers() {
            let inputs = staged_saturating_divide_carrier_inputs(target, carrier, 1, block0);
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "saturating-divide-one fold on {carrier:?} with the literal at operand 1 on \
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
            // The recorded immediate is the folded literal itself — one —
            // the evidence the divide's encoded fault cannot fire; the
            // `CopyI64` rewrite binds the surviving register rather than
            // embedding a constant.
            assert_eq!(action.immediate, 1);
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(
                action.literal_instruction,
                SelectedInstructionId(auxiliaries as u32)
            );
            assert_eq!(
                action.consumer_instruction,
                SelectedInstructionId(auxiliaries as u32 + 1)
            );
            assert_eq!(action.immediate_constraint, keys.copy_i64);

            let function = &result.transformed().functions[0];
            // The fold removes only the divisor literal and its register:
            // the surviving operand, the result, and every dropped tail
            // register stay declared, redensified past the removed victim;
            // each auxiliary zero materialization stays, left dead.
            assert_eq!(function.virtual_registers.len(), 2 + tail);
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1 + auxiliaries);
            for (index, instruction) in instructions[..auxiliaries].iter().enumerate() {
                assert_eq!(instruction.id, SelectedInstructionId(index as u32));
                assert_eq!(
                    instruction.kind,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(0),
                    }
                );
            }
            let rewritten = &instructions[auxiliaries];
            assert_eq!(rewritten.id, SelectedInstructionId(auxiliaries as u32));
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys.copy_i64);
            // The rebuilt operand list binds only the surviving `Use` and
            // the result `Def` — redensified to `VirtualRegisterId(1)` —
            // from the clean copy row: the register pins and the dropped
            // tail are gone with the divide form, and so is every unit
            // effect it carried. `DischargedByLiteral`/`RetiredWhenDead` composition
            // retires the x86-64 `rdx`/`rflags` clobbers unconditionally —
            // dropping a clobber only narrows destruction.
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert!(!rewritten.operands[1].early_clobber);
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            assert!(rewritten.clobbers.is_empty());
            // The folded literal's provenance joins the consumer's, and
            // the divide's obligation custody is retained — the fold's
            // fault discharge rests on the literal, but the rewritten
            // record still carries the obligation its source declared.
            assert_eq!(rewritten.provenance.operations.len(), 2);
            assert_eq!(
                rewritten.provenance.obligations,
                vec![ObligationId::new(7).unwrap()]
            );

            match &function.blocks[0].terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. } => {
                    assert_eq!(
                        instruction.id,
                        SelectedInstructionId(auxiliaries as u32 + 1)
                    );
                }
                SelectedTerminator::Jump { instruction, .. } => {
                    assert_eq!(
                        instruction.id,
                        SelectedInstructionId(auxiliaries as u32 + 1)
                    );
                }
                _ => panic!("the staged block-0 terminator is retained"),
            }
            let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator
            else {
                panic!("return terminator retained");
            };
            assert_eq!(
                instruction.id,
                SelectedInstructionId(auxiliaries as u32 + 2)
            );
        }
    }
}

#[test]
fn saturating_divide_one_fold_rewrites_every_signed_carrier_consumer() {
    for (target, block0) in signed_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        // The signed divide row carries both surfaces this family
        // retires: the encoded `MayArchitecturalFaultV1` the divisor-one
        // literal discharges and the unit traffic the rewritten copy
        // drops — x86-64's `idiv` clobbers `rdx`/`rflags` behind the same
        // operand-3 zeroed `Use` the unsigned row reads, while aarch64's
        // clamped form defines `nzcv` and writes a bound scratch `Def` at
        // operand 3, both `Def`s early-clobber.
        let consumer_row = environment
            .constraint(keys.saturating_divide_signed)
            .unwrap();
        assert_eq!(consumer_row.operands.len(), 4);
        assert!(consumer_row.implicit_uses.is_empty());
        let (tail, auxiliaries) = tail_shape(&environment, keys.saturating_divide_signed);
        if target == NativeTarget::linux_x64() {
            assert_eq!(consumer_row.operands[3].access, RegisterOperandAccess::Use);
            assert_eq!(auxiliaries, 1, "the x86-64 idiv reads a zeroed rdx");
            assert!(consumer_row.implicit_defs.is_empty());
            // The clobber surface is `rdx` — four lane units under the
            // lane-split physical model — plus `rflags`.
            assert_eq!(consumer_row.clobbers.len(), 5);
        } else {
            assert_eq!(consumer_row.operands[3].access, RegisterOperandAccess::Def);
            assert_eq!(auxiliaries, 0, "the aarch64 signed row writes a scratch");
            assert_eq!(
                consumer_row.implicit_defs.len(),
                1,
                "aarch64 defines nzcv — the unit the fold must prove dead"
            );
            assert!(consumer_row.clobbers.is_empty());
            assert!(consumer_row.operands[2].early_clobber);
            assert!(consumer_row.operands[3].early_clobber);
        }
        for carrier in signed_carriers() {
            let inputs = staged_saturating_divide_carrier_inputs(target, carrier, 1, block0);
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "saturating-divide-one fold on {carrier:?} with the literal at operand 1 on \
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
            assert_eq!(action.immediate, 1);
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(
                action.literal_instruction,
                SelectedInstructionId(auxiliaries as u32)
            );
            assert_eq!(
                action.consumer_instruction,
                SelectedInstructionId(auxiliaries as u32 + 1)
            );
            assert_eq!(action.immediate_constraint, keys.copy_i64);

            let function = &result.transformed().functions[0];
            // The fold removes only the divisor literal and its register:
            // the surviving operand, the result, and the dropped tail
            // register stay declared, redensified past the removed
            // victim — the aarch64 scratch entry carries no operand
            // occurrence past the rewrite.
            assert_eq!(function.virtual_registers.len(), 2 + tail);
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1 + auxiliaries);
            for (index, instruction) in instructions[..auxiliaries].iter().enumerate() {
                assert_eq!(instruction.id, SelectedInstructionId(index as u32));
                assert_eq!(
                    instruction.kind,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(0),
                    }
                );
            }
            let rewritten = &instructions[auxiliaries];
            assert_eq!(rewritten.id, SelectedInstructionId(auxiliaries as u32));
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys.copy_i64);
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
            assert_eq!(
                rewritten.provenance.obligations,
                vec![ObligationId::new(7).unwrap()]
            );
        }
    }
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
fn saturating_divide_one_fold_rejects_the_left_literal_form() {
    // Division does not commute: `1 /| x` is `1 / x` clamped, not `x`,
    // so the family declares no left-literal grammar at all. Staging the
    // one literal at operand 0 — the consumer's own `Use` position for
    // its dividend — records a future use at a position no admitted
    // shape folds: both the producer's descriptor selection and the
    // replay's independently re-derived operand-1 victim position refuse
    // it as a future-use mismatch, on the unsigned row and the signed
    // row alike.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let inputs = staged_saturating_divide_carrier_inputs(
                target,
                carrier,
                0,
                BlockZeroTerminator::Jump,
            );
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
                "{carrier:?} `1 /| x` on {target:?} names no admitted grammar"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
                "{carrier:?} `1 /| x` on {target:?} names no admitted grammar, replay"
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
fn saturating_divide_one_fold_rejects_a_scratch_def_without_dead_custody() {
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
        let mut inputs = staged_saturating_divide_carrier_inputs(
            target,
            SaturatingCarrier::I32,
            1,
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
                LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
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
fn saturating_divide_one_fold_rejects_a_scratch_def_read_elsewhere() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    // The occurrence-free custody gate is whole-function: a block-1
    // `CopyI64` reading the bound scratch register keeps a surviving use
    // of a definition the rewrite would stop making, so the fold refuses
    // on both paths.
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let mut inputs = staged_saturating_divide_carrier_inputs(
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
            LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
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
fn saturating_divide_one_fold_rejects_while_a_terminator_reads_the_defined_unit() {
    // On aarch64 the conditional branch implicitly uses `nzcv`, the unit
    // every signed saturating-divide row defines: retiring the definition
    // would leave the branch observing stale flags, so the
    // dead-definitions gate — the producer's `admits_dead_consumer_defs`
    // and the replay's independent `dropped_unit_defs_dead` — refuses the
    // fold on both paths. The unsigned carriers' `udiv` defines no unit
    // and folds under the same terminator, and x86-64 keeps folding on
    // every carrier because `rflags` is only a clobber there: removing a
    // clobber narrows destruction and no reader can go stale.
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let branch = environment.constraint(keys.conditional_branch).unwrap();
    assert!(
        branch.implicit_uses.iter().any(|unit| environment
            .constraint(keys.saturating_divide_signed)
            .unwrap()
            .implicit_defs
            .contains(unit)),
        "the aarch64 branch reads the unit the signed divide row defines"
    );
    for carrier in signed_carriers() {
        let inputs = staged_saturating_divide_carrier_inputs(
            target,
            carrier,
            1,
            BlockZeroTerminator::ConditionalBranch,
        );
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
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
fn saturating_divide_one_fold_rejects_while_another_instruction_reads_the_defined_unit() {
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
            let mut inputs = staged_saturating_divide_carrier_inputs(
                target,
                carrier,
                1,
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
                        LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
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
                    LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
                )
                .expect("a reader of a unit the consumer does not define cannot block the fold");
                assert_eq!(result.receipt().applied_count(), 1);
            }
        }
    }
}

#[test]
fn saturating_divide_one_fold_rejects_a_consumer_implicitly_using_a_unit() {
    // `DischargedByLiteral`/`RetiredWhenDead` composition forbids the consumer's own
    // implicit uses too: a use the `CopyI64` does not carry would
    // silently stop being observed. Forging the branch row's
    // condition-state use onto the consumer — `nzcv` on aarch64,
    // `rflags` on x86-64 — fails the gate on both targets, however dead
    // the definition itself is.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let condition_state = environment
            .constraint(keys.conditional_branch)
            .unwrap()
            .implicit_uses
            .clone();
        assert!(!condition_state.is_empty());
        let mut inputs = staged_saturating_divide_inputs(target, 1, BlockZeroTerminator::Jump);
        let mut plan = inputs.selected.transformed().clone();
        plan.functions[0].blocks[0]
            .instructions
            .last_mut()
            .unwrap()
            .implicit_uses
            .clone_from(&condition_state);
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
fn saturating_divide_one_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The right-literal grammar folds operand 1 alone: a fixture with
        // the literal at operand 1 claiming operand 0 or the operand-2
        // `Def` position names no grammar's victim position, and a
        // fixture with the literal physically at operand 0 — the refused
        // `1 /| x` arrangement — claiming operand 1 finds the surviving
        // register where the victim must sit, a consumer mismatch under
        // the right-literal grammar.
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
            let mut inputs =
                staged_saturating_divide_inputs(target, literal_operand, BlockZeroTerminator::Jump);
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
                    LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
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
fn saturating_divide_one_fold_rejects_tied_consumer_operands_but_keeps_the_marks_it_allows() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS` admits `fixed_view` pins and
    // `early_clobber` marks — the bindings constrain only the dropped
    // operand list — while `tied_to` still rejects: a tied register would
    // be a co-allocation the rewrite silently dissolves. The x86-64
    // divide rows already pin operands 0, 2, and 3; the same gate
    // applies on the allocatable divisor `Use`.
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        for mutation in 0..3 {
            let mut inputs = staged_saturating_divide_carrier_inputs(
                target,
                carrier,
                1,
                BlockZeroTerminator::Jump,
            );
            let mut plan = inputs.selected.transformed().clone();
            let operand = &mut plan.functions[0].blocks[0].instructions[2].operands[1];
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
                        LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "{carrier:?} tied operand"
                );
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "{carrier:?} tied operand, replay"
                );
            } else {
                let result = fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
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
fn saturating_divide_one_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The saturating-divide operand grammar admits the literal only under
    // the saturating-divide-one policy: every selection naming no
    // saturating-divide family sees no admitted consumer kind — including
    // its sibling saturating families and the strongest posture, every
    // other rule enabled at once with both saturating-divide bits closed.
    let inputs = staged_saturating_divide_inputs(target, 1, BlockZeroTerminator::Jump);
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        LiteralFoldPolicy::SATURATING_ADD_ZERO_V1,
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
        policy_without_all(&[
            LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
            LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1,
        ]),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The sibling zero-dividend family admits the same consumer kind at
    // the operand-0 dividend position: with only the divisor-one bit
    // closed — or with the zero-dividend bit alone — the kind is
    // admitted but no enabled grammar covers the operand-1 divisor
    // position the staged literal occupies: a future-use mismatch, not
    // an unadmitted consumer.
    for policy in [
        LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1,
        policy_without(LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And the saturating-divide-one policy admits no other consumer: the
    // saturating-subtract and wrapping-add fixtures' consumer kinds are
    // no admitted kind.
    let saturating_subtract =
        staged_saturating_subtract_inputs(target, 1, BlockZeroTerminator::Jump);
    assert_eq!(
        fold_with(
            &saturating_subtract,
            &environment,
            LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    let wrapping = staged_wrapping_add_inputs(target, 1);
    assert_eq!(
        fold_with(
            &wrapping,
            &environment,
            LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn saturating_divide_one_fold_rejects_a_carrier_kind_against_another_row() {
    // Every carrier shares the family's one mixed-tail operand grammar,
    // so a record whose kind names a carrier on the other side of the
    // signedness boundary still parses — the operand grammar cannot
    // distinguish it — and the fold refuses only when the independently
    // bound effect declaration finds no `SaturatingDivide` form of that
    // carrier under the record's constraint key. Both directions reject
    // as an effect-surface mismatch on both targets.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (staged, claimed) in [
            (SaturatingCarrier::U64, SaturatingCarrier::I32),
            (SaturatingCarrier::I32, SaturatingCarrier::U64),
        ] {
            let mut inputs = staged_saturating_divide_carrier_inputs(
                target,
                staged,
                1,
                BlockZeroTerminator::Jump,
            );
            let mut plan = inputs.selected.transformed().clone();
            let consumer = plan.functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::SaturatingDivide { .. }
                    )
                })
                .unwrap();
            consumer.kind = SelectedInstructionKind::SaturatingDivide {
                carrier: claimed,
                obligation: ObligationId::new(7).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
            };
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
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "{staged:?} record claiming {claimed:?} on {target:?}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "{staged:?} record claiming {claimed:?} on {target:?}, replay"
            );
        }
    }
}

#[test]
fn saturating_divide_one_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The right-literal grammar exercises the operand-1 fold on the
    // auxiliary-`Use` tail both x86-64 rows carry; every recorded
    // decision field must agree with the action the validator
    // independently reconstructs.
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        let inputs =
            staged_saturating_divide_carrier_inputs(target, carrier, 1, BlockZeroTerminator::Jump);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
        )
        .expect("the staged saturating-divide-one fold should validate");

        for mutation in 0..11 {
            let mut plan = result.plan().clone();
            match mutation {
                // The recorded result register is the divide's own `Def`,
                // not the dividend the copy reads.
                0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
                1 => plan.functions[0].action.as_mut().unwrap().result = None,
                // The recorded immediate is the folded literal one; any
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
                // saturating-divide-one policy gate binds; any other key
                // fails the rebuild's binding.
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
                // A policy without the saturating-divide-one bit cannot
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
fn saturating_divide_one_fold_reports_and_enforces_its_measured_work() {
    for (target, block0) in signed_fixtures() {
        let environment = baseline_target_register_environment(target).unwrap();
        for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
            let inputs = staged_saturating_divide_carrier_inputs(target, carrier, 1, block0);
            assert_budget_is_enforced(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
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
