use super::{
    assert_budget_is_enforced, assert_deterministic_fixed_point, budget, fold_with, signed,
    staged_copy_inputs, staged_extension_inputs, staged_inputs, staged_load8_indexed_inputs,
    unsigned, validate,
};
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::{fold_selected_incoming_literal, validate_literal_fold};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::{RecoveryClassification, RecoveryVictimRole};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedOperand, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ScalarType, ValueId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn scalar_result_shape_on_a_flag_defining_consumer_is_rejected() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_inputs(target);

    // A `CompareI64` carrying a scalar `Def` operand does not match the
    // flag-defining shape its rule admits. Both the producer and the
    // independent replay must reject it rather than index past the
    // one-operand rewritten constraint row.
    let mut plan = inputs.selected.transformed().clone();
    let consumer = &mut plan.functions[0].blocks[0].instructions[1];
    let gpr = consumer.operands[0].class;
    consumer.operands.push(SelectedOperand {
        operand: 2,
        virtual_register: VirtualRegisterId(0),
        access: RegisterOperandAccess::Def,
        class: gpr,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    });
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
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
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
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn compare_fold_rejects_consumer_operands_carrying_unit_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The rewrite rebuilds the consumer's operands wholesale from the
    // rewritten constraint row. An operand carrying a unit binding — a
    // fixed view, an allocation tie, or an early clobber — would have that
    // binding silently dropped, so the pair's declared unit-effect surface
    // admits only undecorated consumer operands in the producer and the
    // independent replay.
    for mutation in 0..3 {
        let inputs = staged_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        let operand = &mut plan.functions[0].blocks[0].instructions[1].operands[0];
        match mutation {
            0 => operand.fixed_view = Some(register_model::RegisterViewId(0)),
            1 => operand.tied_to = Some(0),
            2 => operand.early_clobber = true,
            _ => unreachable!(),
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
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
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
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
        );
    }
}

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
fn extension_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1);
    }
}

#[test]
fn extension_fold_is_the_identity_when_no_candidate_is_classified() {
    // An input carrying no admitted classification is a fixed point: the fold
    // is the identity on it, which is what the staged fixed-point driver
    // requires of its terminal attempt. The classification is a legitimate
    // "not a candidate" verdict on the unchanged input custody.
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::SignExtendI8,
            0x80,
            signed(8),
        );
        inputs.recovery.plan.functions[0].classification = None;
        let terminal = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1)
            .expect("an unclassified input is a fixed point");
        assert_eq!(terminal.receipt().applied_count(), 0);
        assert_eq!(
            terminal.receipt().source_selected(),
            terminal.receipt().transformed_selected()
        );
        assert_eq!(terminal.transformed(), inputs.selected.transformed());
    }
}

#[test]
fn extension_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1);
    }
}

#[test]
fn extension_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_extension_inputs(
        target,
        SelectedInstructionKind::SignExtendI8,
        0x80,
        signed(8),
    );
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).unwrap();

    for mutation in 0..10 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = None,
            1 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            2 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
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
            8 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn extension_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
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
            // The binary right-operand position is the disjoint grammar of the
            // immediate forms; a unary consumer does not admit it.
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
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).map(|_| ()),
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
fn extension_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_extension_inputs(
        target,
        SelectedInstructionKind::ZeroExtendU8,
        0x1FF,
        unsigned(8),
    );
    // The extension consumer is unadmitted under every disjoint policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And a binary-positioned consumer is unadmitted under the extension
    // policy even though its producer shape matches.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::EXTENSION_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn extension_fold_rejects_result_types_that_cannot_admit_the_folded_constant() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    for mutation in 0..4 {
        let inputs = staged_extension_inputs(
            target,
            SelectedInstructionKind::ZeroExtendU8,
            0x1FF,
            unsigned(8),
        );
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // A Boolean result register is outside the integer grammar.
            0 => plan.functions[0].virtual_registers[2].scalar_type = ScalarType::Boolean,
            // An i8 result cannot admit the folded 0xFF constant.
            1 => plan.functions[0].virtual_registers[2].scalar_type = signed(8),
            // An extension consumer without a `Def` result does not match the
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
                LiteralFoldPolicy::EXTENSION_V1,
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
fn copy_materialization_folds_the_unary_copy_to_a_materialization_on_both_targets() {
    // The copy fold admits the full u64 literal domain — including values no
    // u12 immediate grammar could carry — and preserves the literal at the
    // copy's destination register.
    let cases = [
        (0x1FF_u64, unsigned(64), IntegerValue::Unsigned(0x1FF)),
        (
            0x1_0000_0001,
            unsigned(64),
            IntegerValue::Unsigned(0x1_0000_0001),
        ),
        (u64::MAX, signed(64), IntegerValue::Signed(-1)),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let materialize_key = environment.allocation_constraint_keys().materialize_i64;
        for (literal_constant, result_scalar, expected_value) in cases {
            let inputs = staged_copy_inputs(target, literal_constant, result_scalar);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1)
                .unwrap_or_else(|error| {
                    panic!("copy fold on {target:?} should validate: {error:?}")
                });

            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            assert_eq!(action.immediate, literal_constant);
            assert_eq!(action.surviving, VirtualRegisterId(1));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, materialize_key);

            let function = &result.transformed().functions[0];
            // The victim register is removed; the copy's result register
            // shifts into its slot and keeps its scalar type.
            assert_eq!(function.virtual_registers.len(), 2);
            assert_eq!(function.virtual_registers[1].id, VirtualRegisterId(1));
            assert_eq!(function.virtual_registers[1].scalar_type, result_scalar);
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
                SelectedInstructionKind::MaterializeI64 {
                    value: expected_value,
                }
            );
            assert_eq!(rewritten.constraint, materialize_key);
            assert_eq!(rewritten.operands.len(), 1);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            // The folded literal's provenance joins the consumer's.
            assert_eq!(rewritten.provenance.operations.len(), 2);
        }
    }
}

#[test]
fn copy_materialization_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_copy_inputs(target, 0x1_0000_0001, unsigned(64));
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).unwrap();

    for mutation in 0..10 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = None,
            1 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            2 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
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
            8 => plan.policy = LiteralFoldPolicy::EXTENSION_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
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
fn copy_materialization_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
    // The copy consumer is unadmitted under every disjoint policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
        LiteralFoldPolicy::LOAD8_INDEXED_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And the unary extension consumers stay unadmitted under the copy
    // policy even though their producer shape is identical: the grammars are
    // disjoint.
    for kind in [
        SelectedInstructionKind::ZeroExtendU8,
        SelectedInstructionKind::SignExtendI32,
    ] {
        let inputs = staged_extension_inputs(target, kind, 0x1FF, unsigned(8));
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{kind:?}"
        );
    }
    // A binary-positioned consumer is likewise unadmitted under the copy
    // policy.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::COPY_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
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
fn copy_materialization_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_copy_inputs(target, 0x1FF, unsigned(64));
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::COPY_V1);
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

#[test]
fn load8_indexed_fold_rewrites_the_index_operand_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let load8_key = keys.load8.unwrap();
        let load8_row = environment.constraint(load8_key).unwrap();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_load8_indexed_inputs(target, 5);

        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1)
            .unwrap_or_else(|error| {
                panic!("indexed byte-load fold on {target:?} should validate: {error:?}")
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
        // The surviving register is the base pointer the rewritten row's
        // `Use` position binds; the folded index register is removed.
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, load8_key);

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::Load8 { byte_offset: 5 }
        );
        assert_eq!(rewritten.constraint, load8_key);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.implicit_uses, load8_row.implicit_uses);
        assert_eq!(rewritten.implicit_defs, load8_row.implicit_defs);
        assert_eq!(rewritten.clobbers, load8_row.clobbers);
        assert_eq!(rewritten.provenance.operations.len(), 2);
    }
}
