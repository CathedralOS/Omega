use super::{
    assert_budget_is_enforced, assert_deterministic_fixed_point, budget, fold_with, policy_without,
    policy_without_all, restage_literal, staged_divide_inputs, staged_divide_zero_dividend_inputs,
    staged_inputs, staged_remainder_inputs, staged_remainder_minus_one_inputs,
    staged_remainder_zero_dividend_inputs, validate,
};
use crate::RecoveryClassification;
use crate::{
    LiteralFoldError, LiteralFoldPolicy, fold_selected_incoming_literal, validate_literal_fold,
    validated_machine_effect_catalog,
};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedTerminator, VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, ObligationId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn divide_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_divide_inputs(target);

    // The pinned divide form carries `fixed_view` decorations the rewrite
    // deliberately drops; `tied_to` and `early_clobber` have no carried
    // meaning once the operand list is rebuilt and reject under the declared
    // unit-effect surface.
    for mutation in 0..2 {
        let mut plan = inputs.selected.transformed().clone();
        let operand = &mut plan.functions[0].blocks[0].instructions[2].operands[1];
        match mutation {
            0 => operand.tied_to = Some(0),
            _ => operand.early_clobber = true,
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

#[test]
fn divide_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The divide's operand grammar admits the literal only under the
    // divide-identity policy: another selected family sees no admitted
    // consumer kind.
    let inputs = staged_divide_inputs(target);
    assert_eq!(
        fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    // And the divide policy admits no other consumer: the compare fixture's
    // flag-defining consumer has no `Use` operand 1 the divide grammar
    // could bind.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(&compare, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn divide_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_divide_inputs(target);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1)
        .expect("the staged divide fold should validate");

    for mutation in 0..10 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the divide's own `Def`, not
            // the dividend the copy reads.
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            // The recorded immediate is the folded divisor: only one is
            // admitted, so any substitution replays differently.
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
            // The surviving register is the dividend: recording the dropped
            // auxiliary scratch instead binds the wrong `Use`.
            _ => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(3),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn divide_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_divide_inputs(target);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1);
    }
}

#[test]
fn divide_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_divide_inputs(target);
        assert_deterministic_fixed_point(&inputs, &environment, LiteralFoldPolicy::EXACT_DIVIDE_V1);
    }
}

#[test]
fn wrapping_remainder_one_fold_rewrites_the_remainder_to_a_zero_materialization_on_both_linux_targets()
 {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let inputs = staged_remainder_inputs(target);
        let scratch_defs = environment
            .constraint(keys.remainder_i64)
            .unwrap()
            .operands
            .len()
            - 3;
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        )
        .expect("the staged remainder fold should validate");

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        // The recorded immediate is the folded constant the rewritten
        // `MaterializeI64` embeds — zero — not the folded divisor literal.
        assert_eq!(action.immediate, 0);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, keys.materialize_i64);

        let function = &result.transformed().functions[0];
        // The fold removes only the divisor literal and its register: the
        // dividend register and every dead scratch `Def` register stay
        // declared, left unreferenced by the rebuilt operand list.
        assert_eq!(function.virtual_registers.len(), 2 + scratch_defs);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            }
        );
        assert_eq!(rewritten.constraint, keys.materialize_i64);
        assert_eq!(rewritten.operands.len(), 1);
        // The rebuilt operand list binds only the result `Def` — the
        // register pins, the early-clobber scratch, and the dropped
        // dividend `Use` are gone with the pinned remainder form.
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.operands[0].fixed_view, None);
        assert!(!rewritten.operands[0].early_clobber);
        assert!(rewritten.implicit_uses.is_empty());
        assert!(rewritten.implicit_defs.is_empty());
        assert!(rewritten.clobbers.is_empty());
        // The folded literal's provenance joins the consumer's, and the
        // remainder's obligation custody is retained.
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
        assert_eq!(instruction.id, SelectedInstructionId(1));
        let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
            panic!("return terminator retained");
        };
        assert_eq!(instruction.id, SelectedInstructionId(2));
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
fn remainder_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_remainder_inputs(target);

    // The pinned remainder form carries `fixed_view` decorations and an
    // early-clobber scratch the rewrite deliberately drops with the folded
    // operand list; `tied_to` has no carried meaning once the operand list
    // is rebuilt and rejects under the declared unit-effect surface.
    let mut plan = inputs.selected.transformed().clone();
    plan.functions[0].blocks[0].instructions[1].operands[1].tied_to = Some(0);
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
        Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
        "replay"
    );
}

#[test]
fn remainder_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The remainder's operand grammar admits the literal only under the
    // remainder policies: a selection naming no remainder family sees no
    // admitted consumer kind — including the strongest posture, every
    // other rule enabled at once with all three remainder bits closed.
    let inputs = staged_remainder_inputs(target);
    for policy in [
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        policy_without_all(&[
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        ]),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The sibling remainder family admits the same consumer kind at the
    // operand-0 dividend position: with only the zero-dividend bit set —
    // or with every family enabled except the two divisor bits — the kind
    // is admitted but no enabled grammar covers the operand-1 divisor
    // position the staged literal occupies: a future-use mismatch, not an
    // unadmitted consumer.
    for policy in [
        LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
        policy_without_all(&[
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        ]),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The operand-1 divisor position is also admitted by the value-disjoint
    // minus-one family: with only its bit set the kind and the position are
    // admitted and the divisor literal of one alone is what rejects — an
    // unsupported immediate, not a consumer or position mismatch.
    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::UnsupportedImmediate { function: 0 })
    );
    // And the remainder policy admits no other consumer: the compare
    // fixture's flag-defining consumer has no `Def` operand 2 the
    // constant-result grammar could bind.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(
            &compare,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn remainder_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_remainder_inputs(target);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
    )
    .expect("the staged remainder fold should validate");

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the remainder's own `Def`,
            // not the dropped dividend `Use`.
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
            // A policy without the remainder bit cannot replay the fold:
            // no `MaterializeI64` row binds for this consumer and the
            // action reconstructs nothing.
            8 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            // The surviving register is the dropped dividend: recording
            // the scratch `Def` register instead fails the re-derived
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
fn remainder_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_inputs(target);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        );
    }
}

#[test]
fn remainder_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        );
    }
}

#[test]
fn wrapping_remainder_zero_dividend_fold_rewrites_the_remainder_to_a_zero_materialization_on_both_linux_targets()
 {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let inputs = staged_remainder_zero_dividend_inputs(target);
        let scratch_defs = environment
            .constraint(keys.remainder_i64)
            .unwrap()
            .operands
            .len()
            - 3;
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
        )
        .expect("the staged zero-dividend remainder fold should validate");

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        // The recorded immediate is the folded constant the rewritten
        // `MaterializeI64` embeds — zero — which the folded zero dividend
        // already is.
        assert_eq!(action.immediate, 0);
        // The surviving register records the dropped operand-1 divisor
        // `Use` for custody; the rewritten row binds no `Use` at all.
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, keys.materialize_i64);

        let function = &result.transformed().functions[0];
        // The fold removes only the dividend literal and its register:
        // the divisor register and every dead scratch `Def` register stay
        // declared, left unreferenced by the rebuilt operand list.
        assert_eq!(function.virtual_registers.len(), 2 + scratch_defs);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            }
        );
        assert_eq!(rewritten.constraint, keys.materialize_i64);
        assert_eq!(rewritten.operands.len(), 1);
        // The rebuilt operand list binds only the result `Def` — the
        // register pins, the early-clobber scratch, and the dropped
        // divisor `Use` are gone with the pinned remainder form.
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.operands[0].fixed_view, None);
        assert!(!rewritten.operands[0].early_clobber);
        assert!(rewritten.implicit_uses.is_empty());
        assert!(rewritten.implicit_defs.is_empty());
        assert!(rewritten.clobbers.is_empty());
        // The folded literal's provenance joins the consumer's, and the
        // remainder's obligation custody is retained — the fold's fault
        // discharge still rests on the carried nonzero-divisor proof.
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
        assert_eq!(instruction.id, SelectedInstructionId(1));
        let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
            panic!("return terminator retained");
        };
        assert_eq!(instruction.id, SelectedInstructionId(2));
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
fn remainder_zero_dividend_fold_rejects_a_missing_obligation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_remainder_zero_dividend_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        // The fold's fault discharge is the carried nonzero-divisor
        // obligation, not the folded literal: an instruction record that
        // no longer retains the obligation its kind declares cannot fold
        // — the producer and the independent replay reject alike under
        // the obligation-discharged surface.
        plan.functions[0].blocks[0].instructions[1]
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
                LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
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
fn remainder_zero_dividend_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_remainder_zero_dividend_inputs(target);

    // The pinned remainder form carries `fixed_view` decorations and an
    // early-clobber scratch the rewrite deliberately drops with the folded
    // operand list; `tied_to` has no carried meaning once the operand list
    // is rebuilt and rejects under the declared unit-effect surface at
    // either `Use` position.
    for position in 0..2 {
        let mut plan = inputs.selected.transformed().clone();
        plan.functions[0].blocks[0].instructions[1].operands[position].tied_to = Some(0);
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
            "position {position}"
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
            "position {position} replay"
        );
    }
}

#[test]
fn remainder_zero_dividend_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The zero-dividend operand grammar admits the literal only under its
    // own policy: a selection naming no remainder family sees no admitted
    // consumer kind — including the strongest posture, every other rule
    // enabled at once with all three remainder bits closed.
    let inputs = staged_remainder_zero_dividend_inputs(target);
    for policy in [
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        policy_without_all(&[
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        ]),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The sibling remainder family admits the same consumer kind at the
    // operand-1 divisor position: with only the divisor-one bit set — or
    // with every family enabled except the zero-dividend bit — the kind
    // is admitted but no enabled grammar covers the operand-0 dividend
    // position the staged literal occupies: a future-use mismatch, not an
    // unadmitted consumer.
    for policy in [
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        policy_without(LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And the zero-dividend policy admits no other consumer: the compare
    // fixture's flag-defining consumer has no `Def` operand 2 the
    // constant-result grammar could bind.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(
            &compare,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn remainder_zero_dividend_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_remainder_zero_dividend_inputs(target);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
    )
    .expect("the staged zero-dividend remainder fold should validate");

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the remainder's own `Def`,
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
            // the scratch `Def` register instead fails the re-derived
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
fn remainder_zero_dividend_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_zero_dividend_inputs(target);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
        );
    }
}

#[test]
fn remainder_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_zero_dividend_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
        );
    }
}

#[test]
fn exact_divide_zero_dividend_fold_rewrites_the_divide_to_a_zero_materialization_on_both_linux_targets()
 {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let inputs = staged_divide_zero_dividend_inputs(target);
        let auxiliary_uses = environment
            .constraint(keys.divide_u64)
            .unwrap()
            .operands
            .len()
            - 3;
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
        )
        .expect("the staged zero-dividend divide fold should validate");

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        // The recorded immediate is the folded constant the rewritten
        // `MaterializeI64` embeds — zero — which the folded zero dividend
        // already is.
        assert_eq!(action.immediate, 0);
        // The surviving register records the dropped operand-1 divisor
        // `Use` for custody; the rewritten row binds no `Use` at all.
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
        assert_eq!(action.immediate_constraint, keys.materialize_i64);

        let function = &result.transformed().functions[0];
        // The fold removes only the dividend literal and its register:
        // every auxiliary scratch materialization and register stays,
        // left dead.
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
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            }
        );
        assert_eq!(rewritten.constraint, keys.materialize_i64);
        assert_eq!(rewritten.operands.len(), 1);
        // The rebuilt operand list binds only the result `Def` — the
        // register pins, the dropped divisor `Use`, and the dropped
        // auxiliary `Use` are gone with the pinned divide form.
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.operands[0].fixed_view, None);
        assert!(!rewritten.operands[0].early_clobber);
        assert!(rewritten.implicit_uses.is_empty());
        assert!(rewritten.implicit_defs.is_empty());
        assert!(rewritten.clobbers.is_empty());
        // The folded literal's provenance joins the consumer's, and the
        // divide's obligation custody is retained — the fold's fault
        // discharge still rests on the carried nonzero-divisor proof.
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
fn divide_zero_dividend_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_divide_zero_dividend_inputs(target);

    // The pinned divide form carries `fixed_view` decorations the rewrite
    // deliberately drops with the folded operand list; `tied_to` and
    // `early_clobber` have no carried meaning once the operand list is
    // rebuilt and reject under the declared unit-effect surface at either
    // leading `Use` position.
    for (position, mutation) in [(0u16, 0u8), (1, 0), (0, 1), (3, 0)] {
        let mut plan = inputs.selected.transformed().clone();
        let operand =
            &mut plan.functions[0].blocks[0].instructions[2].operands[usize::from(position)];
        match mutation {
            0 => operand.tied_to = Some(0),
            _ => operand.early_clobber = true,
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
            "position {position} mutation {mutation}"
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
            "position {position} mutation {mutation} replay"
        );
    }
}

#[test]
fn divide_zero_dividend_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The zero-dividend operand grammar admits the literal only under its
    // own policy: a selection naming no divide family sees no admitted
    // consumer kind — including the strongest posture, every other rule
    // enabled at once with both divide bits closed.
    let inputs = staged_divide_zero_dividend_inputs(target);
    for policy in [
        LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
        policy_without_all(&[
            LiteralFoldPolicy::EXACT_DIVIDE_V1,
            LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
        ]),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The sibling divide family admits the same consumer kind at the
    // operand-1 divisor position: with only the divisor-one bit set — or
    // with every family enabled except the zero-dividend bit — the kind
    // is admitted but no enabled grammar covers the operand-0 dividend
    // position the staged literal occupies: a future-use mismatch, not an
    // unadmitted consumer.
    for policy in [
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        policy_without(LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::FutureUseMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And the zero-dividend policy admits no other consumer: the compare
    // fixture's flag-defining consumer has no `Def` operand 2 the
    // constant-result grammar could bind.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(
            &compare,
            &environment,
            LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
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
fn divide_zero_dividend_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_divide_zero_dividend_inputs(target);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
        );
    }
}

#[test]
fn divide_zero_dividend_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_divide_zero_dividend_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
        );
    }
}

#[test]
fn wrapping_remainder_minus_one_fold_rewrites_the_remainder_to_a_zero_materialization_on_both_linux_targets()
 {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let inputs = staged_remainder_minus_one_inputs(target);
        let scratch_defs = environment
            .constraint(keys.remainder_i64)
            .unwrap()
            .operands
            .len()
            - 3;
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        )
        .expect("the staged remainder minus-one fold should validate");

        assert_eq!(result.receipt().applied_count(), 1);
        let action = result.plan().functions[0].action.unwrap();
        assert_eq!(action.result, Some(VirtualRegisterId(2)));
        // The recorded immediate is the folded constant the rewritten
        // `MaterializeI64` embeds — zero, `x % -1` for every `x` — not
        // the folded all-ones divisor literal.
        assert_eq!(action.immediate, 0);
        assert_eq!(action.surviving, VirtualRegisterId(0));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.literal_instruction, SelectedInstructionId(0));
        assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
        assert_eq!(action.immediate_constraint, keys.materialize_i64);

        let function = &result.transformed().functions[0];
        // The fold removes only the divisor literal and its register: the
        // dividend register and every dead scratch `Def` register stay
        // declared, left unreferenced by the rebuilt operand list.
        assert_eq!(function.virtual_registers.len(), 2 + scratch_defs);
        let instructions = &function.blocks[0].instructions;
        assert_eq!(instructions.len(), 1);
        let rewritten = &instructions[0];
        assert_eq!(rewritten.id, SelectedInstructionId(0));
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            }
        );
        assert_eq!(rewritten.constraint, keys.materialize_i64);
        assert_eq!(rewritten.operands.len(), 1);
        // The rebuilt operand list binds only the result `Def` — the
        // register pins, the early-clobber scratch, and the dropped
        // dividend `Use` are gone with the pinned remainder form.
        assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(1));
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.operands[0].fixed_view, None);
        assert!(!rewritten.operands[0].early_clobber);
        assert!(rewritten.implicit_uses.is_empty());
        assert!(rewritten.implicit_defs.is_empty());
        assert!(rewritten.clobbers.is_empty());
        // The folded literal's provenance joins the consumer's, and the
        // remainder's obligation custody is retained even though the
        // minus-one fold's fault discharge is the divisor literal
        // itself rather than the carried obligation.
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
        assert_eq!(instruction.id, SelectedInstructionId(1));
        let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
            panic!("return terminator retained");
        };
        assert_eq!(instruction.id, SelectedInstructionId(2));
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
fn remainder_minus_one_fold_rejects_a_dropped_def_without_dead_custody() {
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
        let inputs = staged_remainder_minus_one_inputs(target);
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
                LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
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
fn remainder_minus_one_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let inputs = staged_remainder_minus_one_inputs(target);

    // The pinned remainder form carries `fixed_view` decorations and an
    // early-clobber scratch the rewrite deliberately drops with the folded
    // operand list; `tied_to` has no carried meaning once the operand list
    // is rebuilt and rejects under the declared unit-effect surface.
    let mut plan = inputs.selected.transformed().clone();
    plan.functions[0].blocks[0].instructions[1].operands[1].tied_to = Some(0);
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
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
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
        Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
        "replay"
    );
}

#[test]
fn remainder_minus_one_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The remainder's operand grammar admits the literal only under the
    // remainder policies: a selection naming no remainder family sees no
    // admitted consumer kind — including the strongest posture, every
    // other rule enabled at once with all three remainder bits closed.
    let inputs = staged_remainder_minus_one_inputs(target);
    for policy in [
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        policy_without_all(&[
            LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        ]),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The sibling remainder family admits the same consumer kind at the
    // operand-0 dividend position: with only the zero-dividend bit set the
    // kind is admitted but no enabled grammar covers the operand-1
    // divisor position the staged literal occupies: a future-use
    // mismatch, not an unadmitted consumer.
    assert_eq!(
        fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::FutureUseMismatch { function: 0 })
    );
    // The operand-1 divisor position is shared with the value-disjoint
    // divisor-one family: with only its bit set — or with every family
    // enabled except the minus-one bit — the kind and the position are
    // admitted and the all-ones literal alone is what rejects — an
    // unsupported immediate, not a consumer or position mismatch.
    for policy in [
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        policy_without(LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{policy:?}"
        );
    }
    // And the minus-one policy admits no other consumer: the compare
    // fixture's flag-defining consumer has no `Def` operand 2 the
    // constant-result grammar could bind.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(
            &compare,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
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

#[test]
fn remainder_minus_one_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = staged_remainder_minus_one_inputs(target);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
    )
    .expect("the staged remainder minus-one fold should validate");

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the remainder's own `Def`,
            // not the dropped dividend `Use`.
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
            // A policy without the minus-one bit cannot replay the fold:
            // the sibling divisor-one row rejects the all-ones literal
            // and the action reconstructs nothing.
            8 => plan.policy = LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            // The surviving register is the dropped dividend: recording
            // the scratch `Def` register instead fails the re-derived
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
fn remainder_minus_one_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_minus_one_inputs(target);
        assert_budget_is_enforced(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        );
    }
}

#[test]
fn remainder_minus_one_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_remainder_minus_one_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        );
    }
}
