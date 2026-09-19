//! Operand forms that read and write the victim in one instruction — the
//! read-modify-write family. A `Def` tied to a victim use is the two-operand
//! form: the use reloads the pre-write value like any other, the tie binds
//! the write to that reload register's home, and the redefinition's store
//! after the instruction reads the victim operand the write kept. A `UseDef`
//! operand is the one-operand form: the operand moves to a reload register,
//! the instruction writes its result there, and the store after the
//! instruction reads the emitted operand's register rather than the victim.
//! Either way the write closes the still-open shared reload exactly like a
//! plain `Def` redefinition, so the next use reads the new value through a
//! fresh pair — under either span policy, including across a unit writer the
//! crossing policy had kept open.

use super::{
    Arc, MachineId, NativeTarget, SelectedInstructionId, SelectedInstructionKind,
    VirtualRegisterId, baseline_target_register_environment, fixture,
    selected_instruction_plan_identity,
};
use crate::RuntimeSpillError;
use crate::ValidatedRuntimeSpill;
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::budget;
use crate::spill_selected_runtime_value;
use crate::validate_runtime_spill;
use register_model::{RegisterClassId, RegisterOperandAccess};
use selected_instructions::SelectedOperand;

/// The base fixture's copy chain — register 1 defined by instruction 1 and
/// used by instructions 2, 3, and 4 — gains a read-modify-write between the
/// uses at 3 and 4. `usedef` selects the form: instruction 6 either holds the
/// two-operand tied pair (`Use` of register 1 plus a `Def` of register 1 tied
/// back to it) or a single `UseDef` operand on register 1. When `call` is set
/// a zero-argument `CallUnit` sits between the uses at 2 and 3 so the
/// crossing policy has a unit writer to keep open across.
fn read_modify_write_fixture(
    target: NativeTarget,
    usedef: bool,
    call: bool,
) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    if call {
        let call_row = environment
            .constraint(
                *keys
                    .call_unit
                    .first()
                    .expect("every baseline target has a zero-argument unit call row"),
            )
            .unwrap();
        function.blocks[0].instructions.insert(
            2,
            admission::instruction(
                SelectedInstructionId(7),
                SelectedInstructionKind::CallUnit {
                    callee: MachineId::new(2).unwrap(),
                },
                call_row,
                &[],
            ),
        );
    }
    let mut rewriting = admission::instruction(
        SelectedInstructionId(6),
        SelectedInstructionKind::CopyI64,
        copy,
        &[VirtualRegisterId(1), VirtualRegisterId(1)],
    );
    if usedef {
        rewriting.operands.truncate(1);
        rewriting.operands[0].access = RegisterOperandAccess::UseDef;
    } else {
        rewriting.operands[1].tied_to = Some(0);
    }
    function.blocks[0]
        .instructions
        .insert(3 + usize::from(call), rewriting);
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn operand(
    number: u16,
    register: VirtualRegisterId,
    access: RegisterOperandAccess,
    class: RegisterClassId,
) -> SelectedOperand {
    SelectedOperand {
        operand: number,
        virtual_register: register,
        access,
        class,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    }
}

fn reload_named_by(block: &selected_instructions::SelectedBlock, id: u32) -> VirtualRegisterId {
    block
        .instructions
        .iter()
        .find(|instruction| instruction.id == SelectedInstructionId(id))
        .unwrap()
        .operands[0]
        .virtual_register
}

#[test]
fn read_modify_writes_store_the_post_write_value_and_close_the_span() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for usedef in [false, true] {
            for call in [false, true] {
                let source = read_modify_write_fixture(target, usedef, call);
                let bounded = spill_selected_runtime_value(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                )
                .unwrap();
                let crossing = crate::spill_selected_runtime_value_with_span_policy(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
                )
                .unwrap();
                let bounded_block = &bounded.transformed().functions[0].blocks[0];
                let crossing_block = &crossing.transformed().functions[0].blocks[0];
                // One store after each write — the origin instruction 1 and
                // the rewriting instruction 6 — under either policy.
                for block in [bounded_block, crossing_block] {
                    let stores: Vec<usize> = block
                        .instructions
                        .iter()
                        .enumerate()
                        .filter_map(|(position, instruction)| {
                            matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
                                .then_some(position)
                        })
                        .collect();
                    assert_eq!(stores.len(), 2, "{target:?} usedef={usedef} call={call}");
                    let rewritten = block
                        .instructions
                        .iter()
                        .position(|instruction| instruction.id == SelectedInstructionId(6))
                        .unwrap();
                    assert_eq!(stores[1], rewritten + 1);
                    let store = &block.instructions[rewritten + 1];
                    let rewriting = &block.instructions[rewritten];
                    if usedef {
                        // The `UseDef` operand moved to its reload register
                        // and the store reads that register — the instruction
                        // never writes the victim itself.
                        let reloaded = rewriting.operands[0].virtual_register;
                        assert_ne!(reloaded, VirtualRegisterId(1));
                        assert_eq!(store.operands[0].virtual_register, reloaded);
                        assert_eq!(rewriting.operands[0].access, RegisterOperandAccess::UseDef);
                    } else {
                        // The tied `Def` keeps the victim register and its
                        // tie; the store reads that operand's register, which
                        // the tie binds to the reloaded use's home.
                        assert_eq!(rewriting.operands[1].virtual_register, VirtualRegisterId(1));
                        assert_eq!(rewriting.operands[1].tied_to, Some(0));
                        assert_eq!(store.operands[0].virtual_register, VirtualRegisterId(1));
                        assert_ne!(rewriting.operands[0].virtual_register, VirtualRegisterId(1));
                    }
                }
                let count_loads = |block: &selected_instructions::SelectedBlock| {
                    block
                        .instructions
                        .iter()
                        .filter(|instruction| {
                            matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                        })
                        .count()
                };
                // The rewrite closes the still-open span: the use at 4 reads
                // the new value through a fresh pair even under the crossing
                // policy, which crossed the call between the uses at 2 and 3.
                // The rewriting instruction's own read side shares the open
                // pair, so it names the same register as the use at 2 — or at
                // 3 once the bounded policy's pair closed at the call.
                assert_eq!(
                    count_loads(crossing_block),
                    2,
                    "{target:?} usedef={usedef} call={call}"
                );
                let early = reload_named_by(crossing_block, 2);
                assert_eq!(reload_named_by(crossing_block, 3), early);
                assert_eq!(reload_named_by(crossing_block, 6), early);
                let late = reload_named_by(crossing_block, 4);
                assert_ne!(late, early);
                if call {
                    assert_eq!(
                        count_loads(bounded_block),
                        3,
                        "{target:?} usedef={usedef} call={call}"
                    );
                    let first = reload_named_by(bounded_block, 2);
                    let second = reload_named_by(bounded_block, 3);
                    let third = reload_named_by(bounded_block, 4);
                    assert_eq!(reload_named_by(bounded_block, 6), second);
                    assert_ne!(first, second);
                    assert_ne!(second, third);
                    assert_ne!(first, third);
                } else {
                    assert_eq!(bounded_block.instructions, crossing_block.instructions);
                }
                assert!(
                    validate_runtime_spill(
                        &source,
                        0,
                        VirtualRegisterId(1),
                        &environment,
                        budget(),
                        bounded.transformed().clone()
                    )
                    .is_ok(),
                    "{target:?} usedef={usedef} call={call} bounded replay"
                );
                assert!(
                    crate::validate_runtime_spill_with_span_policy(
                        &source,
                        0,
                        VirtualRegisterId(1),
                        &environment,
                        budget(),
                        crossing.transformed().clone(),
                        crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
                    )
                    .is_ok(),
                    "{target:?} usedef={usedef} call={call} crossing replay"
                );
                for (mutation, proposed) in [
                    // Dropping the rewrite's own store leaves the post-write
                    // value untracked.
                    {
                        let mut proposed = crossing.transformed().clone();
                        let block = &mut proposed.functions[0].blocks[0];
                        let rewritten = block
                            .instructions
                            .iter()
                            .position(|instruction| instruction.id == SelectedInstructionId(6))
                            .unwrap();
                        block.instructions.remove(rewritten + 1);
                        proposed
                    },
                    // A use after the rewrite naming the stale pre-write
                    // reload restores nothing.
                    {
                        let mut proposed = crossing.transformed().clone();
                        let early = reload_named_by(&proposed.functions[0].blocks[0], 2);
                        proposed.functions[0].blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|instruction| instruction.id == SelectedInstructionId(4))
                            .unwrap()
                            .operands[0]
                            .virtual_register = early;
                        proposed
                    },
                    // The post-write store must read the operand the write
                    // landed in: the kept victim operand for a tied `Def`,
                    // the redirected operand for a `UseDef`. Reading the
                    // other one mismatches the replay's expectation.
                    {
                        let mut proposed = crossing.transformed().clone();
                        let block = &mut proposed.functions[0].blocks[0];
                        let rewritten = block
                            .instructions
                            .iter()
                            .position(|instruction| instruction.id == SelectedInstructionId(6))
                            .unwrap();
                        let other = if usedef {
                            VirtualRegisterId(1)
                        } else {
                            block.instructions[rewritten].operands[0].virtual_register
                        };
                        block.instructions[rewritten + 1].operands[0].virtual_register = other;
                        proposed
                    },
                ]
                .into_iter()
                .enumerate()
                {
                    assert_eq!(
                        crate::validate_runtime_spill_with_span_policy(
                            &source,
                            0,
                            VirtualRegisterId(1),
                            &environment,
                            budget(),
                            proposed,
                            crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
                        )
                        .unwrap_err(),
                        RuntimeSpillError::ReplayMismatch,
                        "{target:?} usedef={usedef} call={call} mutation {mutation}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_parameter_victims_read_modify_write_stores_after_the_instruction() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for usedef in [false, true] {
            let mut source = super::parameters::parameter_fixture(target);
            {
                let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
                // The destination block holds uses 401 and 402; instruction
                // 403 reads and rewrites the parameter register between them.
                let copy = environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap();
                let mut rewriting = admission::instruction(
                    SelectedInstructionId(403),
                    SelectedInstructionKind::CopyI64,
                    copy,
                    &[VirtualRegisterId(1), VirtualRegisterId(1)],
                );
                if usedef {
                    rewriting.operands.truncate(1);
                    rewriting.operands[0].access = RegisterOperandAccess::UseDef;
                } else {
                    rewriting.operands[1].tied_to = Some(0);
                }
                function.blocks[2].instructions.insert(1, rewriting);
            }
            let identity = selected_instruction_plan_identity(source.transformed());
            source.receipt.source_selected = identity;
            source.receipt.transformed_selected = identity;
            let result = spill_selected_runtime_value(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
            )
            .unwrap();
            let function = &result.transformed().functions[0];
            // Both edge arrivals still store their bound argument, and the
            // destination block gains the rewrite's store after 403.
            let block = &function.blocks[2];
            let rewritten = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == SelectedInstructionId(403))
                .unwrap();
            assert!(matches!(
                block.instructions[rewritten + 1].kind,
                SelectedInstructionKind::Store64 { .. }
            ));
            let store = &block.instructions[rewritten + 1];
            if usedef {
                assert_eq!(
                    store.operands[0].virtual_register,
                    block.instructions[rewritten].operands[0].virtual_register
                );
                assert_ne!(store.operands[0].virtual_register, VirtualRegisterId(1));
            } else {
                assert_eq!(store.operands[0].virtual_register, VirtualRegisterId(1));
                assert_eq!(block.instructions[rewritten].operands[1].tied_to, Some(0));
            }
            let loads = block
                .instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                })
                .count();
            // One reload before the use at 401, a fresh one before the use at
            // 402: the rewrite closed the span between them.
            assert_eq!(loads, 2, "{target:?} usedef={usedef}");
            assert!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    result.transformed().clone()
                )
                .is_ok()
            );
        }
    }
}

#[test]
fn read_modify_write_forms_still_rejected() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap();
    let victim_class = copy.operands[0].class;
    let rewriting = |function: &mut selected_instructions::SelectedFunction,
                     operands: Vec<SelectedOperand>| {
        let mut instruction = admission::instruction(
            SelectedInstructionId(6),
            SelectedInstructionKind::CopyI64,
            copy,
            &[],
        );
        instruction.operands = operands;
        function.blocks[0].instructions.insert(3, instruction);
    };
    for mutation in 0..10 {
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            match mutation {
                // A write tied to the victim's use must be the victim's own
                // redefinition — a tie onto another register's write clobbers
                // the reload with no store to mirror it.
                0 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::Use,
                            victim_class,
                        ),
                        {
                            let mut def = operand(
                                1,
                                VirtualRegisterId(2),
                                RegisterOperandAccess::Def,
                                victim_class,
                            );
                            def.tied_to = Some(0);
                            def
                        },
                    ],
                ),
                // An early-clobber write tied to the use could land before a
                // co-operand's read of the same reload register.
                1 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::Use,
                            victim_class,
                        ),
                        {
                            let mut def = operand(
                                1,
                                VirtualRegisterId(1),
                                RegisterOperandAccess::Def,
                                victim_class,
                            );
                            def.tied_to = Some(0);
                            def.early_clobber = true;
                            def
                        },
                    ],
                ),
                // Two writes tied to one use never name a single home.
                2 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::Use,
                            victim_class,
                        ),
                        {
                            let mut def = operand(
                                1,
                                VirtualRegisterId(1),
                                RegisterOperandAccess::Def,
                                victim_class,
                            );
                            def.tied_to = Some(0);
                            def
                        },
                        {
                            let mut def = operand(
                                2,
                                VirtualRegisterId(1),
                                RegisterOperandAccess::Def,
                                victim_class,
                            );
                            def.tied_to = Some(0);
                            def
                        },
                    ],
                ),
                // A use carrying its own tie reads a writer's home rather than
                // the reload's — the tie points the wrong way for this
                // rewrite.
                3 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(2),
                            RegisterOperandAccess::Def,
                            victim_class,
                        ),
                        {
                            let mut usage = operand(
                                1,
                                VirtualRegisterId(1),
                                RegisterOperandAccess::Use,
                                victim_class,
                            );
                            usage.tied_to = Some(0);
                            usage
                        },
                    ],
                ),
                // An early-clobber `UseDef` could write before a co-operand's
                // read of the shared reload register.
                4 => rewriting(
                    function,
                    vec![{
                        let mut usedef = operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::UseDef,
                            victim_class,
                        );
                        usedef.early_clobber = true;
                        usedef
                    }],
                ),
                // A `UseDef` carrying its own tie would bind a second register
                // to the reload's home.
                5 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(2),
                            RegisterOperandAccess::Use,
                            victim_class,
                        ),
                        {
                            let mut usedef = operand(
                                1,
                                VirtualRegisterId(1),
                                RegisterOperandAccess::UseDef,
                                victim_class,
                            );
                            usedef.tied_to = Some(0);
                            usedef
                        },
                    ],
                ),
                // An operand tied to the `UseDef` writes into the reload's
                // home with no store to mirror it.
                6 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::UseDef,
                            victim_class,
                        ),
                        {
                            let mut def = operand(
                                1,
                                VirtualRegisterId(2),
                                RegisterOperandAccess::Def,
                                victim_class,
                            );
                            def.tied_to = Some(0);
                            def
                        },
                    ],
                ),
                // A second victim-writing operand after the `UseDef` leaves
                // the slot without a defined last writer.
                7 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::UseDef,
                            victim_class,
                        ),
                        operand(
                            1,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::Def,
                            victim_class,
                        ),
                    ],
                ),
                // The same two writers in the other order.
                8 => rewriting(
                    function,
                    vec![
                        operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::Def,
                            victim_class,
                        ),
                        operand(
                            1,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::UseDef,
                            victim_class,
                        ),
                    ],
                ),
                // A `UseDef` in a foreign class cannot ride the victim's rows.
                _ => rewriting(
                    function,
                    vec![{
                        let float_class = environment
                            .constraint(environment.selected_keys().bits_to_float64.unwrap())
                            .unwrap()
                            .operands[1]
                            .class;
                        operand(
                            0,
                            VirtualRegisterId(1),
                            RegisterOperandAccess::UseDef,
                            float_class,
                        )
                    }],
                ),
            }
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedUse,
            "mutation {mutation}"
        );
    }
    // A `UseDef` operand inside the origin instruction is not a later
    // definition at all, and a terminator `UseDef` has no following store
    // point.
    for mutation in 0..2 {
        let mut source = fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            match mutation {
                0 => function.blocks[0].instructions[0].operands.push(operand(
                    2,
                    VirtualRegisterId(1),
                    RegisterOperandAccess::UseDef,
                    victim_class,
                )),
                _ => super::super::control_mut(&mut function.blocks[0].terminator)
                    .operands
                    .push(operand(
                        0,
                        VirtualRegisterId(1),
                        RegisterOperandAccess::UseDef,
                        victim_class,
                    )),
            }
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap_err(),
            RuntimeSpillError::UnsupportedUse,
            "mutation {mutation}"
        );
    }
}
