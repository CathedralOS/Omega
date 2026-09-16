//! Executable runtime-spill rewrite feeding the sequenced legality →
//! register-homes route. The rewritten victim's shared reload interval
//! survives an intervening `CallUnit`, so allocation must give it the only
//! allowlisted view whose units the call does not clobber.

use crate::tests::{
    NativeTarget, RegisterOperandAccess, SelectedInstructionKind, VirtualRegisterId,
    VirtualRegisterOrigin, call_spanning_reload_allowlist, call_spanning_reload_caller,
    call_spanning_reload_surviving_view, call_spanning_reload_victim, selected_lowering_budget,
    staged_call_spanning_reload_legality,
};
use register_homes::AllocationLegalityIdentity;
use selected_instructions_to_register_homes::{
    RegisterHomeError, RuntimeSpillError, SelectedProgramRef, analyze_allocation_legality,
    analyze_live_ranges, analyze_liveness, assign_register_homes, spill_selected_runtime_value,
    validate_register_homes, validate_runtime_spill,
};

#[test]
fn runtime_spill_reload_interval_survives_the_intervening_call_on_every_target() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let legality = staged_call_spanning_reload_legality(target);
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        let model = environment.physical().model();
        let caller = call_spanning_reload_caller();
        let saved = model
            .view_named(call_spanning_reload_surviving_view(target))
            .unwrap()
            .id;
        let function_index = selected
            .selected()
            .plan()
            .functions
            .iter()
            .position(|function| function.machine == caller)
            .expect("the caller must be selected");
        let victim = selected.selected().plan().functions[function_index]
            .virtual_registers
            .iter()
            .find(|register| {
                matches!(register.origin, VirtualRegisterOrigin::InstructionResult {
                    source_value, ..
                } if source_value == call_spanning_reload_victim())
            })
            .expect("the victim must carry its source value")
            .id;
        // The reduced allowlist cannot home the victim's original interval:
        // every candidate meets a `CallUnit` clobber or an argument pin.
        assert!(
            assign_register_homes(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
            )
            .is_err(),
            "{target:?}: the allowlist must leave the victim without a home"
        );

        let source = SelectedProgramRef::new(selected.selected());
        let spilled = spill_selected_runtime_value(
            &source,
            function_index,
            victim,
            environment,
            selected_lowering_budget(),
        )
        .unwrap();
        let transformed = spilled.transformed();
        let block = transformed.functions[function_index]
            .blocks
            .first()
            .expect("the caller keeps its single block");
        // One store follows the victim's definition; the victim survives
        // nowhere else in the rewritten stream.
        let stores = block
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].operands[0].virtual_register, victim);
        // The block shares one address/load pair across every flexible use:
        // exactly one `Load64` defines the reload register and both call
        // argument copies name it.
        let pair = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::FrameAddress { .. }
                )
            })
            .expect("the shared reload must open with a frame address");
        let load = &block.instructions[pair + 1];
        assert!(matches!(load.kind, SelectedInstructionKind::Load64 { .. }));
        let reload = load
            .operands
            .iter()
            .find(|operand| operand.access == RegisterOperandAccess::Def)
            .expect("the load defines its reload register")
            .virtual_register;
        let consumers = block
            .instructions
            .iter()
            .enumerate()
            .filter(|(_, instruction)| {
                instruction.operands.iter().any(|operand| {
                    operand.virtual_register == reload
                        && operand.access == RegisterOperandAccess::Use
                })
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(
            consumers.len(),
            2,
            "{target:?}: both flexible uses share the reload"
        );
        assert!(
            block.instructions[consumers[0]..consumers[1]]
                .iter()
                .any(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
                }),
            "{target:?}: the shared reload interval must span a CallUnit"
        );

        // The sequenced route accepts the rewritten program and homes the
        // call-spanning reload on the callee-saved survivor.
        let spilled_liveness = analyze_liveness(&spilled).unwrap();
        let spilled_ranges = analyze_live_ranges(&spilled, &spilled_liveness).unwrap();
        let spilled_legality = analyze_allocation_legality(
            &spilled_ranges,
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
        )
        .unwrap();
        let homes = assign_register_homes(
            &spilled_legality,
            &spilled_ranges,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
        )
        .unwrap();
        let caller_index = homes
            .plan()
            .functions
            .iter()
            .position(|function| function.machine == caller)
            .unwrap();
        let home = homes.plan().functions[caller_index]
            .assignments
            .iter()
            .find(|assignment| assignment.virtual_register == reload)
            .expect("the shared reload must receive a home");
        assert_eq!(
            home.view, saved,
            "{target:?}: the reload must land callee-saved"
        );

        // Independent replay re-derives both stages: the rewrite's inserted
        // stream and the home assignment's canonical plan.
        assert!(
            validate_runtime_spill(
                &source,
                function_index,
                victim,
                environment,
                selected_lowering_budget(),
                transformed.clone(),
            )
            .is_ok()
        );
        assert!(
            validate_register_homes(
                &spilled_legality,
                &spilled_ranges,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
                homes.plan().clone(),
            )
            .is_ok()
        );

        // Rewrite replay rejects a stream that drops the shared load, splits
        // the second use onto a private register, or inserts a bogus load.
        let mut dropped = transformed.clone();
        dropped.functions[function_index].blocks[0]
            .instructions
            .remove(pair + 1);
        let mut split = transformed.clone();
        split.functions[function_index].blocks[0].instructions[consumers[1]]
            .operands
            .iter_mut()
            .find(|operand| operand.virtual_register == reload)
            .unwrap()
            .virtual_register = VirtualRegisterId(reload.0 + 1_000);
        let mut extra = transformed.clone();
        extra.functions[function_index].blocks[0]
            .instructions
            .insert(consumers[1], load.clone());
        for changed in [dropped, split, extra] {
            assert_eq!(
                validate_runtime_spill(
                    &source,
                    function_index,
                    victim,
                    environment,
                    selected_lowering_budget(),
                    changed,
                ),
                Err(RuntimeSpillError::ReplayMismatch),
                "{target:?}: corrupted stream must fail independent replay"
            );
        }

        // Home replay rejects a caller-saved assignment and a foreign root.
        let caller_saved = call_spanning_reload_allowlist(target)
            .iter()
            .map(|name| model.view_named(name).unwrap().id)
            .find(|view| *view != saved)
            .unwrap();
        let mut caller_saved_home = homes.plan().clone();
        caller_saved_home.functions[caller_index]
            .assignments
            .iter_mut()
            .find(|assignment| assignment.virtual_register == reload)
            .unwrap()
            .view = caller_saved;
        let mut foreign_root = homes.plan().clone();
        foreign_root.legality = AllocationLegalityIdentity::from_bytes([0x5c; 32]);
        for (changed, error) in [
            (
                caller_saved_home,
                RegisterHomeError::VirtualRegisterMismatch {
                    function: caller_index,
                    register: reload.0,
                },
            ),
            (foreign_root, RegisterHomeError::RootMismatch),
        ] {
            assert_eq!(
                validate_register_homes(
                    &spilled_legality,
                    &spilled_ranges,
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    &environment.allocation_constraint_keys(),
                    changed,
                ),
                Err(error),
                "{target:?}: corrupted homes must fail independent replay"
            );
        }
    }
}
