//! A victim a later instruction re-defines — a write-only `Def` operand on
//! the same register — keeps the slot tracking the register: one store after
//! every definition, a use on the redefining instruction still reads the
//! pre-write value through its own reload, and the redefinition ends any
//! still-open shared reload so the next use reads the new value through a
//! fresh pair. The origin definition still dominates every use, so on each
//! path the last executed store is the register's reaching write.

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
use register_model::RegisterOperandAccess;
use selected_instructions::SelectedOperand;

/// The base fixture's copy chain — register 1 defined by instruction 1 and
/// used by instructions 2, 3, and 4 — gains a redefinition: instruction 6
/// copies register 2 back into register 1 between the uses at 3 and 4, and
/// when `call` is set a zero-argument `CallUnit` sits between the uses at 2
/// and 3 so the crossing policy has a unit writer to keep open across.
fn redefining_fixture(target: NativeTarget, call: bool) -> ValidatedRuntimeSpill {
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
    function.blocks[0].instructions.insert(
        3 + usize::from(call),
        admission::instruction(
            SelectedInstructionId(6),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(2), VirtualRegisterId(1)],
        ),
    );
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

#[test]
fn redefinitions_store_each_write_and_uses_reload_the_last_one() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for call in [false, true] {
            let source = redefining_fixture(target, call);
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
            // One store after each definition — the origin instruction 1 and
            // the redefining instruction 6 — under either policy.
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
                assert_eq!(stores.len(), 2, "{target:?} call={call}");
                let redefined = block
                    .instructions
                    .iter()
                    .position(|instruction| instruction.id == SelectedInstructionId(6))
                    .unwrap();
                assert_eq!(stores[1], redefined + 1);
                // The redefinition's `Def` operand keeps the victim register.
                assert_eq!(
                    block.instructions[redefined].operands[1].virtual_register,
                    VirtualRegisterId(1)
                );
            }
            let reload_named_by = |block: &selected_instructions::SelectedBlock, id| {
                block
                    .instructions
                    .iter()
                    .find(|instruction| instruction.id == SelectedInstructionId(id))
                    .unwrap()
                    .operands[0]
                    .virtual_register
            };
            let count_loads = |block: &selected_instructions::SelectedBlock| {
                block
                    .instructions
                    .iter()
                    .filter(|instruction| {
                        matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                    })
                    .count()
            };
            // The redefinition closes the still-open span: the use at 4 reads
            // the new value through a fresh pair even under the crossing
            // policy, which crossed the call between the uses at 2 and 3.
            assert_eq!(count_loads(crossing_block), 2, "{target:?} call={call}");
            let early = reload_named_by(crossing_block, 2);
            assert_eq!(reload_named_by(crossing_block, 3), early);
            let late = reload_named_by(crossing_block, 4);
            assert_ne!(late, early);
            if call {
                // The bounded policy's pair closes at the call too, so the
                // use at 3 already names a fresh register before the
                // redefinition ends that span for the use at 4.
                assert_eq!(count_loads(bounded_block), 3, "{target:?} call={call}");
                let first = reload_named_by(bounded_block, 2);
                let second = reload_named_by(bounded_block, 3);
                let third = reload_named_by(bounded_block, 4);
                assert_ne!(first, second);
                assert_ne!(second, third);
                assert_ne!(first, third);
            } else {
                // No unit writer intervenes: both policies share one reload
                // across the uses at 2 and 3 and reopen after the
                // redefinition, producing the identical plan.
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
                "{target:?} call={call} bounded replay"
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
                "{target:?} call={call} crossing replay"
            );
            for (mutation, proposed) in [
                // Dropping the redefinition's own store leaves the post-write
                // value untracked.
                {
                    let mut proposed = crossing.transformed().clone();
                    let block = &mut proposed.functions[0].blocks[0];
                    let redefined = block
                        .instructions
                        .iter()
                        .position(|instruction| instruction.id == SelectedInstructionId(6))
                        .unwrap();
                    block.instructions.remove(redefined + 1);
                    proposed
                },
                // A use after the redefinition naming the stale pre-write
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
                    "{target:?} call={call} mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn a_parameter_victims_body_redefinition_stores_after_the_write() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let mut source = super::parameters::parameter_fixture(target);
        {
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            // The destination block holds uses 401 and 402; instruction 403
            // rewrites the parameter register between them.
            function.blocks[2].instructions.insert(
                1,
                admission::instruction(
                    SelectedInstructionId(403),
                    SelectedInstructionKind::CopyI64,
                    copy,
                    &[VirtualRegisterId(5), VirtualRegisterId(1)],
                ),
            );
        }
        let identity = selected_instruction_plan_identity(source.transformed());
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        let function = &result.transformed().functions[0];
        // Both edge arrivals still store their bound argument, and the
        // destination block gains the redefinition's store after 403.
        let block = &function.blocks[2];
        let redefined = block
            .instructions
            .iter()
            .position(|instruction| instruction.id == SelectedInstructionId(403))
            .unwrap();
        assert!(matches!(
            block.instructions[redefined + 1].kind,
            SelectedInstructionKind::Store64 { .. }
        ));
        let loads = block
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
            })
            .count();
        // One reload before the use at 401, a fresh one before the use at
        // 402: the redefinition closed the span between them.
        assert_eq!(loads, 2);
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

#[test]
fn redefinition_rejects_writes_before_existence_and_foreign_classes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap();
    let float_class = environment
        .constraint(environment.selected_keys().bits_to_float64.unwrap())
        .unwrap()
        .operands[1]
        .class;
    let victim_class = copy.operands[0].class;
    for mutation in 0..4 {
        let mut source = fixture(target);
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        match mutation {
            // A `Def` on the victim ahead of its origin definition in the
            // same block writes the register before it exists.
            0 => function.blocks[0].instructions.insert(
                0,
                admission::instruction(
                    SelectedInstructionId(6),
                    SelectedInstructionKind::CopyI64,
                    copy,
                    &[VirtualRegisterId(0), VirtualRegisterId(1)],
                ),
            ),
            // A second `Def` inside the origin instruction is not a later
            // definition at all.
            1 => function.blocks[0].instructions[0]
                .operands
                .push(SelectedOperand {
                    operand: 2,
                    virtual_register: VirtualRegisterId(1),
                    access: RegisterOperandAccess::Def,
                    class: victim_class,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }),
            // A redefinition in another register class is an inconsistent
            // plan, not a trackable write.
            2 => function.blocks[0].instructions.insert(
                3,
                admission::instruction(
                    SelectedInstructionId(6),
                    SelectedInstructionKind::CopyI64,
                    copy,
                    &[VirtualRegisterId(2), VirtualRegisterId(1)],
                ),
            ),
            // A `Def` on the victim at a terminator position has no following
            // store point — the block ends when it executes.
            _ => super::super::control_mut(&mut function.blocks[0].terminator)
                .operands
                .push(SelectedOperand {
                    operand: 0,
                    virtual_register: VirtualRegisterId(1),
                    access: RegisterOperandAccess::Def,
                    class: victim_class,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }),
        }
        if mutation == 2 {
            function.blocks[0].instructions[3].operands[1].class = float_class;
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
