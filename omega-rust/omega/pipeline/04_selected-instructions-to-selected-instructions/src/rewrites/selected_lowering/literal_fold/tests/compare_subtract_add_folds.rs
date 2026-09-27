use super::{
    budget, fold_with, restage_literal, staged_add_inputs, staged_inputs, staged_subtract_inputs,
    validate,
};
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::{fold_selected_incoming_literal, validate_literal_fold};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use optimization_core::AcceptedObligationFactIdentity;
use register_environment::baseline_target_register_environment;
use register_homes::{RecoveryClassification, RecoveryVictimRole};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ObligationId, ValueId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn compare_fold_partitions_the_literal_value_between_forms() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        // The family's admission partition is total over the window: zero
        // names the dedicated form, every nonzero literal through the
        // shared twelve-bit encoding limit names the immediate form, and
        // beyond the window nothing admits. Exactly one pair admits each
        // candidate, so selection can never be ambiguous.
        for (literal, expected) in [
            (1_u64, keys.compare_i64_immediate),
            (4095_u64, keys.compare_i64_immediate),
        ] {
            let mut inputs = staged_inputs(target);
            restage_literal(&mut inputs, literal);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1)
                .expect("a nonzero in-window literal folds to the immediate form");
            assert_eq!(
                result.plan().functions[0]
                    .action
                    .unwrap()
                    .immediate_constraint,
                expected,
                "literal {literal} on {target:?}"
            );
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[0].kind,
                SelectedInstructionKind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(u128::from(literal)),
                }
            );
        }
        let mut inputs = staged_inputs(target);
        restage_literal(&mut inputs, 0);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1)
            .expect("the zero literal folds");
        assert_eq!(
            result.plan().functions[0]
                .action
                .unwrap()
                .immediate_constraint,
            keys.compare_i64_zero
        );
    }
}

#[test]
fn subtract_immediate_fold_replaces_the_flag_clobbering_consumer_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let subtract_row = environment.constraint(keys.subtract_i64).unwrap();
        let immediate_row = environment.constraint(keys.subtract_i64_immediate).unwrap();
        let inputs = staged_subtract_inputs(target);

        // x86-64's register subtract clobbers the condition state the
        // immediate form preserves: the pair's declared surface admits the
        // consumer's clobber because the rewrite replaces it wholesale.
        if target == NativeTarget::linux_x64() {
            assert!(!subtract_row.clobbers.is_empty());
        }
        assert!(immediate_row.clobbers.is_empty());
        assert!(immediate_row.implicit_defs.is_empty());

        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1)
            .unwrap_or_else(|error| {
                panic!("subtract fold on {target:?} should validate: {error:?}")
            });

        assert_eq!(
            result.plan().machine_effect_catalog,
            effect_catalog.identity()
        );
        assert_eq!(
            result.receipt().machine_effect_catalog(),
            effect_catalog.identity()
        );
        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        assert_eq!(action.immediate, 5);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, keys.subtract_i64_immediate);

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        assert_eq!(
            function.virtual_registers[1].origin,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(0),
                source_value: ValueId::new(3).unwrap(),
            }
        );
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(5),
                obligation: ObligationId::new(7).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
            }
        );
        assert_eq!(rewritten.constraint, keys.subtract_i64_immediate);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        // The rewritten instruction carries exactly the immediate row's unit
        // surface: the register subtract's flag clobber does not survive.
        assert_eq!(rewritten.implicit_uses, immediate_row.implicit_uses);
        assert_eq!(rewritten.implicit_defs, immediate_row.implicit_defs);
        assert_eq!(rewritten.clobbers, immediate_row.clobbers);
        assert_eq!(rewritten.provenance.operations.len(), 2);
    }
}

#[test]
fn subtract_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..6 {
        let mut inputs = staged_subtract_inputs(target);
        let slot = inputs.recovery.plan.functions[0]
            .classification
            .as_mut()
            .unwrap();
        let (producer_expected, replay_expected) = match mutation {
            // Only an `Incoming` victim may fold; a reclaimed resident is not
            // the operand the consumer reads.
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                };
                (
                    LiteralFoldError::UnsupportedVictimRole { function: 0 },
                    LiteralFoldError::UnsupportedVictimRole { function: 0 },
                )
            }
            // The subtract grammar folds the literal at operand 1 only:
            // claiming the left `Use` is a position no admitted grammar
            // covers.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 0;
                (
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                )
            }
            // An over-bound immediate fails admission in the producer; the
            // replay first refuses the literal record itself, since it no
            // longer materializes the value the classification carries.
            2 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    value, ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                *value = IntegerValue::Unsigned(4096);
                (
                    LiteralFoldError::UnsupportedImmediate { function: 0 },
                    LiteralFoldError::LiteralMismatch { function: 0 },
                )
            }
            // The classified victim must be the register the literal record
            // defines and the claimed `Use` operand binds.
            3 => {
                slot.victim = VirtualRegisterId(0);
                (
                    LiteralFoldError::LiteralMismatch { function: 0 },
                    LiteralFoldError::LiteralMismatch { function: 0 },
                )
            }
            // The use must sit in the classified block itself.
            4 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].block = SelectedBlockId(1);
                (
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                )
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
                (
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                    LiteralFoldError::FutureUseMismatch { function: 0 },
                )
            }
        };
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).map(|_| ()),
            Err(producer_expected),
            "mutation {mutation}"
        );
        assert_eq!(
            validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
            Err(replay_expected),
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn add_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; claiming the
        // other `Use` position in the recovery classification selects the
        // disjoint grammar, whose operand check finds the surviving register
        // where the victim must sit.
        for (literal_operand, claimed_operand) in [(0u16, 1u16), (1u16, 0u16)] {
            let mut inputs = staged_add_inputs(target, literal_operand);
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
                fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} claiming {claimed_operand} on {target:?}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} claiming {claimed_operand} on {target:?} \
                 replay"
            );
        }
    }
}

#[test]
fn add_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest immediate any target's
        // add-immediate encoder admits — the shared 12-bit encoding limit:
        // 4095 folds and 4096 cannot, under either operand grammar.
        for literal_operand in [0u16, 1u16] {
            let mut inputs = staged_add_inputs(target, literal_operand);
            restage_literal(&mut inputs, 4095);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1)
                .expect("the boundary immediate folds");
            assert_eq!(result.receipt().applied_count(), 1);
            assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[0].kind,
                SelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(4095),
                    obligation: ObligationId::new(7).unwrap(),
                    accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
                }
            );

            let mut inputs = staged_add_inputs(target, literal_operand);
            restage_literal(&mut inputs, 4096);
            assert_eq!(
                fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "literal at operand {literal_operand} on {target:?}"
            );
        }
    }
}

#[test]
fn fold_rejects_instructions_the_bound_effect_catalog_does_not_declare() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        for mutation in 0..2 {
            let inputs = staged_inputs(target);
            let mut plan = inputs.selected.transformed().clone();
            match mutation {
                // Binding the literal to the compare row leaves no
                // `(MaterializeI64, compare)` declaration to admit.
                0 => plan.functions[0].blocks[0].instructions[0].constraint = keys.compare_i64,
                // Binding the consumer to the materialize row leaves no
                // `(CompareI64, materialize)` declaration either.
                _ => plan.functions[0].blocks[0].instructions[1].constraint = keys.materialize_i64,
            }
            let mut selected = inputs.selected.clone();
            selected.transformed = Arc::new(plan);

            assert_eq!(
                fold_selected_incoming_literal(
                    &selected,
                    &inputs.ranges,
                    &inputs.legality,
                    &inputs.spill_choices,
                    &inputs.recovery,
                    &inputs.availability,
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    &keys,
                    &effect_catalog,
                    LiteralFoldPolicy::COMPARE_V1,
                    budget(),
                ),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "mutation {mutation} on {target:?}"
            );
            assert_eq!(
                validate_literal_fold(
                    &selected,
                    &inputs.ranges,
                    &inputs.legality,
                    &inputs.spill_choices,
                    &inputs.recovery,
                    &inputs.availability,
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    &keys,
                    &effect_catalog,
                    inputs.selected.plan().clone(),
                ),
                Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
                "mutation {mutation} replay on {target:?}"
            );
        }
    }
}
