use super::{
    assert_budget_is_enforced, assert_deterministic_fixed_point, budget, fold_with,
    staged_byte_view_address_backing_inputs, staged_byte_view_address_inputs, staged_divide_inputs,
    staged_inputs, staged_load8_indexed_inputs, validate,
};
use crate::{
    LiteralFoldError, LiteralFoldPolicy, fold_selected_incoming_literal, validate_literal_fold,
    validated_machine_effect_catalog,
};
use crate::{RecoveryClassification, RecoveryVictimRole};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedTerminator, VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, ObligationId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn load8_indexed_fold_rejects_an_unencodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest byte-offset field any target's
        // `Load8` encoder admits — aarch64 `ldrb`'s 12-bit unsigned offset —
        // so 4096 cannot fold into a byte offset anywhere.
        let inputs = staged_load8_indexed_inputs(target, 4096);
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn load8_indexed_fold_admits_the_widest_encodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // 4095 is the widest offset the shared 12-bit byte-offset bound
        // admits — the boundary the unencodable 4096 sits just past.
        let inputs = staged_load8_indexed_inputs(target, 4095);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1)
            .expect("the boundary offset folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::Load8 { byte_offset: 4095 }
        );
    }
}

#[test]
fn load8_indexed_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_load8_indexed_inputs(target, 5);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1);
    }
}

#[test]
fn load8_indexed_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_load8_indexed_inputs(target, 5);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::LOAD8_INDEXED_V1,
        );
    }
}

#[test]
fn load8_indexed_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_load8_indexed_inputs(target, 5);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).unwrap();

    for mutation in 0..11 {
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
            5 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            6 => plan.functions[0].action = None,
            7 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            8 => plan.usage.candidates += 1,
            // A policy without the indexed-load bit cannot replay the fold:
            // no `Load8` row binds and the action reconstructs nothing.
            9 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            10 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn load8_indexed_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_load8_indexed_inputs(target, 5);
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
            // The folded operand is the index position — operand 1; the
            // base-pointer position 0 is not a foldable literal site.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 0;
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
            fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).map(|_| ()),
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
fn load8_indexed_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_load8_indexed_inputs(target, 5);
    // The indexed byte-load consumer is unadmitted under every other policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And a compare-staged consumer is unadmitted under the indexed-load
    // policy even though its producer shape matches.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn load8_indexed_fold_rejects_consumer_operands_carrying_unit_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The rewrite rebuilds the indexed load's operands wholesale from the
    // `Load8` constraint row; an operand carrying a unit binding would have
    // it silently dropped, so the producer and the independent replay both
    // reject decorated consumers.
    for mutation in 0..3 {
        let inputs = staged_load8_indexed_inputs(target, 5);
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
                LiteralFoldPolicy::LOAD8_INDEXED_V1,
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
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn byte_view_address_fold_rewrites_the_offset_operand_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let address_offset_key = keys.address_offset.unwrap();
        let address_offset_row = environment.constraint(address_offset_key).unwrap();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_byte_view_address_inputs(target, 5);

        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .unwrap_or_else(|error| {
            panic!("byte-view address fold on {target:?} should validate: {error:?}")
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
        // The surviving register is the base the rewritten row's `Use`
        // position binds; the folded offset register is removed.
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, address_offset_key);

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 5 }
        );
        assert_eq!(rewritten.constraint, address_offset_key);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.implicit_uses, address_offset_row.implicit_uses);
        assert_eq!(rewritten.implicit_defs, address_offset_row.implicit_defs);
        assert_eq!(rewritten.clobbers, address_offset_row.clobbers);
        assert_eq!(rewritten.provenance.operations.len(), 2);
    }
}

#[test]
fn byte_view_address_fold_rejects_an_unencodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest byte-offset field any target's
        // `AddressOffset` encoder admits — aarch64 `add`'s 12-bit unsigned
        // immediate — so 4096 cannot fold into a byte offset anywhere.
        let inputs = staged_byte_view_address_inputs(target, 4096);
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
            )
            .map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn byte_view_address_fold_admits_the_widest_encodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // 4095 is the widest offset the shared 12-bit byte-offset bound
        // admits — the boundary the unencodable 4096 sits just past.
        let inputs = staged_byte_view_address_inputs(target, 4095);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .expect("the boundary offset folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 4095 }
        );
    }
}

#[test]
fn byte_view_address_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_byte_view_address_inputs(target, 5);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        );
    }
}

#[test]
fn byte_view_address_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_byte_view_address_inputs(target, 5);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        );
    }
}

#[test]
fn byte_view_address_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_byte_view_address_inputs(target, 5);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
    )
    .unwrap();

    for mutation in 0..11 {
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
            5 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            6 => plan.functions[0].action = None,
            7 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            8 => plan.usage.candidates += 1,
            // A policy without the byte-view-address bit cannot replay the
            // fold: no `AddressOffset` row binds and the action reconstructs
            // nothing.
            9 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            10 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn byte_view_address_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..5 {
        let mut inputs = staged_byte_view_address_inputs(target, 5);
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
            // The backing position 0 is itself an admitted fold site, but
            // this staged consumer binds the surviving base register there
            // while the literal register sits at operand 1 — the commuted
            // grammar rejects the mismatched operand wiring.
            1 => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 0;
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
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
            )
            .map(|_| ()),
            Err(match mutation {
                0 => LiteralFoldError::UnsupportedVictimRole { function: 0 },
                1 | 2 => LiteralFoldError::ConsumerMismatch { function: 0 },
                4 => LiteralFoldError::FutureUseMismatch { function: 0 },
                _ => LiteralFoldError::LiteralMismatch { function: 0 },
            }),
            "mutation {mutation}"
        );
    }
}

#[test]
fn byte_view_address_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_byte_view_address_inputs(target, 5);
    // The byte-view address consumer is unadmitted under every other policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
        LiteralFoldPolicy::LOAD8_INDEXED_V1,
        LiteralFoldPolicy::COPY_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And a compare-staged consumer is unadmitted under the byte-view
    // address policy even though its producer shape matches.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn byte_view_address_fold_rejects_consumer_operands_carrying_unit_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The rewrite rebuilds the projection's operands wholesale from the
    // `AddressOffset` constraint row; an operand carrying a unit binding
    // would have it silently dropped, so the producer and the independent
    // replay both reject decorated consumers.
    for mutation in 0..3 {
        let inputs = staged_byte_view_address_inputs(target, 5);
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
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
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
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn byte_view_address_backing_fold_rewrites_the_backing_operand_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let address_offset_key = keys.address_offset.unwrap();
        let address_offset_row = environment.constraint(address_offset_key).unwrap();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_byte_view_address_backing_inputs(target, 5);

        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .unwrap_or_else(|error| {
            panic!("byte-view backing fold on {target:?} should validate: {error:?}")
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
        // The surviving register is the operand-1 offset `Use` the rewritten
        // row's base `Use` position binds; the folded backing register at
        // operand 0 is removed.
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, address_offset_key);

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 2);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 5 }
        );
        assert_eq!(rewritten.constraint, address_offset_key);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.implicit_uses, address_offset_row.implicit_uses);
        assert_eq!(rewritten.implicit_defs, address_offset_row.implicit_defs);
        assert_eq!(rewritten.clobbers, address_offset_row.clobbers);
        assert_eq!(rewritten.provenance.operations.len(), 2);
    }
}

#[test]
fn byte_view_address_backing_fold_rejects_an_unencodable_backing_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest byte-offset field any target's
        // `AddressOffset` encoder admits — aarch64 `add`'s 12-bit unsigned
        // immediate — so 4096 cannot fold into a byte offset at either
        // operand position.
        let inputs = staged_byte_view_address_backing_inputs(target, 4096);
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
            )
            .map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn byte_view_address_backing_fold_admits_the_widest_encodable_backing_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // 4095 is the widest offset the shared 12-bit byte-offset bound
        // admits — the boundary the unencodable 4096 sits just past.
        let inputs = staged_byte_view_address_backing_inputs(target, 4095);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .expect("the boundary backing literal folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 4095 }
        );
        // A zero backing literal folds too: the projection computes the
        // surviving offset register's own address, so the rewritten form
        // carries byte offset zero.
        let inputs = staged_byte_view_address_backing_inputs(target, 0);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .expect("the zero backing literal folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 0);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 0 }
        );
    }
}

#[test]
fn byte_view_address_backing_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_byte_view_address_backing_inputs(target, 5);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        );
    }
}

#[test]
fn byte_view_address_backing_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_byte_view_address_backing_inputs(target, 5);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        );
    }
}

#[test]
fn byte_view_address_backing_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_byte_view_address_backing_inputs(target, 5);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
    )
    .unwrap();

    for mutation in 0..11 {
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
            // The recorded survivor claims the operand-1 register; the
            // replay independently derives the survivor as the operand-1
            // `Use` — `VirtualRegisterId(0)` — so a forged one rejects.
            5 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            6 => plan.functions[0].action = None,
            7 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            8 => plan.usage.candidates += 1,
            // A policy without the byte-view-address bit cannot replay the
            // fold: no `AddressOffset` row binds and the action reconstructs
            // nothing.
            9 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            10 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn byte_view_address_backing_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..6 {
        let mut inputs = staged_byte_view_address_backing_inputs(target, 5);
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
            // The offset position 1 is itself an admitted fold site, but
            // this staged consumer binds the surviving offset register
            // there while the literal register sits at operand 0 — the
            // right grammar rejects the mismatched operand wiring.
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
            // The projection's operand list ends at the operand-2 `Def`
            // result: no enabled grammar admits a literal use there.
            _ => {
                let RecoveryClassification::ImmediateU64RematerializationCandidate {
                    future_uses,
                    ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                future_uses[0].operand = 2;
            }
        }
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
            )
            .map(|_| ()),
            Err(match mutation {
                0 => LiteralFoldError::UnsupportedVictimRole { function: 0 },
                1 | 2 => LiteralFoldError::ConsumerMismatch { function: 0 },
                4 | 5 => LiteralFoldError::FutureUseMismatch { function: 0 },
                _ => LiteralFoldError::LiteralMismatch { function: 0 },
            }),
            "mutation {mutation}"
        );
    }
}

#[test]
fn byte_view_address_backing_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_byte_view_address_backing_inputs(target, 5);
    // The byte-view address consumer is unadmitted under every other policy.
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::COMPARE_V1,
        LiteralFoldPolicy::EXTENSION_V1,
        LiteralFoldPolicy::LOAD8_INDEXED_V1,
        LiteralFoldPolicy::COPY_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And a compare-staged consumer is unadmitted under the byte-view
    // address policy even though its producer shape matches.
    let inputs = staged_inputs(target);
    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn byte_view_address_backing_fold_rejects_consumer_operands_carrying_unit_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The rewrite rebuilds the projection's operands wholesale from the
    // `AddressOffset` constraint row; an operand carrying a unit binding
    // would have it silently dropped, so the producer and the independent
    // replay both reject decorated consumers.
    for mutation in 0..3 {
        let inputs = staged_byte_view_address_backing_inputs(target, 5);
        let mut plan = inputs.selected.transformed().clone();
        let operand = &mut plan.functions[0].blocks[0].instructions[1].operands[1];
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
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
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
            "mutation {mutation} replay"
        );
    }
}

#[test]
fn exact_divide_identity_fold_rewrites_the_divide_to_a_copy_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let inputs = staged_divide_inputs(target);
        let auxiliary_uses = environment
            .constraint(keys.divide_u64)
            .unwrap()
            .operands
            .len()
            - 3;
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1)
            .expect("the staged divide fold should validate");

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        assert_eq!(action.immediate, 1);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(
            action.literal_instruction,
            SelectedInstructionId(auxiliary_uses as u32)
        );
        assert_eq!(
            action.consumer_instruction,
            SelectedInstructionId(auxiliary_uses as u32 + 1)
        );
        assert_eq!(action.immediate_constraint, keys.copy_i64);

        let function = &result.transformed().functions[0];
        // The fold removes only the divisor literal and its register: every
        // auxiliary scratch materialization and register stays, left dead.
        assert_eq!(function.virtual_registers.len(), 2 + auxiliary_uses);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1 + auxiliary_uses);
        for (index, instruction) in instructions[..auxiliary_uses].iter().enumerate() {
            assert_eq!(instruction.id, SelectedInstructionId(index as u32));
            assert_eq!(
                instruction.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                }
            );
        }
        let rewritten = &instructions[auxiliary_uses];
        assert_eq!(rewritten.id, SelectedInstructionId(auxiliary_uses as u32));
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.constraint, keys.copy_i64);
        assert_eq!(rewritten.operands.len(), 2);
        // The rebuilt operand list binds the dividend `Use` and the result
        // `Def` — the register pins and the dropped auxiliary `Use` are
        // gone with the pinned divide form.
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[0].fixed_view, None);
        assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.operands[1].fixed_view, None);
        assert!(rewritten.implicit_uses.is_empty());
        assert!(rewritten.implicit_defs.is_empty());
        assert!(rewritten.clobbers.is_empty());
        // The folded literal's provenance joins the consumer's, and the
        // divide's obligation custody is retained.
        assert_eq!(rewritten.provenance.operations.len(), 2);
        assert_eq!(
            rewritten.provenance.obligations,
            vec![ObligationId::new(7).unwrap()]
        );

        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &function.blocks[0].terminator
        else {
            panic!("conditional branch terminator retained");
        };
        assert_eq!(
            instruction.id,
            SelectedInstructionId(auxiliary_uses as u32 + 1)
        );
        let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
            panic!("return terminator retained");
        };
        assert_eq!(
            instruction.id,
            SelectedInstructionId(auxiliary_uses as u32 + 2)
        );
    }
}

#[test]
fn divide_fold_rejects_a_non_unit_divisor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_divide_inputs(target);
        let auxiliary_uses = environment
            .constraint(keys.divide_u64)
            .unwrap()
            .operands
            .len()
            - 3;
        let mut plan = inputs.selected.transformed().clone();
        // The literal is the divisor only under the identity fold: a divisor
        // of two is a different computation both the producer's declared
        // bound and the replay's re-derived grammar reject.
        plan.functions[0].blocks[0].instructions[auxiliary_uses].kind =
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(2),
            };
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        let mut recovery = inputs.recovery.clone();
        let classification = recovery.plan.functions[0].classification.as_mut().unwrap();
        let RecoveryClassification::ImmediateU64RematerializationCandidate { value, .. } =
            &mut classification.classification
        else {
            panic!("staged classification is the immediate candidate");
        };
        *value = IntegerValue::Unsigned(2);

        assert_eq!(
            fold_selected_incoming_literal(
                &selected,
                &inputs.ranges,
                &inputs.legality,
                &inputs.spill_choices,
                &recovery,
                &inputs.availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                &effect_catalog,
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
                budget(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
        assert_eq!(
            validate_literal_fold(
                &selected,
                &inputs.ranges,
                &inputs.legality,
                &inputs.spill_choices,
                &recovery,
                &inputs.availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                &effect_catalog,
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn divide_fold_rejects_an_auxiliary_operand_without_zero_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let divide_operands = environment
        .constraint(keys.divide_u64)
        .unwrap()
        .operands
        .len();
    assert_eq!(
        divide_operands, 4,
        "the x86-64 divide carries one auxiliary use"
    );

    // The dropped operand is inert only when its register is defined solely
    // by zero materializations: a nonzero materialization, a non-materialize
    // definition, and a `Def`-access auxiliary operand each reject — the
    // producer and the independent replay alike.
    for mutation in 0..3 {
        let inputs = staged_divide_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // The high-half scratch materializes seven, not zero.
            0 => {
                plan.functions[0].blocks[0].instructions[0].kind =
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    };
            }
            // The scratch register is defined by a copy, not a zero
            // materialization at all.
            1 => {
                plan.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::CopyI64;
            }
            // The auxiliary operand defines a register rather than reading
            // the proven-zero scratch.
            _ => {
                plan.functions[0].blocks[0].instructions[2].operands[3].access =
                    RegisterOperandAccess::Def;
            }
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
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
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
            "mutation {mutation} replay"
        );
    }
}
