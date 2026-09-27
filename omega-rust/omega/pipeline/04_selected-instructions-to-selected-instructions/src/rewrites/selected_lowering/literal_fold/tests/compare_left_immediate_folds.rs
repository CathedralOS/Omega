//! The operand-swapped compare grammars: `MaterializeI64` feeding the
//! operand-0 minuend `Use` of `CompareI64` folds to `CompareI64Immediate`
//! computing `x - literal` for `literal - x`, or to `CompareI64Zero`
//! computing `x - 0` when the folded literal is exactly zero — the
//! family's value partition binds each realization. Either rewrite
//! preserves the zero condition exactly and inverts every ordering
//! predicate, so the pair admits the fold only while every reader each
//! preserved condition-state definition can reach through the
//! function's CFG is equality-sensing — `MaterializeBooleanEqual` or
//! `ConditionalBranchNonZero`. These tests stage that grammar and its
//! reader-flow audit: same-block readers, readers reached through
//! successor edges, joins, loops, redefinitions, malformed unit surfaces,
//! and the producer/replay disagreement matrix.
use super::{
    Inputs, fold, fold_with, restage_literal, staged_left_inputs, successor, unsigned, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_homes::{RecoveryClassification, RecoveryVictimRole};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedOperand, SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::ValueId;
use std::sync::Arc;
use target::NativeTarget;

/// Apply `edit` to the staged function's selected plan in place — the
/// fold reads the plan only through `inputs.selected.transformed`.
fn edit_function(inputs: &mut Inputs, edit: impl FnOnce(&mut SelectedFunction)) {
    let mut plan = inputs.selected.transformed().clone();
    edit(&mut plan.functions[0]);
    inputs.selected.transformed = Arc::new(plan);
}

/// The staged compare record — the consumer the operand-swapped grammar
/// folds.
fn edit_compare(inputs: &mut Inputs, edit: impl FnOnce(&mut SelectedInstruction)) {
    edit_function(inputs, |function| {
        let compare = function.blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
            .expect("the staged fixture carries the compare");
        edit(compare);
    });
}

/// A flag-reading materialization of `kind` defining the staged boolean
/// register — the block-body reader the audit meets.
fn boolean_reader(
    environment: &ValidatedTargetRegisterEnvironment,
    id: u32,
    kind: SelectedInstructionKind,
) -> SelectedInstruction {
    let row = environment
        .constraint(environment.selected_keys().materialize_boolean)
        .unwrap();
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: row.key,
        operands: vec![SelectedOperand {
            operand: row.operands[0].operand,
            virtual_register: VirtualRegisterId(2),
            access: row.operands[0].access,
            class: row.operands[0].class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: SelectedInstructionProvenance::default(),
    }
}

/// The boolean register the staged readers define, in the
/// materialization row's own operand class.
fn boolean_register(
    environment: &ValidatedTargetRegisterEnvironment,
    instruction: u32,
) -> VirtualRegister {
    let row = environment
        .constraint(environment.selected_keys().materialize_boolean)
        .unwrap();
    VirtualRegister {
        id: VirtualRegisterId(2),
        scalar_type: unsigned(8),
        class: row.operands[0].class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(instruction),
            source_value: ValueId::new(9).unwrap(),
        },
        definition_site: None,
        entry_fixed_view: None,
    }
}

/// A terminator-carried instruction of `kind` on the staged conditional
/// branch row — the flag reader a terminator position carries.
fn branch_reader(
    environment: &ValidatedTargetRegisterEnvironment,
    id: u32,
    kind: SelectedInstructionKind,
) -> SelectedInstruction {
    let row = environment
        .constraint(environment.selected_keys().conditional_branch)
        .unwrap();
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: row.key,
        operands: Vec::new(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: SelectedInstructionProvenance::default(),
    }
}

/// A second `CompareI64` re-publishing the staged compare's whole implicit
/// surface — the redefinition that ends a carried flag live range.
fn second_compare(
    environment: &ValidatedTargetRegisterEnvironment,
    id: u32,
) -> SelectedInstruction {
    let row = environment
        .constraint(environment.selected_keys().compare_i64)
        .unwrap();
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind: SelectedInstructionKind::CompareI64,
        constraint: row.key,
        operands: vec![
            SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                access: RegisterOperandAccess::Use,
                class: row.operands[0].class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            SelectedOperand {
                operand: 1,
                virtual_register: VirtualRegisterId(0),
                access: RegisterOperandAccess::Use,
                class: row.operands[1].class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: SelectedInstructionProvenance::default(),
    }
}

/// A `Jump` terminator into `target` carrying the staged jump row's
/// instruction.
fn jump(
    environment: &ValidatedTargetRegisterEnvironment,
    id: u32,
    target: u32,
    edge: u64,
) -> SelectedTerminator {
    let row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            id: SelectedInstructionId(id),
            kind: SelectedInstructionKind::Jump,
            constraint: row.key,
            operands: Vec::new(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: SelectedInstructionProvenance::default(),
        },
        successor: successor(target, edge),
    }
}

/// Push the boolean reader and its register onto the staged function,
/// appended at the tail of block `block_index`'s instruction list.
fn append_reader(
    inputs: &mut Inputs,
    environment: &ValidatedTargetRegisterEnvironment,
    block_index: usize,
    id: u32,
    kind: SelectedInstructionKind,
) {
    edit_function(inputs, |function| {
        function.blocks[block_index]
            .instructions
            .push(boolean_reader(environment, id, kind));
        function
            .virtual_registers
            .push(boolean_register(environment, id));
    });
}

#[test]
fn left_zero_compare_fold_carries_the_reader_flow_audit() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // `0 - x` rewriting to `x - 0` inverts the ordering predicates
        // exactly as the left-immediate grammar does: an
        // ordering-sensitive reader of the kept condition-state
        // definitions refuses the fold under either admission path —
        // the producer's pair surface and the replay's independently
        // re-derived flow audit.
        let mut inputs = staged_left_inputs(target);
        restage_literal(&mut inputs, 0);
        append_reader(
            &mut inputs,
            &environment,
            0,
            4,
            SelectedInstructionKind::MaterializeBooleanI64LessThan,
        );
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{target:?} producer"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{target:?} replay"
        );

        // The same staging with an equality-sensing reader folds.
        let mut inputs = staged_left_inputs(target);
        restage_literal(&mut inputs, 0);
        append_reader(
            &mut inputs,
            &environment,
            0,
            4,
            SelectedInstructionKind::MaterializeBooleanEqual,
        );
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1)
            .expect("the left zero literal folds under an equality reader");
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::CompareI64Zero
        );
    }
}

#[test]
fn left_compare_fold_rejects_every_ordering_sensing_reader_kind() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for kind in [
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            SelectedInstructionKind::MaterializeBooleanI64LessThan,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        ] {
            let mut inputs = staged_left_inputs(target);
            append_reader(&mut inputs, &environment, 0, 4, kind);
            assert_eq!(
                fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "{kind:?} on {target:?} producer"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "{kind:?} on {target:?} replay"
            );
        }
        // The predicate-aware terminators are ordering readers too: the
        // staged `ConditionalBranch` swaps for each strict-less-than form.
        for kind in [
            SelectedInstructionKind::ConditionalBranchU64LessThan,
            SelectedInstructionKind::ConditionalBranchI64LessThan,
        ] {
            let mut inputs = staged_left_inputs(target);
            let instruction = branch_reader(&environment, 2, kind);
            edit_function(&mut inputs, |function| {
                function.blocks[0].terminator = match kind {
                    SelectedInstructionKind::ConditionalBranchU64LessThan => {
                        SelectedTerminator::ConditionalBranchU64LessThan {
                            instruction,
                            when_less: successor(1, 1),
                            when_not_less: successor(1, 2),
                        }
                    }
                    _ => SelectedTerminator::ConditionalBranchI64LessThan {
                        instruction,
                        when_less: successor(1, 1),
                        when_not_less: successor(1, 2),
                    },
                };
            });
            assert_eq!(
                fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "{kind:?} on {target:?} producer"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "{kind:?} on {target:?} replay"
            );
        }
    }
}

#[test]
fn left_compare_fold_ends_the_carried_flow_at_a_redefinition() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Block 0 branches to block 1 — which immediately redefines the
        // condition state with a second compare and then reads it through
        // an *ordering* materialization — and to the return block... the
        // base fixture's block 1. The reader observes the later
        // definition, not the folded one, so the fold stays admitted.
        let mut inputs = staged_left_inputs(target);
        edit_function(&mut inputs, |function| {
            function.blocks[1]
                .instructions
                .push(second_compare(&environment, 4));
            function.blocks[1].instructions.push(boolean_reader(
                &environment,
                5,
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
            ));
            function
                .virtual_registers
                .push(boolean_register(&environment, 5));
        });
        fold(&inputs, &environment);
    }
}

#[test]
fn left_compare_fold_refuses_an_unresolved_successor_target() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // A successor edge naming a block the function does not contain
        // leaves the unit's readers unprovable — the audit refuses rather
        // than assume.
        let mut inputs = staged_left_inputs(target);
        edit_function(&mut inputs, |function| {
            function.blocks[0].terminator = jump(&environment, 2, 9, 1);
        });
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{target:?} producer"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn left_compare_fold_rejects_decorated_and_misshapen_operands() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..6 {
        let mut inputs = staged_left_inputs(target);
        match mutation {
            // A `fixed_view` pin on the victim would silently drop with
            // the rebuilt operand list.
            0 => edit_compare(&mut inputs, |compare| {
                compare.operands[0].fixed_view = Some(register_model::RegisterViewId(0));
            }),
            // A `tied_to` tie has no carried meaning once the operands are
            // rebuilt from the row.
            1 => edit_compare(&mut inputs, |compare| {
                compare.operands[0].tied_to = Some(1);
            }),
            // An `early_clobber` mark likewise.
            2 => edit_compare(&mut inputs, |compare| {
                compare.operands[1].early_clobber = true;
            }),
            // The surviving operand must stay a `Use`.
            3 => edit_compare(&mut inputs, |compare| {
                compare.operands[1].access = RegisterOperandAccess::UseDef;
            }),
            // The victim operand must stay a `Use`.
            4 => edit_compare(&mut inputs, |compare| {
                compare.operands[0].access = RegisterOperandAccess::UseDef;
            }),
            // A third operand is no admitted grammar.
            _ => edit_compare(&mut inputs, |compare| {
                let extra = compare.operands[1];
                compare.operands.push(extra);
            }),
        }
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation} producer"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn left_compare_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_left_inputs(target);
        let slot = inputs.recovery.plan.functions[0]
            .classification
            .as_mut()
            .unwrap();
        let expected = match mutation {
            // Only an `Incoming` victim may fold.
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                };
                LiteralFoldError::UnsupportedVictimRole { function: 0 }
            }
            // Claiming the right `Use` names the in-place grammar, whose
            // operand-1 victim requirement the staged `[victim,
            // subtrahend]` record does not meet — operand 1 binds the
            // entry parameter, not the literal register.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 1;
                LiteralFoldError::ConsumerMismatch { function: 0 }
            }
            // An operand position no compare grammar covers.
            2 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 7;
                LiteralFoldError::FutureUseMismatch { function: 0 }
            }
            // The classified victim must be the register the literal
            // record defines and the claimed `Use` binds.
            3 => {
                slot.victim = VirtualRegisterId(0);
                LiteralFoldError::LiteralMismatch { function: 0 }
            }
            // A second recorded use means the literal is not single-use.
            _ => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                let extra = future_uses[0];
                future_uses.push(extra);
                LiteralFoldError::FutureUseMismatch { function: 0 }
            }
        };
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
            Err(expected.clone()),
            "mutation {mutation} producer"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(expected),
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn left_compare_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_left_inputs(target);
    let result = fold(&inputs, &environment);

    for mutation in 0..9 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            1 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            2 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .immediate_constraint
                    .variant += 1
            }
            4 => plan.functions[0].action = None,
            5 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            6 => plan.usage.candidates += 1,
            7 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            8 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}
