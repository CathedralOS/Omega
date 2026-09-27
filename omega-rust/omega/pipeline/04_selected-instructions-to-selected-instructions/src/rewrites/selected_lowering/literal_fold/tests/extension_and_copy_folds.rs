use super::{
    assert_deterministic_fixed_point, budget, fold_with, signed, staged_copy_inputs,
    staged_extension_inputs, unsigned,
};
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::{fold_selected_incoming_literal, validate_literal_fold};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::{RecoveryClassification, RecoveryVictimRole};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ScalarType, ValueId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn extension_elimination_folds_every_unary_consumer_to_a_materialization_on_both_targets() {
    let cases = [
        (
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF_u64,
            unsigned(8),
            0xFF_u64,
            IntegerValue::Unsigned(0xFF),
        ),
        (
            SelectedInstructionKind::ZeroExtendU16,
            0x1_FFFF,
            unsigned(16),
            0xFFFF,
            IntegerValue::Unsigned(0xFFFF),
        ),
        (
            SelectedInstructionKind::ZeroExtendU32,
            0x1_FFFF_FFFF,
            unsigned(32),
            0xFFFF_FFFF,
            IntegerValue::Unsigned(0xFFFF_FFFF),
        ),
        (
            SelectedInstructionKind::SignExtendI8,
            0x80,
            signed(8),
            u64::MAX - 0x7F,
            IntegerValue::Signed(-128),
        ),
        (
            SelectedInstructionKind::SignExtendI16,
            0x8000,
            signed(16),
            u64::MAX - 0x7FFF,
            IntegerValue::Signed(-32768),
        ),
        (
            SelectedInstructionKind::SignExtendI32,
            0x8000_0000,
            signed(32),
            u64::MAX - 0x7FFF_FFFF,
            IntegerValue::Signed(-2_147_483_648),
        ),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let materialize_key = environment.allocation_constraint_keys().materialize_i64;
        for (kind, literal_constant, result_scalar, expected_bits, expected_value) in cases {
            let inputs = staged_extension_inputs(target, kind, literal_constant, result_scalar);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1)
                .unwrap_or_else(|error| {
                    panic!("{kind:?} fold on {target:?} should validate: {error:?}")
                });

            assert_eq!(result.receipt().applied_count(), 1, "{kind:?}");
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)), "{kind:?}");
            assert_eq!(action.immediate, expected_bits, "{kind:?}");
            assert_eq!(action.surviving, VirtualRegisterId(1), "{kind:?}");
            assert_eq!(action.victim, VirtualRegisterId(1), "{kind:?}");
            assert_eq!(
                action.literal_instruction,
                SelectedInstructionId(0),
                "{kind:?}"
            );
            assert_eq!(
                action.consumer_instruction,
                SelectedInstructionId(1),
                "{kind:?}"
            );
            assert_eq!(action.immediate_constraint, materialize_key, "{kind:?}");

            let function = &result.transformed().functions[0];
            // The victim register is removed; the extension's result register
            // shifts into its slot and keeps its scalar type.
            assert_eq!(function.virtual_registers.len(), 2, "{kind:?}");
            assert_eq!(
                function.virtual_registers[1].id,
                VirtualRegisterId(1),
                "{kind:?}"
            );
            assert_eq!(
                function.virtual_registers[1].scalar_type, result_scalar,
                "{kind:?}"
            );
            assert_eq!(
                function.virtual_registers[1].origin,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(0),
                    source_value: ValueId::new(3).unwrap(),
                },
                "{kind:?}"
            );
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1, "{kind:?}");
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0), "{kind:?}");
            assert_eq!(
                rewritten.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: expected_value,
                },
                "{kind:?}"
            );
            assert_eq!(rewritten.constraint, materialize_key, "{kind:?}");
            assert_eq!(rewritten.operands.len(), 1, "{kind:?}");
            assert_eq!(
                rewritten.operands[0].virtual_register,
                VirtualRegisterId(1),
                "{kind:?}"
            );
            assert_eq!(
                rewritten.operands[0].access,
                RegisterOperandAccess::Def,
                "{kind:?}"
            );
            assert!(rewritten.implicit_uses.is_empty(), "{kind:?}");
            assert!(rewritten.implicit_defs.is_empty(), "{kind:?}");
            // The folded literal's provenance joins the consumer's.
            assert_eq!(rewritten.provenance.operations.len(), 2, "{kind:?}");
        }
    }
}

#[test]
fn copy_materialization_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
        let slot = inputs.recovery.plan.functions[0]
            .classification
            .as_mut()
            .unwrap();
        match mutation {
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                }
            }
            // The binary right-operand position is the disjoint grammar of
            // the immediate forms; a unary copy consumer does not admit it.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 1;
            }
            2 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].instruction = SelectedInstructionId(0);
            }
            3 => slot.victim = VirtualRegisterId(0),
            4 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].block = SelectedBlockId(1);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).map(|_| ()),
            Err(match mutation {
                0 => LiteralFoldError::UnsupportedVictimRole { function: 0 },
                1 | 4 => LiteralFoldError::FutureUseMismatch { function: 0 },
                2 => LiteralFoldError::ConsumerMismatch { function: 0 },
                _ => LiteralFoldError::LiteralMismatch { function: 0 },
            }),
            "mutation {mutation}"
        );
    }
}

#[test]
fn copy_materialization_fold_rejects_result_types_that_cannot_admit_the_literal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    for mutation in 0..4 {
        let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // A Boolean result register is outside the integer grammar.
            0 => plan.functions[0].virtual_registers[2].scalar_type = ScalarType::Boolean,
            // Unlike the extension fold, the copy cannot narrow: a u8 result
            // cannot admit the unfolded 0x1FF literal.
            1 => plan.functions[0].virtual_registers[2].scalar_type = unsigned(8),
            // A copy consumer without a `Def` result does not match the
            // unary grammar.
            2 => {
                let consumer = &mut plan.functions[0].blocks[0].instructions[1];
                consumer.operands.pop();
            }
            // An extra `Use` operand does not match the unary grammar either.
            _ => {
                let consumer = &mut plan.functions[0].blocks[0].instructions[1];
                consumer.operands.push(consumer.operands[0]);
            }
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);

        assert!(
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
                LiteralFoldPolicy::COPY_V1,
                budget(),
            )
            .is_err(),
            "mutation {mutation}"
        );
        assert!(
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
            )
            .is_err(),
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn copy_materialization_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::COPY_V1);
    }
}
