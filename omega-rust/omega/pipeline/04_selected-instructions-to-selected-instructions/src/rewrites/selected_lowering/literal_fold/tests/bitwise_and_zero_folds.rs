use super::{budget, fold_with, restage_literal, staged_and_inputs, validate};
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::{fold_selected_incoming_literal, validate_literal_fold};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedInstructionId, SelectedOperand, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn and_zero_fold_rejects_a_nonzero_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The literal is the annihilator only when it is exactly zero: `x &
        // 1` is a different computation both the producer's declared bound
        // and the replay's re-derived grammar reject, at either `Use`
        // position.
        for literal_operand in [0u16, 1u16] {
            let mut inputs = staged_and_inputs(target, literal_operand);
            restage_literal(&mut inputs, 1);
            assert_eq!(
                fold_with(
                    &inputs,
                    &environment,
                    LiteralFoldPolicy::BITWISE_AND_ZERO_V1
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
fn and_zero_fold_drops_a_dead_scratch_def_and_rejects_one_without_dead_custody() {
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

    // A `Def` operand past the result is droppable only when its register
    // occurs nowhere else in the function. The real row has no scratch tail,
    // so the test stages one: a `Def` operand at position 3 binding a
    // register the consumer alone defines. A `Use` in the scratch position,
    // a scratch register another operand also reads, and a scratch register
    // another operand also defines each reject — the producer and the
    // independent replay alike.
    for literal_operand in [0u16, 1u16] {
        let dropped_use_position = usize::from(1 - literal_operand);
        for mutation in 0..4 {
            let mut inputs = staged_and_inputs(target, literal_operand);
            let mut plan = inputs.selected.transformed().clone();
            let function = &mut plan.functions[0];
            let scalar = function.virtual_registers[0].scalar_type;
            let class = function.virtual_registers[0].class;
            function.virtual_registers.push(VirtualRegister {
                id: VirtualRegisterId(3),
                scalar_type: scalar,
                class,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(1),
                    operand: 3,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            let consumer = &mut function.blocks[0].instructions[1];
            consumer.operands.push(SelectedOperand {
                operand: 3,
                virtual_register: VirtualRegisterId(3),
                access: RegisterOperandAccess::Def,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            });
            match mutation {
                // The clean dead scratch: the fold admits the extra `Def`
                // and drops it with the folded operand list.
                0 => {}
                // The operand past the result reads a register rather than
                // writing a dead scratch — not a `Def` the grammar may drop.
                1 => {
                    function.blocks[0].instructions[1].operands[3].access =
                        RegisterOperandAccess::Use;
                }
                // The scratch operand binds the dropped `Use`'s register,
                // which that `Use` also reads — the dropped `Def` would
                // strand a definition its sibling operand still observes.
                2 => {
                    function.blocks[0].instructions[1].operands[3].virtual_register =
                        VirtualRegisterId(0);
                }
                // The dropped `Use` reads the scratch register, which the
                // operand-3 `Def` also writes — the dropped `Def` is not
                // dead.
                _ => {
                    function.blocks[0].instructions[1].operands[dropped_use_position]
                        .virtual_register = VirtualRegisterId(3);
                }
            }
            let mut selected = inputs.selected.clone();
            selected.transformed = Arc::new(plan);
            inputs.selected = selected;

            let produced = fold_selected_incoming_literal(
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
                LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
                budget(),
            );
            if mutation == 0 {
                // The clean dead scratch: the fold admits the extra `Def`
                // and drops it with the folded operand list, and the
                // produced plan replays to the same transformed output.
                let result = produced.unwrap_or_else(|error| {
                    panic!(
                        "a dead scratch at operand {literal_operand}'s grammar drops cleanly: \
                         {error:?}"
                    )
                });
                assert_eq!(result.receipt().applied_count(), 1);
                let replayed = validate_literal_fold(
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
                    result.plan().clone(),
                )
                .expect("the produced plan replays");
                assert_eq!(replayed.receipt().applied_count(), 1);
                assert_eq!(replayed.transformed(), result.transformed());
            } else {
                assert_eq!(
                    produced.map(|_| ()),
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
                    )
                    .map(|_| ()),
                    Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
                    "literal at operand {literal_operand} mutation {mutation} replay"
                );
            }
        }
    }
}
