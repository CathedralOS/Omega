use super::{
    assert_budget_is_enforced, assert_deterministic_fixed_point, budget, fold_with, policy_without,
    restage_literal, staged_inputs, staged_wrapping_add_inputs, staged_xor_inputs, validate,
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
fn wrapping_add_zero_fold_rewrites_the_consumer_to_a_surviving_operand_copy_on_both_linux_targets()
{
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        // The wrapping-add form binds the flag-transparent add constraint
        // row: the selected wrapping add is realized flag-preserving on
        // both targets — x86-64 `lea` and aarch64 `add` touch no condition
        // state — so unlike the bitwise forms the fold retires no flag
        // clobber at all.
        assert!(
            environment
                .constraint(keys.add_i64)
                .unwrap()
                .clobbers
                .is_empty(),
            "the {target:?} add row clobbers no unit — no flag clobber to retire"
        );
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_wrapping_add_inputs(target, literal_operand);
            let result = fold_with(
                &inputs,
                &environment,
                LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "wrapping-add-zero fold with the literal at operand {literal_operand} on \
                     {target:?} should validate: {error:?}"
                )
            });

            assert_eq!(
                result.plan().machine_effect_catalog,
                effect_catalog.identity()
            );
            assert_eq!(result.receipt().applied_count(), 1);
            let action = result.plan().functions[0].action.unwrap();
            assert_eq!(action.result, Some(VirtualRegisterId(2)));
            // The recorded immediate is the folded literal itself — zero —
            // under either operand grammar; the `CopyI64` rewrite binds the
            // surviving register rather than embedding a constant.
            assert_eq!(action.immediate, 0);
            // The surviving record is the non-victim `Use` register,
            // whichever operand position the folded literal occupied.
            assert_eq!(action.surviving, VirtualRegisterId(0));
            assert_eq!(action.victim, VirtualRegisterId(1));
            assert_eq!(action.literal_instruction, SelectedInstructionId(0));
            assert_eq!(action.consumer_instruction, SelectedInstructionId(1));
            assert_eq!(action.immediate_constraint, keys.copy_i64);

            let function = &result.transformed().functions[0];
            // The fold removes only the zero literal and its register: the
            // surviving operand and the result stay declared, redensified
            // past the removed victim.
            assert_eq!(function.virtual_registers.len(), 2);
            let instructions = &function.blocks[0].instructions;
            assert_eq!(instructions.len(), 1);
            let rewritten = &instructions[0];
            assert_eq!(rewritten.id, SelectedInstructionId(0));
            assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rewritten.constraint, keys.copy_i64);
            // The rebuilt operand list binds the surviving `Use` and the
            // result `Def` — redensified to `VirtualRegisterId(1)` — and
            // the flag-transparent add row carries no clobber the fold
            // could retire.
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            assert!(rewritten.clobbers.is_empty());
            // The folded literal's provenance joins the consumer's; the
            // fieldless wrapping-add kind carries no obligation custody.
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
fn wrapping_add_zero_fold_rejects_a_nonzero_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the identity element only when it is exactly zero:
        // `x + 1` is a different computation both the producer's declared
        // bound and the replay's re-derived grammar reject, at either `Use`
        // position.
        for literal_operand in [0u16, 1u16] {
            let mut inputs = staged_wrapping_add_inputs(target, literal_operand);
            restage_literal(&mut inputs, 1);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1
                )
                .map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "literal at operand {literal_operand} on {target:?}"
            );
            assert_eq!(
                validate(&inputs, &environment, inputs.selected.plan().clone()).map(|_| ()),
                Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
                "literal at operand {literal_operand} on {target:?} replay"
            );
        }
    }
}

#[test]
fn wrapping_add_zero_fold_rejects_a_literal_claiming_the_wrong_operand_position() {
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
            let mut inputs = staged_wrapping_add_inputs(target, literal_operand);
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
                    LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1
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
fn wrapping_add_zero_fold_rejects_malformed_operand_arrangements() {
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
            let mut inputs = staged_wrapping_add_inputs(target, literal_operand);
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
                    LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1
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
fn wrapping_add_zero_fold_rejects_a_scratch_def_the_copy_grammar_cannot_drop() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    assert_eq!(
        environment.constraint(keys.add_i64).unwrap().operands.len(),
        3,
        "the x86-64 wrapping-add row carries no scratch def — the test appends one"
    );

    // Unlike the constant-result grammars, which drop a dead scratch `Def`
    // under occurrence-free custody, the copy grammar binds the surviving
    // operand and admits no fourth operand at all: a scratch `Def` past the
    // result — however dead — is a malformed arrangement for this family.
    for literal_operand in [0u16, 1u16] {
        let mut inputs = staged_wrapping_add_inputs(target, literal_operand);
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
                LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
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
fn wrapping_add_zero_fold_rejects_consumer_operands_carrying_forbidden_bindings() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();

    // The add row pins nothing: under the declared `Isolated` unit-effect
    // surface no operand may carry a binding — unlike the divide and
    // remainder folds, which deliberately drop the pins their pinned
    // realizations require. A `fixed_view`, a `tied_to`, or an
    // `early_clobber` on any operand rejects under either grammar.
    for literal_operand in [0u16, 1u16] {
        let surviving_position = usize::from(1 - literal_operand);
        for mutation in 0..3 {
            let mut inputs = staged_wrapping_add_inputs(target, literal_operand);
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
                    LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
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
fn wrapping_add_zero_fold_rejects_consumers_the_selection_does_not_enable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The wrapping-add operand grammar admits the literal only under the
    // wrapping-add-zero policy: every other selected family sees no
    // admitted consumer kind — including the strongest posture, every other
    // rule enabled at once.
    let inputs = staged_wrapping_add_inputs(target, 1);
    for policy in [
        LiteralFoldPolicy::EXACT_ADD_V1,
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
        policy_without(LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1),
    ] {
        assert_eq!(
            fold_with(&inputs, &environment, policy).map(|_| ()),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "{policy:?}"
        );
    }
    // And the wrapping-add-zero policy admits no other consumer: the
    // compare fixture's flag-defining consumer has no `Def` operand 2 the
    // copy-result grammar could bind, and the xor fixture's consumer kind
    // is no admitted kind at all.
    let compare = staged_inputs(target);
    assert_eq!(
        fold_with(
            &compare,
            &environment,
            LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1
        )
        .map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
    let xor = staged_xor_inputs(target, 1);
    assert_eq!(
        fold_with(&xor, &environment, LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1).map(|_| ()),
        Err(LiteralFoldError::ConsumerMismatch { function: 0 })
    );
}

#[test]
fn wrapping_add_zero_fold_replay_rejects_every_decision_field_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The left-literal grammar exercises the operand-0 fold.
    let inputs = staged_wrapping_add_inputs(target, 0);
    let result = fold_with(
        &inputs,
        &environment,
        LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
    )
    .expect("the staged wrapping-add-zero fold should validate");

    for mutation in 0..11 {
        let mut plan = result.plan().clone();
        match mutation {
            // The recorded result register is the add's own `Def`, not the
            // surviving `Use`.
            0 => plan.functions[0].action.as_mut().unwrap().result = Some(VirtualRegisterId(0)),
            1 => plan.functions[0].action.as_mut().unwrap().result = None,
            // The recorded immediate is the folded literal zero; any
            // substitution replays differently.
            2 => plan.functions[0].action.as_mut().unwrap().immediate += 1,
            3 => {
                plan.functions[0]
                    .action
                    .as_mut()
                    .unwrap()
                    .consumer_instruction = SelectedInstructionId(9)
            }
            // The recorded constraint is the `CopyI64` row the
            // wrapping-add-zero policy gate binds; any other key fails the
            // rebuild's binding.
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
            // A policy without the wrapping-add-zero bit cannot replay the
            // fold: no `CopyI64` row binds for this consumer and the action
            // reconstructs nothing.
            8 => plan.policy = LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
            9 => plan.machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes([98; 32]),
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
fn wrapping_add_zero_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_wrapping_add_inputs(target, literal_operand);
            assert_budget_is_enforced(
                &inputs,
                &environment,
                LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
            );
        }
    }
}

#[test]
fn wrapping_add_zero_fold_is_deterministic_and_a_fixed_point_on_its_output() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for literal_operand in [0u16, 1u16] {
            let inputs = staged_wrapping_add_inputs(target, literal_operand);
            assert_deterministic_fixed_point(
                &inputs,
                &environment,
                LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
            );
        }
    }
}
