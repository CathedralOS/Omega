use super::{
    CLASSIFICATION_CORRUPTIONS, assert_classification_does_not_steer, assert_declines,
    corrupt_classification, fold_with, restage_literal, staged_add_inputs, staged_inputs,
    staged_subtract_inputs,
};
use crate::LiteralFoldPolicy;
use crate::analyses::validated_machine_effect_catalog;
use optimization_core::AcceptedObligationFactIdentity;
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId, VirtualRegisterOrigin,
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
                result.plan().functions[0].actions[0].immediate_constraint,
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
            result.plan().functions[0].actions[0].immediate_constraint,
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
        let action = result.plan().functions[0].actions[0];
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

/// Corrupting only the recovery classification row cannot redirect or
/// suppress the subtract-immediate fold: nomination reads the stream.
#[test]
fn subtract_fold_ignores_corrupted_classification_rows() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for corruption in 0..CLASSIFICATION_CORRUPTIONS {
        let mut inputs = staged_subtract_inputs(target);
        corrupt_classification(&mut inputs, corruption);
        let result = assert_classification_does_not_steer(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_SUBTRACT_V1,
            &format!("corruption {corruption}"),
        );
        assert_eq!(
            result.receipt().applied_count(),
            1,
            "corruption {corruption}"
        );
    }
}

#[test]
fn add_fold_ignores_a_classification_claiming_another_operand() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; the
        // recovery classification claims `claimed_operand` instead. The
        // classification no longer nominates, so the claim neither
        // redirects nor suppresses the fold the stream itself admits.
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
            assert_classification_does_not_steer(
                &inputs,
                &environment,
                LiteralFoldPolicy::EXACT_ADD_V1,
                &format!(
                    "literal at operand {literal_operand} claiming {claimed_operand} on {target:?}"
                ),
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
            assert_eq!(result.plan().functions[0].actions[0].immediate, 4095);
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
            assert_declines(
                &inputs,
                &environment,
                LiteralFoldPolicy::EXACT_ADD_V1,
                &format!("literal at operand {literal_operand} on {target:?}"),
            );
        }
    }
}

#[test]
fn fold_declines_instructions_the_bound_effect_catalog_does_not_declare() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
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
            let mut inputs = inputs;
            inputs.selected.transformed = Arc::new(plan);
            // A record the bound catalog does not declare is a surface the
            // fold cannot prove isolated: the candidate declines.
            assert_declines(
                &inputs,
                &environment,
                LiteralFoldPolicy::COMPARE_V1,
                &format!("mutation {mutation} on {target:?}"),
            );
        }
    }
}
