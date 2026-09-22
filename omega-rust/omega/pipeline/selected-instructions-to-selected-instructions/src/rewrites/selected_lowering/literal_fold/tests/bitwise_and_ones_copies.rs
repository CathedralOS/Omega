use super::{
    assert_budget_is_enforced, assert_deterministic_fixed_point, budget, fold_with, policy_without,
    restage_literal, staged_and_inputs, staged_and_ones_inputs, staged_inputs, staged_xor_inputs,
    validate,
};
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::{fold_selected_incoming_literal, validate_literal_fold};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedOperand, SelectedTerminator, VirtualRegisterId,
};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn and_ones_fold_rewrites_the_consumer_to_a_surviving_operand_copy_on_both_linux_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        // The and form binds the flag-clobbering subtract constraint row:
        // on x86-64 the consumer carries an `rflags` clobber the fold drops
        // with the folded form, on aarch64 the `and` row is already
        // flag-free.
        let consumer_clobbers = environment
            .constraint(keys.subtract_i64)
            .unwrap()
            .clobbers
            .len();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_and_ones_inputs(target, literal_operand);
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::BITWISE_AND_ONES_V1,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "and-ones fold with the literal at operand {literal_operand} on {target:?} \
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
            // The recorded immediate is the folded literal itself — all
            // ones — under either operand grammar; the `CopyI64` rewrite
            // binds the surviving register rather than embedding a
            // constant.
            assert_eq!(action.immediate, u64::MAX);
            // The surviving record is the non-victim `Use` register,
            // whichever operand position the folded literal occupied.
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, keys.copy_i64);

            let function = &result.transformed().functions[0];
            // The fold removes only the all-ones literal and its register:
            // the surviving operand and the result stay declared,
            // redensified past the removed victim.
            assert_eq!(function.virtual_registers.len(), 2);
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1);
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0));
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys.copy_i64);
            // The rebuilt operand list binds the surviving `Use` and the
            // result `Def` — redensified to `VirtualRegisterId(1)` — and the
            // flag clobber the consumer row carried dies with the folded
            // form.
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            assert!(
                rewritten.clobbers.is_empty(),
                "the {consumer_clobbers} clobber(s) the consumer row carried drop with the form"
            );
            // The folded literal's provenance joins the consumer's; the
            // fieldless and kind carries no obligation custody.
            assert_eq!(rewritten.provenance.operations.len(), 2);
            assert!(rewritten.provenance.obligations.is_empty());

            let SelectedTerminator::ConditionalBranch { instruction, .. } =
                &function.blocks[0].terminator
            else {
                panic!("conditional branch terminator retained");
            };
            assert_eq!(instruction.id, SelectedInstructionId(1));
            let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator
            else {
                panic!("return terminator retained");
            };
            assert_eq!(instruction.id, SelectedInstructionId(2));
        }
    }
}

#[test]
fn and_ones_fold_rejects_a_literal_that_is_not_all_ones() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the identity element only when it is all ones:
        // `x & 1` is a different computation, and `x & 0` is the sibling
        // family's annihilator — both the producer's declared bound and
        // the replay's re-derived grammar reject either at either `Use`
        // position, keeping the two `BitwiseAndI64` families disjoint on
        // the literal's value.
        for wrong_literal in [1u64, 0] {
            for literal_operand in [0u16, 1u16] {
                let mut inputs = staged_and_ones_inputs(target, literal_operand);
                restage_literal(&mut inputs, wrong_literal);
                assert_eq!(
                    fold_with(
                        &inputs,
                        &environment,
                        LiteralFoldPolicy::BITWISE_AND_ONES_V1
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                    "literal {wrong_literal:#x} at operand {literal_operand} on {target:?}"
                );
                assert_eq!(
                    validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                    Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                    "literal {wrong_literal:#x} at operand {literal_operand} on {target:?} replay"
                );
            }
        }
    }
}

#[test]
fn and_ones_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; claiming the
        // other `Use` position in the recovery classification selects the
        // disjoint grammar, whose operand check finds the surviving register
        // where the victim must sit. Claiming the operand-2 `Def` position
        // names no grammar's victim position at all.
        for (literal_operand, claimed_operand, expected) in [
            (
                0u16,
                1u16,
                LiteralFoldError::ConsumerMismatch { function: 0 },
            ),
            (
                1u16,
                0u16,
                LiteralFoldError::ConsumerMismatch { function: 0 },
            ),
            (
                0u16,
                2u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
            (
                1u16,
                2u16,
                LiteralFoldError::FutureUseMismatch { function: 0 },
            ),
        ] {
            let mut inputs = staged_and_ones_inputs(target, literal_operand);
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
                    LiteralFoldPolicy::BITWISE_AND_ONES_V1
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
fn and_ones_fold_rejects_malformed_operand_arrangements() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `literal_operand` selects the grammar: operand positions swap roles
    // between the right and left identity shapes, so each mutation names
    // the position the grammar under test reads. The copy grammar admits
    // exactly the two `Use` operands and the `Def` result — unlike the
    // constant-result grammars it binds the surviving operand, so no
    // operand past the result has a droppable role.
    for literal_operand in [0u16, 1u16] {
        let victim_position = usize::from(literal_operand);
        let surviving_position = usize::from(1 - literal_operand);
        for mutation in 0..4 {
            let mut inputs = staged_and_ones_inputs(target, literal_operand);
            let mut plan = inputs.selected.transformed().clone();
            let consumer = &mut plan.functions[0].blocks[0].instructions[1];
            match mutation {
                // The folded operand is a `Def`, not a `Use`.
                0 => consumer.operands[victim_position].access = RegisterOperandAccess::Def,
                // The surviving position is not a `Use`.
                1 => consumer.operands[surviving_position].access = RegisterOperandAccess::Def,
                // The result `Def` is missing from the grammar.
                2 => {
                    consumer.operands.pop();
                }
                // An operand past the result is no droppable scratch under
                // the copy grammar — the operand list must be exactly the
                // two `Use`s and the `Def`.
                _ => {
                    let mut extra = consumer.operands[surviving_position];
                    extra.operand = 3;
                    extra.virtual_register = VirtualRegisterId(0);
                    consumer.operands.push(extra);
                }
            }
            let mut selected = inputs.selected.clone();
            selected.transformed = Arc::new(plan);
            inputs.selected = selected;
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::BITWISE_AND_ONES_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} mutation {mutation}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} mutation {mutation} replay"
            );
        }
    }
}

#[test]
fn and_ones_fold_rejects_a_scratch_def_the_copy_grammar_cannot_drop() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    assert_eq!(
        environment
            .constraint(keys.subtract_i64)
            .unwrap()
            .operands
            .len(),
        3,
        "the x86-64 and row carries no scratch def — the test appends one"
    );

    // Unlike the constant-result grammars, which drop a dead scratch `Def`
    // under occurrence-free custody, the copy grammar binds the surviving
    // operand and admits no fourth operand at all: a scratch `Def` past the
    // result — however dead — is a malformed arrangement for this family.
    for literal_operand in [0u16, 1u16] {
        let mut inputs = staged_and_ones_inputs(target, literal_operand);
        let mut plan = inputs.selected.transformed().clone();
        let function = &mut plan.functions[0];
        let scalar = function.virtual_registers[0].scalar_type;
        let class = function.virtual_registers[0].class;
        function
            .virtual_registers
            .push(selected_instructions::VirtualRegister {
                id: VirtualRegisterId(3),
                scalar_type: scalar,
                class,
                origin: selected_instructions::VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(1),
                    operand: 3,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
        function.blocks[0].instructions[1]
            .operands
            .push(SelectedOperand {
                operand: 3,
                virtual_register: VirtualRegisterId(3),
                access: RegisterOperandAccess::Def,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            });
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        inputs.selected = selected;

        assert_eq!(
            fold_selected_incoming_literal(
                &inputs.selected,
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
                LiteralFoldPolicy::BITWISE_AND_ONES_V1,
                budget(),
            )
            .map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "literal at operand {literal_operand}"
        );
        assert_eq!(
            validate_literal_fold(
                &inputs.selected,
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
            .map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "literal at operand {literal_operand} replay"
        );
    }
}

#[test]
fn and_ones_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The and row pins nothing: under the declared `Isolated` unit-effect
    // surface no operand may carry a binding — unlike the divide and
    // remainder folds, which deliberately drop the pins their pinned
    // realizations require. A `fixed_view`, a `tied_to`, or an
    // `early_clobber` on any operand rejects under either grammar.
    for literal_operand in [0u16, 1u16] {
        let surviving_position = usize::from(1 - literal_operand);
        for mutation in 0..3 {
            let mut inputs = staged_and_ones_inputs(target, literal_operand);
            let mut plan = inputs.selected.transformed().clone();
            let operand =
                &mut plan.functions[0].blocks[0].instructions[1].operands[surviving_position];
            match mutation {
                0 => operand.fixed_view = Some(register_model::RegisterViewId(0)),
                1 => operand.tied_to = Some(0),
                _ => operand.early_clobber = true,
            }
            let mut selected = inputs.selected.clone();
            selected.transformed = Arc::new(plan);
            inputs.selected = selected;

            assert_eq!(
                fold_selected_incoming_literal(
                    &inputs.selected,
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
                    LiteralFoldPolicy::BITWISE_AND_ONES_V1,
                    budget(),
                ),
                Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                "literal at operand {literal_operand} mutation {mutation}"
            );
            assert_eq!(
                validate_literal_fold(
                    &inputs.selected,
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
                "literal at operand {literal_operand} mutation {mutation} replay"
            );
        }
    }
}

#[test]
fn and_ones_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A `BitwiseAndI64` consumer with the all-ones literal is admitted only
    // under the and-ones policy. A selection enabling a different consumer
    // kind's family reports the unadmitted kind as a consumer mismatch.
    let inputs = staged_and_ones_inputs(target, 1);
    for policy in [
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
        LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // The sibling and family admits the same kind and operand positions
    // under a disjoint literal bound: with only the and-zero bit set the
    // consumer kind and the `Use` position are admitted and the all-ones
    // literal alone is what rejects — an unsupported immediate, not a
    // consumer or position mismatch. The strongest disabled posture —
    // every other family enabled at once — reports the same boundary.
    for policy in [
        LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
        policy_without(LiteralFoldPolicy::BITWISE_AND_ONES_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{policy:?}"
        );
    }
    // The disjointness is symmetric: the and-zero fixture's zero literal
    // is the admitted kind and position under the and-ones policy, but
    // the literal lies outside the all-ones bound.
    let and_zero = staged_and_inputs(target, 1);
    assert_eq!(
        fold_with(
            &and_zero,
            &environment,
            LiteralFoldPolicy::BITWISE_AND_ONES_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::UnsupportedImmediate { function: 0 })
    );
    // And the and-ones policy admits no other consumer kind: the compare
    // fixture's flag-defining consumer has no `Def` operand 2 the
    // copy-result grammar could bind, and the xor fixture's consumer kind
    // is no admitted kind at all.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(
            &compare,
            &environment,
            LiteralFoldPolicy::BITWISE_AND_ONES_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    let xor = staged_xor_inputs(target, 1);
    assert_eq!(
        fold_with(&xor, &environment, LiteralFoldPolicy::BITWISE_AND_ONES_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn and_ones_and_and_zero_families_coexist_under_the_literal_value_key() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    // With both `BitwiseAndI64` families enabled, the literal's value alone
    // selects the grammar at either `Use` position: zero takes the
    // annihilator fold to a materialized zero, all ones takes the identity
    // fold to a copy of the surviving operand. Neither catalog order nor a
    // first-match rule may arbitrate the overlap — the partition must be
    // on the admitted literal.
    let both = LiteralFoldPolicy::BITWISE_AND_ZERO_V1.union(LiteralFoldPolicy::BITWISE_AND_ONES_V1);
    for literal_operand in [0u16, 1u16] {
        let ones = staged_and_ones_inputs(target, literal_operand);
        let ones_result = fold_with(&ones, &environment, both)
            .expect("the and-ones fold applies under both families enabled");
        assert_eq!(ones_result.receipt().applied_count(), 1);
        let action = ones_result.plan().functions[0].action.unwrap();
        assert_eq!(action.immediate, u64::MAX);
        assert_eq!(action.immediate_constraint, keys.copy_i64);
        let rewritten = &ones_result.transformed().functions[0].blocks[0].instructions[0];
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.constraint, keys.copy_i64);

        let zero = staged_and_inputs(target, literal_operand);
        let zero_result = fold_with(&zero, &environment, both)
            .expect("the and-zero fold applies under both families enabled");
        assert_eq!(zero_result.receipt().applied_count(), 1);
        let action = zero_result.plan().functions[0].action.unwrap();
        assert_eq!(action.immediate, 0);
        assert_eq!(action.immediate_constraint, keys.materialize_i64);
        let rewritten = &zero_result.transformed().functions[0].blocks[0].instructions[0];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(0)
            }
        );
        assert_eq!(rewritten.constraint, keys.materialize_i64);
    }
}

#[test]
fn and_ones_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The left-literal grammar exercises the operand-0 fold.
    let inputs = staged_and_ones_inputs(target, 0);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::BITWISE_AND_ONES_V1,
    )
    .expect("the staged and-ones fold should validate");

    for mutation in 0..12 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the and's own `Def`, not the
            // surviving `Use`.
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            1 => plan.functions[0].action.as_mut().unwrap().result = None,
            // The recorded immediate is the folded literal all-ones; any
            // substitution replays differently — and any lesser literal
            // falls outside the family's exact bound.
            2 => plan.functions[0].action.as_mut().unwrap().immediate -= 1,
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            // The recorded constraint is the `CopyI64` row the and-ones
            // policy gate binds; any other key fails the rebuild's binding.
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
            // A policy without the and-ones bit cannot replay the fold:
            // no `CopyI64` row binds for this consumer and the action
            // reconstructs nothing.
            8 => plan.policy = LiteralFoldPolicy::EXACT_ADD_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
            // The sibling family's policy is no substitute either: the
            // and-zero gate binds no `CopyI64` row for this consumer, so
            // the all-ones literal replays as an unsupported immediate
            // rather than silently landing under the other family's row.
            10 => plan.policy = LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
            // The surviving register is the non-victim `Use` the rewritten
            // copy binds: recording the result `Def` register instead fails
            // the re-derived action.
            _ => plan.functions[0].action.as_mut().unwrap().surviving = VirtualRegisterId(2),
        }
        assert!(
            validate(&inputs, &environment, plan).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn and_ones_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_and_ones_inputs(target, literal_operand);
            assert_budget_is_enforced(
                &inputs,
                &environment,
                LiteralFoldPolicy::BITWISE_AND_ONES_V1,
            );
        }
    }
}

#[test]
fn and_ones_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_and_ones_inputs(target, literal_operand);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::BITWISE_AND_ONES_V1,
            );
        }
    }
}
