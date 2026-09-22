use super::{
    assert_budget_is_enforced, assert_deterministic_fixed_point, budget, fold, fold_with,
    policy_without, restage_literal, staged_add_inputs, staged_inputs, staged_subtract_inputs,
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
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ObligationId, ValueId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn compare_immediate_fold_rewrites_the_flag_defining_consumer_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let compare_row = environment
            .constraint(
                environment
                    .allocation_constraint_keys()
                    .compare_i64_immediate,
            )
            .unwrap();
        let inputs = staged_inputs(target);
        let result = fold(&inputs, &environment);

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, None);
        assert_eq!(action.immediate, 5);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(
            action.immediate_constraint,
            environment
                .allocation_constraint_keys()
                .compare_i64_immediate
        );

        let function = &result.transformed().functions[0];
        assert_eq!(function.virtual_registers.len(), 1);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(5),
            }
        );
        assert_eq!(
            rewritten.constraint,
            environment
                .allocation_constraint_keys()
                .compare_i64_immediate
        );
        assert_eq!(rewritten.operands.len(), 1);
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.implicit_defs, compare_row.implicit_defs);
        // The folded literal's provenance joins the consumer's.
        assert_eq!(rewritten.provenance.operations.len(), 2);

        // Densification keeps instruction ids dense through the terminator.
        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &function.blocks[0].terminator
        else {
            panic!("conditional branch terminator retained");
        };
        assert_eq!(instruction.id, SelectedInstructionId(1));
        let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
            panic!("return terminator retained");
        };
        assert_eq!(instruction.id, SelectedInstructionId(2));
    }
}

#[test]
fn compare_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_inputs(target);
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
            // A plan binding a different effect catalog is a root mismatch:
            // the replay refuses to re-derive the fold under a foreign
            // declaration set.
            8 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn compare_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest immediate any target's
        // compare-immediate encoder admits — the shared 12-bit encoding
        // limit: 4095 folds and 4096 cannot.
        let mut inputs = staged_inputs(target);
        restage_literal(&mut inputs, 4095);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1)
            .expect("the boundary immediate folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
            }
        );

        let mut inputs = staged_inputs(target);
        restage_literal(&mut inputs, 4096);
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
    }
}

#[test]
fn compare_fold_is_disabled_without_the_compare_bit() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_inputs(target);
        // Every other selected-lowering family enabled — and the fully empty
        // policy — both leave the compare bit's gate closed: the fold cannot
        // fire without it.
        for policy in [
            policy_without(LiteralFoldPolicy::COMPARE_V1),
            LiteralFoldPolicy::empty(),
        ] {
            assert_eq!(
                fold_with(&inputs, &environment, policy).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{target:?}"
            );
        }
    }
}

#[test]
fn compare_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_inputs(target);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1);
    }
}

#[test]
fn compare_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_inputs(target);
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::COMPARE_V1);
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
fn subtract_fold_admits_the_u12_boundary_immediate_and_rejects_beyond_it() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The declared bound is the narrowest immediate any target's
        // subtract-immediate encoder admits — the shared 12-bit encoding
        // limit: 4095 folds and 4096 cannot.
        let mut inputs = staged_subtract_inputs(target);
        restage_literal(&mut inputs, 4095);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1)
            .expect("the boundary immediate folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
                obligation: ObligationId::new(7).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
            }
        );

        let mut inputs = staged_subtract_inputs(target);
        restage_literal(&mut inputs, 4096);
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
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
fn subtract_fold_is_disabled_without_the_subtract_bit() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_subtract_inputs(target);
        // Every other selected-lowering family enabled — and the fully empty
        // policy — both leave the subtract bit's gate closed: the fold cannot
        // fire without it.
        for policy in [
            policy_without(LiteralFoldPolicy::EXACT_SUBTRACT_V1),
            LiteralFoldPolicy::empty(),
        ] {
            assert_eq!(
                fold_with(&inputs, &environment, policy).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "{target:?}"
            );
        }
    }
}

#[test]
fn subtract_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_subtract_inputs(target);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).unwrap();

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The subtract's `Def` result is decision-bearing: dropping it or
            // rebinding it to the surviving operand replays differently.
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
            // The rewritten `Use` must bind the surviving register, not the
            // removed victim.
            5 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            6 => plan.functions[0].action.as_mut().unwrap().victim = VirtualRegisterId(0),
            7 => plan.functions[0].action = None,
            8 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            9 => plan.usage.candidates += 1,
            // A policy without the subtract bit cannot replay the fold: no
            // subtract-immediate row binds and the action reconstructs
            // nothing.
            10 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            _ => unreachable!(),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn subtract_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_subtract_inputs(target);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1);
    }
}

#[test]
fn subtract_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_subtract_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        );
    }
}

#[test]
fn add_immediate_fold_commutes_the_literal_operand_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let immediate_row = environment.constraint(keys.add_i64_immediate).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1)
                .unwrap_or_else(|error| {
                    panic!(
                        "add fold with the literal at operand {literal_operand} on {target:?} \
                         should validate: {error:?}"
                    )
                });

            assert_eq!(
                result.plan().machine_effect_catalog,
                effect_catalog.identity()
            );
            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            assert_eq!(action.immediate, 5);
            // The action records the surviving register, not the operand
            // position it occupied: `VirtualRegisterId(0)` binds the
            // rewritten row's `Use` position under either grammar.
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, keys.add_i64_immediate);

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
            // Both grammars rewrite to the same immediate form with proof
            // custody carried from the source consumer.
            assert_eq!(
                rewritten.kind,
                SelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(5),
                    obligation: ObligationId::new(7).unwrap(),
                    accepted_fact: AcceptedObligationFactIdentity::from_bytes([9; 32]),
                }
            );
            assert_eq!(rewritten.constraint, keys.add_i64_immediate);
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert_eq!(rewritten.implicit_uses, immediate_row.implicit_uses);
            assert_eq!(rewritten.implicit_defs, immediate_row.implicit_defs);
            assert_eq!(rewritten.clobbers, immediate_row.clobbers);
            assert_eq!(rewritten.provenance.operations.len(), 2);
        }
    }
}

#[test]
fn add_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The left-literal grammar exercises the operand-0 fold.
    let inputs = staged_add_inputs(target, 0);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).unwrap();

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            0 => plan.functions[0].action.as_mut().unwrap().result = None,
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
            // The survivor record is decision-bearing: binding the removed
            // victim register into the rewritten `Use` must not validate.
            4 => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(1),
            5 => plan.functions[0].action.as_mut().unwrap().victim = VirtualRegisterId(0),
            6 => plan.functions[0].action = None,
            7 => plan.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([99; 32]),
            8 => plan.usage.candidates += 1,
            9 => plan.policy = LiteralFoldPolicy::COMPARE_V1,
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
fn add_fold_rejects_malformed_left_grammar_operand_arrangements() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for mutation in 0..4 {
        let mut inputs = staged_add_inputs(target, 0);
        let mut plan = inputs.selected.transformed().clone();
        let consumer = &mut plan.functions[0].blocks[0].instructions[1];
        match mutation {
            // The folded operand is a `Def`, not a `Use`.
            0 => consumer.operands[0].access = RegisterOperandAccess::Def,
            // The surviving position is not a `Use`.
            1 => consumer.operands[1].access = RegisterOperandAccess::Def,
            // The result `Def` is missing from the binary grammar.
            2 => {
                consumer.operands.pop();
            }
            // An extra operand exceeds the binary grammar.
            _ => consumer.operands.push(consumer.operands[1]),
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        inputs.selected = selected;
        assert_eq!(
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1).map(|_| ()),
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
fn add_fold_is_disabled_without_the_add_bit() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Every other selected-lowering family enabled — and the fully empty
        // policy — both leave the add bit's gate closed: neither operand
        // grammar can fire without it.
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            for policy in [
                policy_without(LiteralFoldPolicy::EXACT_ADD_V1),
                LiteralFoldPolicy::empty(),
            ] {
                assert_eq!(
                    fold_with(&inputs, &environment, policy).map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "literal at operand {literal_operand} on {target:?}"
                );
            }
        }
    }
}

#[test]
fn add_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1);
        }
    }
}

#[test]
fn add_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_add_inputs(target, literal_operand);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::EXACT_ADD_V1,
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

#[test]
fn fold_rejects_a_literal_record_carrying_implicit_unit_traffic() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let condition_unit = environment
            .constraint(keys.compare_i64)
            .unwrap()
            .implicit_defs[0];
        let inputs = staged_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        // The eliminated instruction must carry no unit traffic at all: an
        // implicit condition-state definition on the literal would be dropped
        // silently by its removal.
        plan.functions[0].blocks[0].instructions[0]
            .implicit_defs
            .push(condition_unit);
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
            Err(LiteralFoldError::LiteralMismatch { function: 0 }),
            "{target:?}"
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
            Err(LiteralFoldError::LiteralMismatch { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn compare_fold_rejects_unadmitted_candidate_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    for mutation in 0..5 {
        let inputs = staged_inputs(target);
        let mut recovery = inputs.recovery.clone();
        let slot = recovery.plan.functions[0].classification.as_mut().unwrap();
        match mutation {
            0 => {
                slot.role = RecoveryVictimRole::ActiveResident {
                    current_view: register_model::RegisterViewId(0),
                    reclaimed_view: register_model::RegisterViewId(0),
                }
            }
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
                    value, ..
                } = &mut slot.classification
                else {
                    unreachable!()
                };
                *value = IntegerValue::Unsigned(4096);
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
        assert!(
            fold_selected_incoming_literal(
                &inputs.selected,
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
                LiteralFoldPolicy::COMPARE_V1,
                budget(),
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}
