use super::{
    assert_deterministic_fixed_point, budget, fold_with, restage_literal, staged_divide_inputs,
    staged_divide_zero_dividend_inputs, staged_remainder_inputs, staged_remainder_minus_one_inputs,
    staged_remainder_zero_dividend_inputs, validate,
};
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::{fold_selected_incoming_literal, validate_literal_fold};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn divide_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_divide_inputs(target);
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1);
    }
}

#[test]
fn remainder_fold_rejects_a_non_unit_divisor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_remainder_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        // The literal is the divisor only under the constant fold: a
        // divisor of two is a different computation both the producer's
        // declared bound and the replay's re-derived grammar reject.
        plan.functions[0].blocks[0].instructions[0].kind =
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
                LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
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
fn remainder_fold_rejects_a_dropped_def_without_dead_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let remainder_operands = environment
        .constraint(keys.remainder_i64)
        .unwrap()
        .operands
        .len();
    assert_eq!(
        remainder_operands, 4,
        "the x86-64 remainder carries one dropped scratch def"
    );

    // A `Def` operand past the result is droppable only when its register
    // occurs nowhere else in the function: a `Use` in the scratch position,
    // a scratch register another operand also reads, and a scratch register
    // another operand also defines each reject — the producer and the
    // independent replay alike.
    for mutation in 0..3 {
        let inputs = staged_remainder_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // The operand past the result reads a register rather than
            // writing a dead scratch — not a `Def` the grammar may drop.
            0 => {
                plan.functions[0].blocks[0].instructions[1].operands[3].access =
                    RegisterOperandAccess::Use;
            }
            // The scratch operand binds the dividend register, which the
            // operand-0 `Use` also reads — the dropped `Def` would strand
            // that read's only definition's own operand position.
            1 => {
                plan.functions[0].blocks[0].instructions[1].operands[3].virtual_register =
                    VirtualRegisterId(0);
            }
            // The dividend operand reads the scratch register, which the
            // operand-3 `Def` also writes — the dropped `Def` is not dead.
            _ => {
                plan.functions[0].blocks[0].instructions[1].operands[0].virtual_register =
                    VirtualRegisterId(3);
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
                LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
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
fn remainder_zero_dividend_fold_rejects_a_non_zero_dividend() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_remainder_zero_dividend_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        // The literal is the zero dividend only under the constant fold:
        // a dividend of seven is a different computation both the
        // producer's declared bound and the replay's re-derived grammar
        // reject.
        plan.functions[0].blocks[0].instructions[0].kind =
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7),
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
        *value = IntegerValue::Unsigned(7);

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
                LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
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
fn remainder_zero_dividend_fold_rejects_a_dropped_def_without_dead_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let remainder_operands = environment
        .constraint(keys.remainder_i64)
        .unwrap()
        .operands
        .len();
    assert_eq!(
        remainder_operands, 4,
        "the x86-64 remainder carries one dropped scratch def"
    );

    // A `Def` operand past the result is droppable only when its register
    // occurs nowhere else in the function: a `Use` in the scratch position,
    // a scratch register the dropped divisor `Use` also reads, and a
    // scratch register that `Use` also defines each reject — the producer
    // and the independent replay alike.
    for mutation in 0..3 {
        let inputs = staged_remainder_zero_dividend_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // The operand past the result reads a register rather than
            // writing a dead scratch — not a `Def` the grammar may drop.
            0 => {
                plan.functions[0].blocks[0].instructions[1].operands[3].access =
                    RegisterOperandAccess::Use;
            }
            // The scratch operand binds the divisor register, which the
            // operand-1 `Use` the fold drops also reads — the dropped
            // `Def` would strand that read's own operand position.
            1 => {
                plan.functions[0].blocks[0].instructions[1].operands[3].virtual_register =
                    VirtualRegisterId(0);
            }
            // The divisor operand reads the scratch register, which the
            // operand-3 `Def` also writes — the dropped `Def` is not dead.
            _ => {
                plan.functions[0].blocks[0].instructions[1].operands[1].virtual_register =
                    VirtualRegisterId(3);
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
                LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
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
fn divide_zero_dividend_fold_rejects_a_non_zero_dividend() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the zero dividend only under the constant fold:
        // a dividend of seven is a different computation both the
        // producer's declared bound and the replay's re-derived grammar
        // reject.
        let mut inputs = staged_divide_zero_dividend_inputs(target);
        restage_literal(&mut inputs, 7);
        assert_eq!(
            fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1
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
fn divide_zero_dividend_fold_rejects_a_missing_obligation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_divide_zero_dividend_inputs(target);
        let auxiliary_uses = environment
            .constraint(keys.divide_u64)
            .unwrap()
            .operands
            .len()
            - 3;
        let mut plan = inputs.selected.transformed().clone();
        // The fold's fault discharge is the carried nonzero-divisor
        // obligation, not the folded literal: an instruction record that
        // no longer retains the obligation its kind declares cannot fold
        // — the producer and the independent replay reject alike under
        // the obligation-discharged surface.
        plan.functions[0].blocks[0].instructions[auxiliary_uses + 1]
            .provenance
            .obligations
            .clear();
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
                LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
                budget(),
            ),
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
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
            Err(LiteralFoldError::EffectSurfaceMismatch { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn divide_zero_dividend_fold_rejects_an_auxiliary_operand_without_zero_custody() {
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

    // A `Use` operand past the result is droppable only when its register
    // is defined solely by zero materializations — the fold discards
    // whatever the operand carried, and a literal of zero at operand 0
    // fixes only the low dividend half an x86-64 `div` reads. A nonzero
    // materialization, a non-materialize definition, a `Def`-access
    // auxiliary operand, and a register no instruction defines each
    // reject — the producer and the independent replay alike.
    for mutation in 0..4 {
        let inputs = staged_divide_zero_dividend_inputs(target);
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
            // The auxiliary operand defines a register rather than
            // reading the proven-zero scratch.
            2 => {
                plan.functions[0].blocks[0].instructions[2].operands[3].access =
                    RegisterOperandAccess::Def;
            }
            // The auxiliary operand reads the entry-parameter register no
            // instruction defines — not a zero materialization.
            _ => {
                plan.functions[0].blocks[0].instructions[2].operands[3].virtual_register =
                    VirtualRegisterId(0);
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
                LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
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
fn divide_zero_dividend_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_divide_zero_dividend_inputs(target);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
    )
    .expect("the staged zero-dividend divide fold should validate");

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the divide's own `Def`,
            // not the dropped divisor `Use`.
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            1 => plan.functions[0].action.as_mut().unwrap().result = None,
            // The recorded immediate is the materialized constant zero;
            // any substitution replays differently.
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
            // A policy without the zero-dividend bit cannot replay the
            // fold: the sibling family's row covers only the divisor
            // position, so the action reconstructs nothing.
            8 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            // The surviving register is the dropped divisor: recording
            // the auxiliary `Use` register instead fails the re-derived
            // action.
            _ => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(3),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn remainder_minus_one_fold_rejects_a_non_minus_one_divisor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the minus-one divisor only under this fold: a
        // divisor of zero, of one — the sibling divisor-one family's own
        // admitted value — of two, or of `u64::MAX - 1` is a different
        // computation both the producer's declared bound and the replay's
        // re-derived grammar reject.
        for value in [0, 1, 2, u64::MAX - 1] {
            let mut inputs = staged_remainder_minus_one_inputs(target);
            restage_literal(&mut inputs, value);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} value {value}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "{target:?} value {value} replay"
            );
        }
    }
}

#[test]
fn remainder_minus_one_fold_dispatches_on_the_literal_value_with_sibling_families_enabled() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // With every remainder family enabled the folded literal's
        // operand position and value select the grammar without
        // ambiguity: each of the three fixtures admits exactly one pair.
        let all_remainder = LiteralFoldPolicy::WRAPPING_REMAINDER_V1
            .union(LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1)
            .union(LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1);
        for inputs in [
            staged_remainder_inputs(target),
            staged_remainder_zero_dividend_inputs(target),
            staged_remainder_minus_one_inputs(target),
        ] {
            let result = fold_with(&inputs, &environment, all_remainder)
                .expect("the staged remainder fold should validate");
            assert_eq!(result.receipt().applied_count(), 1, "{target:?}");
        }
    }
}
