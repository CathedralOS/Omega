mod codegen;
mod format;
mod host;
mod identity;
mod object;
mod stats;

use omega_artifacts::NativeSurfaceReport;
use omega_backend_plan::NativePlan;
use omega_control_flow::StateKey;
use omega_runtime_branching::{
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_schedule::{
    StateScheduleContext, build_entry_state_schedule, scheduled_state_flow,
};
use omega_typed_program::statement::TransitionGuard;

pub fn native_report_text(
    native_surface: &NativeSurfaceReport,
    native_plan: &NativePlan,
) -> String {
    let mut output = String::new();

    output.push_str("# Omega Native Plan\n\n");
    output.push_str(&format!("target: {:?}\n", native_plan.target));
    output.push_str(&format!(
        "entry: {}.{} as `{}`\n\n",
        native_plan.entry_machine_name(),
        native_plan.entry_state_name(),
        native_plan.object.entry_symbol
    ));

    stats::write_native_phase_timings(&mut output, native_plan);
    stats::write_native_string_storage(&mut output, native_plan);

    host::write_host_sections(&mut output, native_plan);

    output.push_str("## State Call Lowering\n");
    output.push_str(&format!("calls: {}\n", native_plan.state_calls.calls.len()));
    if native_plan.state_calls.calls.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, state_call) in native_plan.state_calls.calls.iter() {
            let source_name = native_state_name(native_plan, state_call.source_key);
            let target_name = if state_call.target_key.is_valid() {
                native_state_name(native_plan, state_call.target_key)
            } else {
                "unresolved".to_owned()
            };
            output.push_str(&format!(
                "- {} statement {} `{}` -> {} args {} {:?}/{:?} reachable {} required {}\n",
                source_name,
                state_call.statement_index,
                state_call.receiver_display,
                target_name,
                state_call.argument_count,
                state_call.resolution,
                state_call.lowering,
                state_call.reachable,
                state_call.required
            ));

            match native_plan.state_calls.arguments.span(state_call.arguments) {
                Some(arguments) if arguments.is_empty() => {
                    output.push_str("  arguments: none\n");
                }
                Some(arguments) => {
                    output.push_str("  arguments:\n");
                    for argument in arguments {
                        output.push_str(&format!(
                            "    - #{} `{}` {:?}: `{}` required {}\n",
                            argument.index,
                            argument.parameter_name,
                            argument.kind,
                            argument.expression.display_name(),
                            argument.required
                        ));
                    }
                }
                None => output.push_str("  arguments: invalid span\n"),
            }
        }
    }
    output.push('\n');

    output.push_str("## Alias Flow\n");
    output.push_str(&format!(
        "aliases: {}\n",
        native_plan.alias_flow.aliases.len()
    ));
    if native_plan.alias_flow.aliases.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, alias) in native_plan.alias_flow.aliases.iter() {
            let caller_name = native_state_name(native_plan, alias.caller_key);
            let callee_name = native_state_name(native_plan, alias.callee_key);
            output.push_str(&format!(
                "- {} statement {} -> {} `{}` aliases `{}` required {}\n",
                caller_name,
                alias.statement_index,
                callee_name,
                alias.parameter_name,
                alias.argument.display_name(),
                alias.required
            ));
        }
    }
    output.push('\n');

    output.push_str("## State Storage\n");
    output.push_str(&format!(
        "locals: {}\n",
        native_plan.state_storage.locals.len()
    ));
    for (_, local) in native_plan.state_storage.locals.iter() {
        let source_name = native_state_name(native_plan, local.source_key);
        output.push_str(&format!(
            "- {} statement {} local `{}`: {} required {}\n",
            source_name, local.statement_index, local.name, local.type_name, local.required
        ));
    }
    output.push_str(&format!(
        "mutations: {}\n",
        native_plan.state_storage.mutations.len()
    ));
    for (_, mutation) in native_plan.state_storage.mutations.iter() {
        let source_name = native_state_name(native_plan, mutation.source_key);
        output.push_str(&format!(
            "- {} statement {} {:?}/{:?}: `{}` = `{}` required {}\n",
            source_name,
            mutation.statement_index,
            mutation.mutation_kind,
            mutation.lowering,
            mutation.target.display_name(),
            mutation.value.display_name(),
            mutation.required
        ));
    }
    output.push('\n');

    output.push_str("## Runtime Storage\n");
    output.push_str(&format!(
        "frame slots: {}\n",
        native_plan.runtime_storage.frame_slots.len()
    ));
    for (_, slot) in native_plan.runtime_storage.frame_slots.iter() {
        let source_name = native_state_name(native_plan, slot.source_key);
        output.push_str(&format!(
            "- #{} {} statement {} local `{}`: {} offset {} bytes {} align {}\n",
            slot.dispatch_index,
            source_name,
            slot.statement_index,
            slot.name,
            slot.type_name,
            slot.byte_offset,
            slot.byte_size,
            slot.alignment
        ));
    }
    output.push_str(&format!(
        "writes: {}\n",
        native_plan.runtime_storage.writes.len()
    ));
    for (_, write) in native_plan.runtime_storage.writes.iter() {
        let source_name = native_state_name(native_plan, write.source_key);
        output.push_str(&format!(
            "- #{} {} statement {} {:?}/{:?}: `{}` = `{}`\n",
            write.dispatch_index,
            source_name,
            write.statement_index,
            write.mutation_kind,
            write.lowering,
            write.target.display_name(),
            write.value.display_name()
        ));
    }
    output.push('\n');

    output.push_str("## State Values\n");
    output.push_str(&format!(
        "values: {}\n",
        native_plan.state_values.values.len()
    ));
    for (_, value) in native_plan.state_values.values.iter() {
        let source_name = native_state_name(native_plan, value.source_key);
        output.push_str(&format!(
            "- {} statement {} {:?}/{:?}: `{}` required {}\n",
            source_name,
            value.statement_index,
            value.role,
            value.kind,
            value.expression.display_name(),
            value.required
        ));
    }
    output.push('\n');

    output.push_str("## Runtime Text\n");
    output.push_str(&format!("uses: {}\n", native_plan.runtime_text.uses.len()));
    output.push_str(&format!(
        "buffers: {}\n",
        native_plan.runtime_text.buffers.len()
    ));
    output.push_str(&format!(
        "slots: {}\n",
        native_plan.runtime_text.slots.len()
    ));
    output.push_str(&format!(
        "writes: {}\n",
        native_plan.runtime_text.writes.len()
    ));
    output.push_str(&format!(
        "builders: {}\n",
        native_plan.runtime_text.builders.len()
    ));
    output.push_str(&format!(
        "builder segments: {}\n",
        native_plan.runtime_text.builder_segments.len()
    ));
    if native_plan.runtime_text.uses.is_empty() {
        output.push_str("uses: none\n");
    } else {
        for (_, text_use) in native_plan.runtime_text.uses.iter() {
            let source_name = native_state_name(native_plan, text_use.source_key);
            output.push_str(&format!(
                "- {} statement {} `{}` {:?} newline {}\n",
                source_name,
                text_use.statement_index,
                text_use.expression.display_name(),
                text_use.source,
                text_use.append_newline
            ));
        }
    }
    if native_plan.runtime_text.buffers.is_empty() {
        output.push_str("buffers: none\n");
    } else {
        for (_, text_buffer) in native_plan.runtime_text.buffers.iter() {
            let source_name = native_state_name(native_plan, text_buffer.source_key);
            output.push_str(&format!(
                "- buffer {} statement {} `{}` bytes {}\n",
                source_name,
                text_buffer.statement_index,
                text_buffer.target.display_name(),
                text_buffer.byte_capacity
            ));
        }
    }
    if native_plan.runtime_text.slots.is_empty() {
        output.push_str("slots: none\n");
    } else {
        for (_, text_slot) in native_plan.runtime_text.slots.iter() {
            output.push_str(&format!(
                "- slot `{}` bytes {} input_buffer {}\n",
                text_slot.place.display_name(),
                text_slot.byte_capacity,
                text_slot.has_input_buffer
            ));
        }
    }
    if native_plan.runtime_text.writes.is_empty() {
        output.push_str("writes: none\n");
    } else {
        for (_, text_write) in native_plan.runtime_text.writes.iter() {
            let source_name = native_state_name(native_plan, text_write.source_key);
            output.push_str(&format!(
                "- write {} statement {} `{}` = `{}` {:?}\n",
                source_name,
                text_write.statement_index,
                text_write.target.display_name(),
                text_write.value.display_name(),
                text_write.kind
            ));
        }
    }
    if native_plan.runtime_text.builders.is_empty() {
        output.push_str("builders: none\n");
    } else {
        for (_, text_builder) in native_plan.runtime_text.builders.iter() {
            let source_name = native_state_name(native_plan, text_builder.source_key);
            output.push_str(&format!(
                "- builder {} statement {} `{}` segments {}\n",
                source_name,
                text_builder.statement_index,
                text_builder.target.display_name(),
                text_builder.segments.count()
            ));
            if let Some(segments) = native_plan
                .runtime_text
                .builder_segments
                .span(text_builder.segments)
            {
                for segment in segments {
                    output.push_str(&format!(
                        "  - segment `{}` {:?}\n",
                        segment.expression.display_name(),
                        segment.kind
                    ));
                }
            }
        }
    }
    output.push('\n');

    codegen::write_codegen_sections(&mut output, native_plan);

    output.push_str("## Source Native Surface\n");
    output.push_str(&format!(
        "entry candidates: {}\n",
        native_surface.entry_points.len()
    ));
    for (_, entry_point) in native_surface.entry_points.iter() {
        output.push_str(&format!(
            "- entry {}.{}\n",
            entry_point.machine, entry_point.state
        ));
    }

    output.push_str(&format!("platforms: {}\n", native_surface.platforms.len()));
    for (_, platform) in native_surface.platforms.iter() {
        output.push_str(&format!(
            "- platform {}: {} state(s)\n",
            platform.name, platform.states
        ));
    }

    output.push_str(&format!("machines: {}\n", native_surface.machines.len()));
    for (_, machine) in native_surface.machines.iter() {
        output.push_str(&format!(
            "- machine {}: contains {}, owned data {}, states {}\n",
            machine.name, machine.contained_objects, machine.owned_data, machine.states
        ));
    }
    output.push('\n');

    output.push_str("## State Schedule\n");
    let schedule_context =
        StateScheduleContext::new(&native_plan.control_flow, &native_plan.host_calls);
    match build_entry_state_schedule(&schedule_context, native_plan.entry_key) {
        Ok(schedule) if schedule.is_empty() => output.push_str("states: 0\nnone\n"),
        Ok(schedule) => {
            output.push_str(&format!("states: {}\n", schedule.len()));
            for scheduled_state in schedule {
                if let Some(state_flow) = scheduled_state_flow(&schedule_context, &scheduled_state)
                {
                    output.push_str(&format!(
                        "- {}.{}#{}\n",
                        native_plan
                            .control_flow
                            .machines
                            .iter()
                            .find(|(_, machine)| machine.symbol == state_flow.key.machine)
                            .map(|(_, machine)| machine.name.as_str())
                            .unwrap_or("<missing-machine>"),
                        state_flow.name,
                        state_flow.key.segment_index
                    ));
                } else {
                    output.push_str(&format!(
                        "- symbol {}.{}#{}\n",
                        scheduled_state.key.machine.arena_index(),
                        scheduled_state.key.state.arena_index(),
                        scheduled_state.key.segment_index
                    ));
                }
            }
        }
        Err(reason) => {
            output.push_str("status: blocked\n");
            output.push_str(&format!("reason: {reason}\n"));
        }
    }

    output.push_str("\n## Runtime State Flow\n");
    output.push_str(&format!(
        "states: {}\n",
        native_plan.runtime_flow.states.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        native_plan.runtime_flow.edges.len()
    ));
    output.push_str(&format!(
        "cycles: {}\n",
        native_plan.runtime_flow.cycles.len()
    ));
    if native_plan.runtime_flow.states.is_empty() {
        output.push_str("none\n");
    } else {
        output.push_str("states:\n");
        for (_, state) in native_plan.runtime_flow.states.iter() {
            output.push_str(&format!(
                "- {}\n",
                native_state_name(native_plan, state.key)
            ));
        }
    }
    if !native_plan.runtime_flow.edges.is_empty() {
        output.push_str("edges:\n");
        for (_, edge) in native_plan.runtime_flow.edges.iter() {
            output.push_str(&format!(
                "- {} -> {} {}",
                native_state_name(native_plan, edge.from),
                runtime_transition_target_name(native_plan, &edge.target),
                transition_guard_name(&edge.guard)
            ));

            if edge.continuation != RuntimeTransitionTarget::None {
                output.push_str(&format!(
                    " -> {}",
                    runtime_transition_target_name(native_plan, &edge.continuation)
                ));
            }

            if edge.forms_cycle {
                output.push_str(" [cycle]");
            }

            output.push('\n');
        }
    }
    if !native_plan.runtime_flow.cycles.is_empty() {
        output.push_str("cycle paths:\n");
        for (_, cycle) in native_plan.runtime_flow.cycles.iter() {
            match native_plan.runtime_flow.cycle_states.span(cycle.states) {
                Some(states) => {
                    let path = states
                        .iter()
                        .map(|state| native_state_name(native_plan, state.key))
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    output.push_str(&format!("- {path}\n"));
                }
                None => output.push_str("- invalid cycle span\n"),
            }
        }
    }

    output.push_str("\n## Runtime Dispatch\n");
    output.push_str(&format!(
        "states: {}\n",
        native_plan.state_dispatch.states.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        native_plan.state_dispatch.edges.len()
    ));
    if native_plan.state_dispatch.states.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, state) in native_plan.state_dispatch.states.iter() {
            let machine_name = native_plan
                .control_flow
                .machine_by_symbol(state.key.machine)
                .map(|machine| machine.name.as_str())
                .unwrap_or("<unknown>");
            let state_name = native_plan
                .control_flow
                .state_by_key(state.key)
                .map(|state| state.name.as_str())
                .unwrap_or("<unknown>");
            output.push_str(&format!(
                "- #{} {}.{} label `{}`\n",
                state.dispatch_index, machine_name, state_name, state.label
            ));

            match native_plan.state_dispatch.edges.span(state.edges) {
                Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                Some(edges) => {
                    output.push_str("  edges:\n");
                    for edge in edges {
                        output.push_str(&format!(
                            "    - -> #{} {} {}",
                            edge.target_dispatch_index,
                            runtime_transition_target_name(native_plan, &edge.target),
                            transition_guard_name(&edge.guard)
                        ));

                        if edge.continuation != RuntimeTransitionTarget::None {
                            output.push_str(&format!(
                                " -> #{} {}",
                                edge.continuation_dispatch_index,
                                runtime_transition_target_name(native_plan, &edge.continuation)
                            ));
                        }

                        if edge.forms_cycle {
                            output.push_str(" [cycle]");
                        }

                        output.push('\n');
                    }
                }
                None => output.push_str("  edges: invalid span\n"),
            }
        }
    }

    output.push_str("\n## Runtime Guards\n");
    output.push_str(&format!(
        "guards: {}\n",
        native_plan.state_guards.guards.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        native_plan.state_guards.operands.len()
    ));
    if native_plan.state_guards.guards.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, guard) in native_plan.state_guards.guards.iter() {
            let machine_name = native_plan
                .control_flow
                .machine_by_symbol(guard.source.machine)
                .map(|machine| machine.name.as_str())
                .unwrap_or("<unknown>");
            let state_name = native_plan
                .control_flow
                .state_by_key(guard.source)
                .map(|state| state.name.as_str())
                .unwrap_or("<unknown>");
            output.push_str(&format!(
                "- #{} {}.{} edge {} -> #{} {} {:?}/{:?}/{:?}",
                guard.source_dispatch_index,
                machine_name,
                state_name,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(native_plan, &guard.target),
                guard.kind,
                guard.operator,
                guard.lowering
            ));

            if guard.has_expression {
                output.push_str(&format!(" `{}`", guard.expression.display_name()));
            }

            if guard.continuation != RuntimeTransitionTarget::None {
                output.push_str(&format!(
                    " -> #{} {}",
                    guard.continuation_dispatch_index,
                    runtime_transition_target_name(native_plan, &guard.continuation)
                ));
            }

            if guard.forms_cycle {
                output.push_str(" [cycle]");
            }

            output.push('\n');
            if let Some(operands) = native_plan.state_guards.operands.span(guard.operands)
                && !operands.is_empty()
            {
                for operand in operands {
                    output.push_str(&format!(
                        "  - operand `{}` {:?} {:?} offset {} bytes {}\n",
                        operand.expression.display_name(),
                        operand.kind,
                        operand.storage,
                        operand.byte_offset,
                        operand.byte_size
                    ));
                    if operand.has_resolved_value {
                        output
                            .push_str(&format!("    resolved value: {}\n", operand.resolved_value));
                    }
                }
            }
        }
    }

    output.push_str("\n## Runtime Dispatch Loop\n");
    output.push_str(&format!(
        "needed: {}\n",
        native_plan.runtime_dispatch_loop.needed
    ));
    output.push_str(&format!(
        "entry dispatch index: #{}\n",
        native_plan.runtime_dispatch_loop.entry_dispatch_index
    ));
    output.push_str(&format!(
        "terminal dispatch index: #{}\n",
        native_plan.runtime_dispatch_loop.terminal_dispatch_index
    ));
    output.push_str(&format!(
        "current state slot: `{}`\n",
        native_plan.runtime_dispatch_loop.current_state_slot
    ));
    output.push_str(&format!(
        "next state slot: `{}`\n",
        native_plan.runtime_dispatch_loop.next_state_slot
    ));
    output.push_str(&format!(
        "cases: {}\n",
        native_plan.runtime_dispatch_loop.cases.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        native_plan.runtime_dispatch_loop.edges.len()
    ));
    if native_plan.runtime_dispatch_loop.cases.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, dispatch_case) in native_plan.runtime_dispatch_loop.cases.iter() {
            let machine_name = native_plan
                .control_flow
                .machine_by_symbol(dispatch_case.key.machine)
                .map(|machine| machine.name.as_str())
                .unwrap_or("<unknown>");
            let state_name = native_plan
                .control_flow
                .state_by_key(dispatch_case.key)
                .map(|state| state.name.as_str())
                .unwrap_or("<unknown>");
            output.push_str(&format!(
                "- #{} {}.{} label `{}` operations {}\n",
                dispatch_case.dispatch_index,
                machine_name,
                state_name,
                dispatch_case.label,
                dispatch_case.operation_count
            ));

            match native_plan
                .runtime_dispatch_loop
                .edges
                .span(dispatch_case.edges)
            {
                Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                Some(edges) => {
                    output.push_str("  edges:\n");
                    for edge in edges {
                        output.push_str(&format!(
                            "    - #{} -> #{} {} {:?}/{:?} {}",
                            edge.order,
                            edge.target_dispatch_index,
                            runtime_transition_target_name(native_plan, &edge.target),
                            edge.guard_lowering,
                            edge.action,
                            transition_guard_name(&edge.guard)
                        ));
                        if edge.guard_has_storage {
                            output.push_str(&format!(
                                " storage offset {} bytes {} expected {}",
                                edge.guard_byte_offset,
                                edge.guard_byte_size,
                                edge.guard_expected_value
                            ));
                        }

                        if edge.continuation != RuntimeTransitionTarget::None {
                            output.push_str(&format!(
                                " -> #{} {}",
                                edge.continuation_dispatch_index,
                                runtime_transition_target_name(native_plan, &edge.continuation)
                            ));
                        }

                        if edge.forms_cycle {
                            output.push_str(" [cycle]");
                        }

                        output.push('\n');
                    }
                }
                None => output.push_str("  edges: invalid span\n"),
            }
        }
    }

    output.push_str("\n## Runtime Bodies\n");
    output.push_str(&format!(
        "bodies: {}\n",
        native_plan.runtime_bodies.bodies.len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        native_plan.runtime_bodies.operations.len()
    ));
    if native_plan.runtime_bodies.bodies.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, body) in native_plan.runtime_bodies.bodies.iter() {
            let source_name = native_state_name(native_plan, body.key);
            output.push_str(&format!("- #{} {}\n", body.dispatch_index, source_name));

            match native_plan.runtime_bodies.operations.span(body.operations) {
                Some(operations) if operations.is_empty() => {
                    output.push_str("  operations: none\n");
                }
                Some(operations) => {
                    output.push_str("  operations:\n");
                    for operation in operations {
                        let source_name = native_state_name(native_plan, operation.source_key);
                        output.push_str(&format!(
                            "    - {} statement {} {:?}\n",
                            source_name, operation.statement_index, operation.kind
                        ));
                    }
                }
                None => output.push_str("  operations: invalid span\n"),
            }
        }
    }

    output.push_str("\n## Runtime Branching Calls\n");
    output.push_str(&format!(
        "calls: {}\n",
        native_plan.runtime_branching_calls.calls.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        native_plan.runtime_branching_calls.edges.len()
    ));
    if native_plan.runtime_branching_calls.calls.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, call) in native_plan.runtime_branching_calls.calls.iter() {
            let source_name = native_state_name(native_plan, call.source_key);
            let target_name = native_state_name(native_plan, call.target_key);
            output.push_str(&format!(
                "- #{} {} statement {} -> {} args {}\n",
                call.dispatch_index,
                source_name,
                call.statement_index,
                target_name,
                call.argument_count
            ));

            match native_plan.runtime_branching_calls.edges.span(call.edges) {
                Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                Some(edges) => {
                    output.push_str(&format!("  expansion: {:?}\n", call.expansion));
                    output.push_str("  edges:\n");
                    for edge in edges {
                        output.push_str(&format!(
                            "    - #{} -> {} {:?} {:?} {}",
                            edge.order,
                            runtime_transition_target_name(native_plan, &edge.target),
                            edge.lowering,
                            edge.guard_kind,
                            transition_guard_name(&edge.guard)
                        ));

                        let target_arguments = native_plan
                            .runtime_branching_calls
                            .target_arguments
                            .span_or_empty(edge.target_arguments);
                        if !target_arguments.is_empty() {
                            output.push_str(&format!(
                                " args ({})",
                                target_arguments
                                    .iter()
                                    .map(|argument| argument.display_name())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                        }

                        if edge.continuation != RuntimeTransitionTarget::None {
                            output.push_str(&format!(
                                " -> {}",
                                runtime_transition_target_name(native_plan, &edge.continuation)
                            ));
                        }

                        output.push('\n');
                    }
                }
                None => output.push_str("  edges: invalid span\n"),
            }
        }
    }

    output.push_str("\n## Runtime Leaf Branch Expansions\n");
    output.push_str(&format!(
        "expansions: {}\n",
        native_plan.runtime_branching_calls.leaf_expansions.len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        native_plan.runtime_branching_calls.leaf_operations.len()
    ));
    output.push_str(&format!(
        "bindings: {}\n",
        native_plan.runtime_branching_calls.leaf_bindings.len()
    ));
    if native_plan
        .runtime_branching_calls
        .leaf_expansions
        .is_empty()
    {
        output.push_str("none\n");
    } else {
        for (_, expansion) in native_plan.runtime_branching_calls.leaf_expansions.iter() {
            let source_name = native_state_name(native_plan, expansion.source_key);
            let branch_name = native_state_name(native_plan, expansion.branch_key);
            let leaf_name = native_state_name(native_plan, expansion.leaf_key);
            output.push_str(&format!(
                "- #{} {} statement {} {} edge {} -> {} {:?} {}\n",
                expansion.dispatch_index,
                source_name,
                expansion.statement_index,
                branch_name,
                expansion.edge_order,
                leaf_name,
                expansion.guard_kind,
                transition_guard_name(&expansion.guard)
            ));
            if expansion.resolved_guard != expansion.guard {
                output.push_str(&format!(
                    "  resolved guard: {}\n",
                    transition_guard_name(&expansion.resolved_guard)
                ));
            }

            match native_plan
                .runtime_branching_calls
                .leaf_bindings
                .span(expansion.bindings)
            {
                Some(bindings) if bindings.is_empty() => {
                    output.push_str("  bindings: none\n");
                }
                Some(bindings) => {
                    output.push_str("  bindings:\n");
                    for binding in bindings {
                        output.push_str(&format!(
                            "    - {:?} `{}` = `{}`\n",
                            binding.kind,
                            binding.parameter_name,
                            binding.expression.display_name()
                        ));
                    }
                }
                None => output.push_str("  bindings: invalid span\n"),
            }

            match native_plan
                .runtime_branching_calls
                .leaf_operations
                .span(expansion.operations)
            {
                Some(operations) if operations.is_empty() => {
                    output.push_str("  operations: none\n");
                }
                Some(operations) => {
                    output.push_str("  operations:\n");
                    for operation in operations {
                        write_runtime_leaf_branch_operation(&mut output, native_plan, operation);
                    }
                }
                None => output.push_str("  operations: invalid span\n"),
            }
        }
    }

    output.push_str("\n## Runtime Straight-Line Branch Expansions\n");
    output.push_str(&format!(
        "expansions: {}\n",
        native_plan
            .runtime_branching_calls
            .straight_line_expansions
            .len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        native_plan
            .runtime_branching_calls
            .straight_line_operations
            .len()
    ));
    output.push_str(&format!(
        "bindings: {}\n",
        native_plan
            .runtime_branching_calls
            .straight_line_bindings
            .len()
    ));
    if native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .is_empty()
    {
        output.push_str("none\n");
    } else {
        for (_, expansion) in native_plan
            .runtime_branching_calls
            .straight_line_expansions
            .iter()
        {
            let source_name = native_state_name(native_plan, expansion.source_key);
            let branch_name = native_state_name(native_plan, expansion.branch_key);
            let target_name = native_state_name(native_plan, expansion.target_key);
            output.push_str(&format!(
                "- #{} {} statement {} {} edge {} -> {} {:?} {}\n",
                expansion.dispatch_index,
                source_name,
                expansion.statement_index,
                branch_name,
                expansion.edge_order,
                target_name,
                expansion.guard_kind,
                transition_guard_name(&expansion.guard)
            ));
            if expansion.resolved_guard != expansion.guard {
                output.push_str(&format!(
                    "  resolved guard: {}\n",
                    transition_guard_name(&expansion.resolved_guard)
                ));
            }

            match native_plan
                .runtime_branching_calls
                .straight_line_bindings
                .span(expansion.bindings)
            {
                Some(bindings) if bindings.is_empty() => {
                    output.push_str("  bindings: none\n");
                }
                Some(bindings) => {
                    output.push_str("  bindings:\n");
                    for binding in bindings {
                        output.push_str(&format!(
                            "    - {:?} `{}` = `{}`\n",
                            binding.kind,
                            binding.parameter_name,
                            binding.expression.display_name()
                        ));
                    }
                }
                None => output.push_str("  bindings: invalid span\n"),
            }

            match native_plan
                .runtime_branching_calls
                .straight_line_operations
                .span(expansion.operations)
            {
                Some(operations) if operations.is_empty() => {
                    output.push_str("  operations: none\n");
                }
                Some(operations) => {
                    output.push_str("  operations:\n");
                    for operation in operations {
                        write_runtime_straight_line_branch_operation(
                            &mut output,
                            native_plan,
                            operation,
                        );
                    }
                }
                None => output.push_str("  operations: invalid span\n"),
            }
        }
    }

    object::write_layout_object_sections(&mut output, native_plan);
    output
}

fn write_runtime_leaf_branch_operation(
    output: &mut String,
    native_plan: &NativePlan,
    operation: &RuntimeLeafBranchOperation,
) {
    let source_name = native_state_name(native_plan, operation.source_key);
    match &operation.kind {
        RuntimeLeafBranchOperationKind::HostCall { platform_call } => {
            output.push_str(&format!(
                "    - {} statement {} host call `{}`\n",
                source_name, operation.statement_index, platform_call
            ));
        }
        RuntimeLeafBranchOperationKind::Mutation {
            mutation_kind,
            lowering,
            target,
            value,
        } => {
            output.push_str(&format!(
                "    - {} statement {} {:?}/{:?}: `{}` = `{}`\n",
                source_name,
                operation.statement_index,
                mutation_kind,
                lowering,
                target.display_name(),
                value.display_name()
            ));
        }
        RuntimeLeafBranchOperationKind::Other => {
            output.push_str(&format!(
                "    - {} statement {} other\n",
                source_name, operation.statement_index
            ));
        }
    }
}

fn write_runtime_straight_line_branch_operation(
    output: &mut String,
    native_plan: &NativePlan,
    operation: &RuntimeStraightLineBranchOperation,
) {
    let source_name = native_state_name(native_plan, operation.source_key);
    match &operation.kind {
        RuntimeStraightLineBranchOperationKind::HostCall { platform_call } => {
            output.push_str(&format!(
                "    - {} statement {} host call `{}`\n",
                source_name, operation.statement_index, platform_call
            ));
        }
        RuntimeStraightLineBranchOperationKind::Mutation {
            mutation_kind,
            lowering,
            target,
            value,
        } => {
            output.push_str(&format!(
                "    - {} statement {} {:?}/{:?}: `{}` = `{}`\n",
                source_name,
                operation.statement_index,
                mutation_kind,
                lowering,
                target.display_name(),
                value.display_name()
            ));
        }
        RuntimeStraightLineBranchOperationKind::StateCall {
            target_key,
            argument_count,
            lowering,
            ..
        } => {
            let target_name = native_state_name(native_plan, *target_key);
            output.push_str(&format!(
                "    - {} statement {} state call {} args {} {:?}\n",
                source_name, operation.statement_index, target_name, argument_count, lowering
            ));
        }
        RuntimeStraightLineBranchOperationKind::LocalData => {
            output.push_str(&format!(
                "    - {} statement {} local data\n",
                source_name, operation.statement_index
            ));
        }
        RuntimeStraightLineBranchOperationKind::Other => {
            output.push_str(&format!(
                "    - {} statement {} other\n",
                source_name, operation.statement_index
            ));
        }
    }
}

fn transition_guard_name(guard: &TransitionGuard) -> String {
    match guard {
        TransitionGuard::Always => "always".to_owned(),
        TransitionGuard::When(expression) => format!("when {}", expression.display_name()),
    }
}

fn runtime_transition_target_name(
    native_plan: &NativePlan,
    target: &RuntimeTransitionTarget,
) -> String {
    match target {
        RuntimeTransitionTarget::State { key } => native_state_name(native_plan, *key),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}

fn native_state_name(native_plan: &NativePlan, key: StateKey) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
