//! Executable runtime-spill rewrite feeding the sequenced legality →
//! register-homes route. A `CallUnit` between the spill victim's flexible
//! uses can write the unit hosting a still-open reload. Under the bounded
//! span policy the rewrite closes the block's shared reload at that
//! boundary: each use opens its own address/load pair and no produced
//! interval ever demands a cross-call home. Under the crossing policy
//! admission instead proves the callee-saved survivor avoids every unit the
//! intervening call writes, so one reload pair stays open across it and the
//! produced interval homes to that survivor — the fixture's own filler held
//! live across the first call is exactly what keeps the survivor from being
//! free for the victim's original interval, while the reload's shorter span
//! across only the later call still fits it.

use crate::tests::{
    NativeTarget, RegisterOperandAccess, RegisterViewId, SelectedInstructionKind,
    VirtualRegisterOrigin, call_spanning_reload_allowlist, call_spanning_reload_caller,
    call_spanning_reload_incoming, call_spanning_reload_surviving_view,
    call_spanning_reload_victim, selected_lowering_budget, staged_call_spanning_reload_legality,
};
use register_homes::AllocationLegalityIdentity;
use selected_instructions_to_register_homes::{
    RegisterHomeError, RuntimeSpillError, RuntimeSpillSpanPolicy, SelectedProgramRef,
    analyze_allocation_legality, analyze_live_ranges, analyze_liveness, assign_register_homes,
    spill_selected_runtime_value, spill_selected_runtime_value_with_span_policy,
    validate_register_homes, validate_runtime_spill, validate_runtime_spill_with_span_policy,
};

#[test]
fn runtime_spill_closes_the_shared_reload_at_the_intervening_call_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::macos_arm64(),
    ] {
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
        let allowlisted = call_spanning_reload_allowlist(target)
            .iter()
            .map(|name| model.view_named(name).unwrap().id)
            .collect::<Vec<RegisterViewId>>();
        let function_index = selected
            .selected()
            .plan()
            .functions
            .iter()
            .position(|function| function.machine == caller)
            .expect("the caller must be selected");
        let registers = &selected.selected().plan().functions[function_index].virtual_registers;
        let source_register = |value| {
            registers
                .iter()
                .find(|register| {
                    matches!(register.origin, VirtualRegisterOrigin::InstructionResult {
                        source_value, ..
                    } if source_value == value)
                })
                .map(|register| register.id)
        };
        let victim = source_register(call_spanning_reload_victim())
            .expect("the victim must carry its source value");
        let incoming = source_register(call_spanning_reload_incoming())
            .expect("the cross-call filler must carry its source value");
        // The reduced allowlist cannot home the victim's original interval:
        // it reaches across every call, where every candidate but the
        // callee-saved survivor is clobbered — and the survivor is already
        // occupied by the filler live across the first call.
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
        // The `CallUnit` between the victim's uses may write the unit
        // hosting a still-open reload, so the shared reload closes there
        // and each side of the call opens a private frame-address/load
        // pair.
        let loads = block
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| {
                matches!(instruction.kind, SelectedInstructionKind::Load64 { .. }).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            loads.len(),
            2,
            "{target:?}: each flexible use keeps its own reload pair"
        );
        let reloads = loads
            .iter()
            .map(|&index| {
                let load = &block.instructions[index];
                let address = load
                    .operands
                    .iter()
                    .find(|operand| operand.access == RegisterOperandAccess::Use)
                    .expect("the load reads its address register")
                    .virtual_register;
                let opener = &block.instructions[index - 1];
                assert!(
                    matches!(opener.kind, SelectedInstructionKind::FrameAddress { .. }),
                    "{target:?}: each pair opens with a frame address"
                );
                assert_eq!(
                    opener
                        .operands
                        .iter()
                        .find(|operand| operand.access == RegisterOperandAccess::Def)
                        .expect("the frame address defines its register")
                        .virtual_register,
                    address,
                    "{target:?}: each pair's frame address feeds its own load"
                );
                load.operands
                    .iter()
                    .find(|operand| operand.access == RegisterOperandAccess::Def)
                    .expect("the load defines its reload register")
                    .virtual_register
            })
            .collect::<Vec<_>>();
        assert_ne!(
            reloads[0], reloads[1],
            "{target:?}: the call boundary keeps the two reloads distinct"
        );
        let consumers = reloads
            .iter()
            .map(|reload| {
                let uses = block
                    .instructions
                    .iter()
                    .enumerate()
                    .filter(|(_, instruction)| {
                        instruction.operands.iter().any(|operand| {
                            operand.virtual_register == *reload
                                && operand.access == RegisterOperandAccess::Use
                        })
                    })
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();
                assert_eq!(
                    uses.len(),
                    1,
                    "{target:?}: a bounded reload serves exactly one use"
                );
                uses[0]
            })
            .collect::<Vec<_>>();
        for (&load, &consumer) in loads.iter().zip(&consumers) {
            assert!(
                !block.instructions[load..=consumer]
                    .iter()
                    .any(|instruction| {
                        matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
                    }),
                "{target:?}: a produced reload interval never reaches across a call"
            );
        }
        // The uses genuinely straddle a call: a clobbering `CallUnit` sits
        // between the first pair's consumer and the second pair's load.
        assert!(consumers[0] < loads[1]);
        assert!(
            block.instructions[consumers[0] + 1..loads[1]]
                .iter()
                .any(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
                }),
            "{target:?}: the victim's uses must straddle a CallUnit"
        );

        // The sequenced route accepts the rewritten program. The only
        // interval still demanding a call-surviving home is the fixture's
        // own filler live across the first call; every produced reload is
        // call-free and homes inside the allowlist.
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
        let assignments = &homes.plan().functions[caller_index].assignments;
        let incoming_home = assignments
            .iter()
            .find(|assignment| assignment.virtual_register == incoming)
            .expect("the cross-call filler must receive a home");
        assert_eq!(
            incoming_home.view, saved,
            "{target:?}: the filler's interval still demands the callee-saved survivor"
        );
        for reload in &reloads {
            let home = assignments
                .iter()
                .find(|assignment| assignment.virtual_register == *reload)
                .expect("each produced reload must receive a home");
            assert!(
                allowlisted.contains(&home.view),
                "{target:?}: a bounded reload homes inside the allowlist"
            );
        }

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

        // Rewrite replay rejects a dropped pair, a post-call use rebound to
        // the pre-call reload — exactly the call-spanning shape the
        // boundary exists to refuse — a use rebound to the spilled victim,
        // and a bogus extra load.
        let mut dropped = transformed.clone();
        dropped.functions[function_index].blocks[0]
            .instructions
            .remove(loads[1]);
        let mut shared = transformed.clone();
        shared.functions[function_index].blocks[0].instructions[consumers[1]]
            .operands
            .iter_mut()
            .find(|operand| operand.virtual_register == reloads[1])
            .unwrap()
            .virtual_register = reloads[0];
        let mut revived = transformed.clone();
        revived.functions[function_index].blocks[0].instructions[consumers[1]]
            .operands
            .iter_mut()
            .find(|operand| operand.virtual_register == reloads[1])
            .unwrap()
            .virtual_register = victim;
        let mut extra = transformed.clone();
        extra.functions[function_index].blocks[0]
            .instructions
            .insert(consumers[1], block.instructions[loads[1]].clone());
        for changed in [dropped, shared, revived, extra] {
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

        // Home replay rejects a non-canonical view on a produced reload and
        // a foreign legality root.
        let canonical = assignments
            .iter()
            .find(|assignment| assignment.virtual_register == reloads[0])
            .unwrap()
            .view;
        let noncanonical = allowlisted
            .iter()
            .copied()
            .find(|view| *view != canonical)
            .expect("the allowlist holds a second view");
        let mut wrong_view = homes.plan().clone();
        wrong_view.functions[caller_index]
            .assignments
            .iter_mut()
            .find(|assignment| assignment.virtual_register == reloads[0])
            .unwrap()
            .view = noncanonical;
        let mut foreign_root = homes.plan().clone();
        foreign_root.legality = AllocationLegalityIdentity::from_bytes([0x5c; 32]);
        for (changed, error) in [
            (
                wrong_view,
                RegisterHomeError::VirtualRegisterMismatch {
                    function: caller_index,
                    register: reloads[0].0,
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

/// The crossing counterpart on the same staged legality: admission proves
/// the allowlist's callee-saved member avoids every unit the intervening
/// `CallUnit` writes, so one reload pair stays open across the call and the
/// produced interval homes to that survivor. The sequenced route assigns the
/// crossing plan, crossing replay accepts exactly it — the bounded replay of
/// the same plan, the bounded emission under crossing replay, and each
/// corrupted stream all reject.
#[test]
fn runtime_spill_crosses_the_intervening_call_onto_the_surviving_view_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::macos_arm64(),
    ] {
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
        let allowlisted = call_spanning_reload_allowlist(target)
            .iter()
            .map(|name| model.view_named(name).unwrap().id)
            .collect::<Vec<RegisterViewId>>();
        let function_index = selected
            .selected()
            .plan()
            .functions
            .iter()
            .position(|function| function.machine == caller)
            .expect("the caller must be selected");
        let registers = &selected.selected().plan().functions[function_index].virtual_registers;
        let victim = registers
            .iter()
            .find(|register| {
                matches!(register.origin, VirtualRegisterOrigin::InstructionResult {
                    source_value, ..
                } if source_value == call_spanning_reload_victim())
            })
            .map(|register| register.id)
            .expect("the victim must carry its source value");
        let incoming = registers
            .iter()
            .find(|register| {
                matches!(register.origin, VirtualRegisterOrigin::InstructionResult {
                    source_value, ..
                } if source_value == call_spanning_reload_incoming())
            })
            .map(|register| register.id)
            .expect("the cross-call filler must carry its source value");

        let source = SelectedProgramRef::new(selected.selected());
        let crossing = spill_selected_runtime_value_with_span_policy(
            &source,
            function_index,
            victim,
            environment,
            selected_lowering_budget(),
            RuntimeSpillSpanPolicy::UnitWriteCrossing,
        )
        .unwrap();
        let transformed = crossing.transformed();
        let block = transformed.functions[function_index]
            .blocks
            .first()
            .expect("the caller keeps its single block");
        // One store follows the victim's definition, and exactly one
        // address/load pair serves both flexible uses.
        let stores = block
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].operands[0].virtual_register, victim);
        let loads = block
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| {
                matches!(instruction.kind, SelectedInstructionKind::Load64 { .. }).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            loads.len(),
            1,
            "{target:?}: one shared reload crosses the intervening call"
        );
        let load = loads[0];
        assert!(matches!(
            block.instructions[load - 1].kind,
            SelectedInstructionKind::FrameAddress { .. }
        ));
        let reload = block.instructions[load]
            .operands
            .iter()
            .find(|operand| operand.access == RegisterOperandAccess::Def)
            .expect("the load defines its reload register")
            .virtual_register;
        let uses = block
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
            uses.len(),
            2,
            "{target:?}: the still-open reload serves both flexible uses"
        );
        assert!(
            block.instructions[load..uses[1]].iter().any(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
            }),
            "{target:?}: the produced interval reaches across a CallUnit"
        );

        // The sequenced route's probe: the rewritten program analyzes and
        // assigns, and the crossing reload's only legal home is the
        // callee-saved survivor — occupied by the fixture's filler only
        // across the first call, free for this shorter span.
        let crossing_liveness = analyze_liveness(&crossing).unwrap();
        let crossing_ranges = analyze_live_ranges(&crossing, &crossing_liveness).unwrap();
        let crossing_legality = analyze_allocation_legality(
            &crossing_ranges,
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
        )
        .unwrap();
        let homes = assign_register_homes(
            &crossing_legality,
            &crossing_ranges,
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
        let assignments = &homes.plan().functions[caller_index].assignments;
        let reload_home = assignments
            .iter()
            .find(|assignment| assignment.virtual_register == reload)
            .expect("the crossing reload must receive a home");
        assert_eq!(
            reload_home.view, saved,
            "{target:?}: the cross-call interval demands the callee-saved survivor"
        );
        let incoming_home = assignments
            .iter()
            .find(|assignment| assignment.virtual_register == incoming)
            .expect("the cross-call filler must receive a home");
        assert_eq!(
            incoming_home.view, saved,
            "{target:?}: the filler's interval still demands the callee-saved survivor"
        );

        // Independent replay re-derives the crossing shape; bounded replay
        // of the same plan expects the post-call pair that is not there.
        assert!(
            validate_runtime_spill_with_span_policy(
                &source,
                function_index,
                victim,
                environment,
                selected_lowering_budget(),
                transformed.clone(),
                RuntimeSpillSpanPolicy::UnitWriteCrossing,
            )
            .is_ok()
        );
        assert_eq!(
            validate_runtime_spill(
                &source,
                function_index,
                victim,
                environment,
                selected_lowering_budget(),
                transformed.clone(),
            ),
            Err(RuntimeSpillError::ReplayMismatch),
            "{target:?}: bounded replay must reject the call-crossing interval"
        );
        assert!(
            validate_register_homes(
                &crossing_legality,
                &crossing_ranges,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
                homes.plan().clone(),
            )
            .is_ok()
        );

        // The bounded emission — a second private pair after the call — is
        // not the crossing canonical output.
        let bounded = spill_selected_runtime_value(
            &source,
            function_index,
            victim,
            environment,
            selected_lowering_budget(),
        )
        .unwrap();
        assert_eq!(
            validate_runtime_spill_with_span_policy(
                &source,
                function_index,
                victim,
                environment,
                selected_lowering_budget(),
                bounded.transformed().clone(),
                RuntimeSpillSpanPolicy::UnitWriteCrossing,
            ),
            Err(RuntimeSpillError::ReplayMismatch),
            "{target:?}: crossing replay must reject the bounded shape"
        );

        // Corruptions under crossing replay: the shared load dropped, the
        // post-call use rebound to the spilled victim, and the bounded
        // post-call pair spliced in — each must fail independent replay.
        let mut dropped = transformed.clone();
        dropped.functions[function_index].blocks[0]
            .instructions
            .remove(load);
        let mut revived = transformed.clone();
        revived.functions[function_index].blocks[0].instructions[uses[1]]
            .operands
            .iter_mut()
            .find(|operand| operand.virtual_register == reload)
            .unwrap()
            .virtual_register = victim;
        let mut extra = transformed.clone();
        {
            let bounded_block = &bounded.transformed().functions[function_index].blocks[0];
            let pair: Vec<_> = bounded_block
                .instructions
                .iter()
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::FrameAddress { .. }
                            | SelectedInstructionKind::Load64 { .. }
                    )
                })
                .skip(1)
                .cloned()
                .collect();
            let block = &mut extra.functions[function_index].blocks[0];
            let at = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == block.instructions[uses[1]].id)
                .unwrap();
            block.instructions.splice(at..at, pair);
        }
        for changed in [dropped, revived, extra] {
            assert_eq!(
                validate_runtime_spill_with_span_policy(
                    &source,
                    function_index,
                    victim,
                    environment,
                    selected_lowering_budget(),
                    changed,
                    RuntimeSpillSpanPolicy::UnitWriteCrossing,
                ),
                Err(RuntimeSpillError::ReplayMismatch),
                "{target:?}: corrupted crossing stream must fail independent replay"
            );
        }

        // Home replay rejects a non-surviving allowlist view on the crossing
        // reload — its interval reaches across the call's clobber set — and
        // a foreign legality root.
        let noncanonical = allowlisted
            .iter()
            .copied()
            .find(|view| *view != saved)
            .expect("the allowlist holds a caller-saved view");
        let mut wrong_view = homes.plan().clone();
        wrong_view.functions[caller_index]
            .assignments
            .iter_mut()
            .find(|assignment| assignment.virtual_register == reload)
            .unwrap()
            .view = noncanonical;
        let mut foreign_root = homes.plan().clone();
        foreign_root.legality = AllocationLegalityIdentity::from_bytes([0x5c; 32]);
        for changed in [wrong_view, foreign_root] {
            assert!(
                validate_register_homes(
                    &crossing_legality,
                    &crossing_ranges,
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    &environment.allocation_constraint_keys(),
                    changed,
                )
                .is_err(),
                "{target:?}: corrupted crossing homes must fail independent replay"
            );
        }
    }
}
