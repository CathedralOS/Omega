mod codegen;
mod format;
mod host;
mod identity;
mod object;
mod stats;

use omega_artifacts::BackendSurfaceReport;
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::allocations::AllocationDelta;
use omega_layout::LayoutPlan;
use omega_machine_program::{EncodedMachinePlan, MachineCodePlan};
use omega_object::{ObjectPlan, RelocationPlan};
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::{
    RuntimeBranchingCallPlan, RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_dispatch::StateDispatchPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::StateGuardPlan;
use omega_state_schedule::{
    StateScheduleContext, build_entry_state_schedule, scheduled_state_flow,
};
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::NativeTarget;
use omega_target_program::{InstructionPlan, TargetDataPlan};
use omega_typed_program::statement::TransitionGuard;

pub struct BackendReportPhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}

pub struct BackendReportInput<'plan> {
    pub target: NativeTarget,
    pub entry_key: StateKey,
    pub phase_timings: &'plan [BackendReportPhaseTiming],
    pub host_abi: &'plan HostAbiPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    pub alias_flow: &'plan AliasFlowPlan,
    pub state_storage: &'plan StateStoragePlan,
    pub state_values: &'plan StateValuePlan,
    pub data: &'plan TargetDataPlan,
    pub instructions: &'plan InstructionPlan,
    pub control_flow: &'plan ControlFlowPlan,
    pub runtime_flow: &'plan RuntimeFlowPlan,
    pub state_dispatch: &'plan StateDispatchPlan,
    pub state_guards: &'plan StateGuardPlan,
    pub runtime_bodies: &'plan RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: &'plan RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: &'plan RuntimeDispatchLoopPlan,
    pub runtime_storage: &'plan RuntimeStoragePlan,
    pub runtime_text: &'plan RuntimeTextPlan,
    pub layouts: &'plan LayoutPlan,
    pub machine_code: &'plan MachineCodePlan,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub object: &'plan ObjectPlan,
    pub relocations: &'plan RelocationPlan,
}

impl<'plan> BackendReportInput<'plan> {
    pub fn entry_machine_name(&self) -> &str {
        self.control_flow
            .machine_by_symbol(self.entry_key.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("")
    }

    pub fn entry_state_name(&self) -> &str {
        self.control_flow
            .state_by_key(self.entry_key)
            .map(|state| state.name.as_str())
            .unwrap_or("")
    }
}

pub fn backend_report_text(
    backend_surface: &BackendSurfaceReport,
    backend_plan: &BackendReportInput<'_>,
) -> String {
    let mut output = String::new();

    output.push_str("# Omega Backend Plan\n\n");
    output.push_str(&format!("target: {:?}\n", backend_plan.target));
    output.push_str(&format!(
        "entry: {}.{} as `{}`\n\n",
        backend_plan.entry_machine_name(),
        backend_plan.entry_state_name(),
        backend_plan.object.entry_symbol
    ));

    stats::write_backend_phase_timings(&mut output, backend_plan);
    stats::write_backend_string_storage(&mut output, backend_plan);

    host::write_host_sections(&mut output, backend_plan);

    output.push_str("## State Call Lowering\n");
    output.push_str(&format!(
        "calls: {}\n",
        backend_plan.state_calls.calls.len()
    ));
    if backend_plan.state_calls.calls.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, state_call) in backend_plan.state_calls.calls.iter() {
            let source_name = backend_state_name(backend_plan, state_call.source_key);
            let target_name = if state_call.target_key.is_valid() {
                backend_state_name(backend_plan, state_call.target_key)
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

            match backend_plan
                .state_calls
                .arguments
                .span(state_call.arguments)
            {
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
                            backend_plan
                                .state_calls
                                .expressions
                                .display_name(argument.expression),
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
        backend_plan.alias_flow.aliases.len()
    ));
    if backend_plan.alias_flow.aliases.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, alias) in backend_plan.alias_flow.aliases.iter() {
            let caller_name = backend_state_name(backend_plan, alias.caller_key);
            let callee_name = backend_state_name(backend_plan, alias.callee_key);
            output.push_str(&format!(
                "- {} statement {} -> {} `{}` aliases `{}` required {}\n",
                caller_name,
                alias.statement_index,
                callee_name,
                alias.parameter_name,
                backend_plan
                    .alias_flow
                    .expressions
                    .display_name(alias.argument),
                alias.required
            ));
        }
    }
    output.push('\n');

    output.push_str("## State Storage\n");
    output.push_str(&format!(
        "locals: {}\n",
        backend_plan.state_storage.locals.len()
    ));
    for (_, local) in backend_plan.state_storage.locals.iter() {
        let source_name = backend_state_name(backend_plan, local.source_key);
        output.push_str(&format!(
            "- {} statement {} local `{}`: {} required {}\n",
            source_name, local.statement_index, local.name, local.type_name, local.required
        ));
    }
    output.push_str(&format!(
        "mutations: {}\n",
        backend_plan.state_storage.mutations.len()
    ));
    for (_, mutation) in backend_plan.state_storage.mutations.iter() {
        let source_name = backend_state_name(backend_plan, mutation.source_key);
        output.push_str(&format!(
            "- {} statement {} {:?}/{:?}: `{}` = `{}` required {}\n",
            source_name,
            mutation.statement_index,
            mutation.mutation_kind,
            mutation.lowering,
            backend_plan
                .state_storage
                .expressions
                .display_name(mutation.target),
            backend_plan
                .state_storage
                .expressions
                .display_name(mutation.value),
            mutation.required
        ));
    }
    output.push('\n');

    output.push_str("## Runtime Storage\n");
    output.push_str(&format!(
        "frame slots: {}\n",
        backend_plan.runtime_storage.frame_slots.len()
    ));
    for (_, slot) in backend_plan.runtime_storage.frame_slots.iter() {
        let source_name = backend_state_name(backend_plan, slot.source_key);
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
        backend_plan.runtime_storage.writes.len()
    ));
    for (_, write) in backend_plan.runtime_storage.writes.iter() {
        let source_name = backend_state_name(backend_plan, write.source_key);
        output.push_str(&format!(
            "- #{} {} statement {} {:?}/{:?}: `{}` = `{}`\n",
            write.dispatch_index,
            source_name,
            write.statement_index,
            write.mutation_kind,
            write.lowering,
            backend_plan
                .runtime_storage
                .expressions
                .display_name(write.target),
            backend_plan
                .runtime_storage
                .expressions
                .display_name(write.value)
        ));
    }
    output.push('\n');

    output.push_str("## State Values\n");
    output.push_str(&format!(
        "values: {}\n",
        backend_plan.state_values.values.len()
    ));
    for (_, value) in backend_plan.state_values.values.iter() {
        let source_name = backend_state_name(backend_plan, value.source_key);
        output.push_str(&format!(
            "- {} statement {} {:?}/{:?}: `{}` required {}\n",
            source_name,
            value.statement_index,
            value.role,
            value.kind,
            backend_plan
                .state_values
                .expressions
                .display_name(value.expression),
            value.required
        ));
    }
    output.push('\n');

    output.push_str("## Runtime Text\n");
    output.push_str(&format!("uses: {}\n", backend_plan.runtime_text.uses.len()));
    output.push_str(&format!(
        "buffers: {}\n",
        backend_plan.runtime_text.buffers.len()
    ));
    output.push_str(&format!(
        "slots: {}\n",
        backend_plan.runtime_text.slots.len()
    ));
    output.push_str(&format!(
        "writes: {}\n",
        backend_plan.runtime_text.writes.len()
    ));
    output.push_str(&format!(
        "builders: {}\n",
        backend_plan.runtime_text.builders.len()
    ));
    output.push_str(&format!(
        "builder segments: {}\n",
        backend_plan.runtime_text.builder_segments.len()
    ));
    if backend_plan.runtime_text.uses.is_empty() {
        output.push_str("uses: none\n");
    } else {
        for (_, text_use) in backend_plan.runtime_text.uses.iter() {
            let source_name = backend_state_name(backend_plan, text_use.source_key);
            output.push_str(&format!(
                "- {} statement {} `{}` {:?} newline {}\n",
                source_name,
                text_use.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_use.expression),
                text_use.source,
                text_use.append_newline
            ));
        }
    }
    if backend_plan.runtime_text.buffers.is_empty() {
        output.push_str("buffers: none\n");
    } else {
        for (_, text_buffer) in backend_plan.runtime_text.buffers.iter() {
            let source_name = backend_state_name(backend_plan, text_buffer.source_key);
            output.push_str(&format!(
                "- buffer {} statement {} `{}` bytes {}\n",
                source_name,
                text_buffer.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_buffer.target),
                text_buffer.byte_capacity
            ));
        }
    }
    if backend_plan.runtime_text.slots.is_empty() {
        output.push_str("slots: none\n");
    } else {
        for (_, text_slot) in backend_plan.runtime_text.slots.iter() {
            output.push_str(&format!(
                "- slot `{}` bytes {} input_buffer {}\n",
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_slot.place),
                text_slot.byte_capacity,
                text_slot.has_input_buffer
            ));
        }
    }
    if backend_plan.runtime_text.writes.is_empty() {
        output.push_str("writes: none\n");
    } else {
        for (_, text_write) in backend_plan.runtime_text.writes.iter() {
            let source_name = backend_state_name(backend_plan, text_write.source_key);
            output.push_str(&format!(
                "- write {} statement {} `{}` = `{}` {:?}\n",
                source_name,
                text_write.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_write.target),
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_write.value),
                text_write.kind
            ));
        }
    }
    if backend_plan.runtime_text.builders.is_empty() {
        output.push_str("builders: none\n");
    } else {
        for (_, text_builder) in backend_plan.runtime_text.builders.iter() {
            let source_name = backend_state_name(backend_plan, text_builder.source_key);
            output.push_str(&format!(
                "- builder {} statement {} `{}` segments {}\n",
                source_name,
                text_builder.statement_index,
                backend_plan
                    .runtime_text
                    .expressions
                    .display_name(text_builder.target),
                text_builder.segments.count()
            ));
            if let Some(segments) = backend_plan
                .runtime_text
                .builder_segments
                .span(text_builder.segments)
            {
                for segment in segments {
                    output.push_str(&format!(
                        "  - segment `{}` {:?}\n",
                        backend_plan
                            .runtime_text
                            .expressions
                            .display_name(segment.expression),
                        segment.kind
                    ));
                }
            }
        }
    }
    output.push('\n');

    codegen::write_codegen_sections(&mut output, backend_plan);

    output.push_str("## Source Native Surface\n");
    output.push_str(&format!(
        "entry candidates: {}\n",
        backend_surface.entry_points.len()
    ));
    for (_, entry_point) in backend_surface.entry_points.iter() {
        output.push_str(&format!(
            "- entry {}.{}\n",
            entry_point.machine, entry_point.state
        ));
    }

    output.push_str(&format!("platforms: {}\n", backend_surface.platforms.len()));
    for (_, platform) in backend_surface.platforms.iter() {
        output.push_str(&format!(
            "- platform {}: {} state(s)\n",
            platform.name, platform.states
        ));
    }

    output.push_str(&format!("machines: {}\n", backend_surface.machines.len()));
    for (_, machine) in backend_surface.machines.iter() {
        output.push_str(&format!(
            "- machine {}: contains {}, owned data {}, states {}\n",
            machine.name, machine.contained_objects, machine.owned_data, machine.states
        ));
    }
    output.push('\n');

    output.push_str("## State Schedule\n");
    let schedule_context =
        StateScheduleContext::new(&backend_plan.control_flow, &backend_plan.host_calls);
    match build_entry_state_schedule(&schedule_context, backend_plan.entry_key) {
        Ok(schedule) if schedule.is_empty() => output.push_str("states: 0\nnone\n"),
        Ok(schedule) => {
            output.push_str(&format!("states: {}\n", schedule.len()));
            for scheduled_state in schedule {
                if let Some(state_flow) = scheduled_state_flow(&schedule_context, &scheduled_state)
                {
                    output.push_str(&format!(
                        "- {}.{}#{}\n",
                        backend_plan
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
        backend_plan.runtime_flow.states.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        backend_plan.runtime_flow.edges.len()
    ));
    output.push_str(&format!(
        "cycles: {}\n",
        backend_plan.runtime_flow.cycles.len()
    ));
    if backend_plan.runtime_flow.states.is_empty() {
        output.push_str("none\n");
    } else {
        output.push_str("states:\n");
        for (_, state) in backend_plan.runtime_flow.states.iter() {
            output.push_str(&format!(
                "- {}\n",
                backend_state_name(backend_plan, state.key)
            ));
        }
    }
    if !backend_plan.runtime_flow.edges.is_empty() {
        output.push_str("edges:\n");
        for (_, edge) in backend_plan.runtime_flow.edges.iter() {
            output.push_str(&format!(
                "- {} -> {} {}",
                backend_state_name(backend_plan, edge.from),
                runtime_transition_target_name(backend_plan, &edge.target),
                transition_guard_name(&edge.guard)
            ));

            if edge.continuation != RuntimeTransitionTarget::None {
                output.push_str(&format!(
                    " -> {}",
                    runtime_transition_target_name(backend_plan, &edge.continuation)
                ));
            }

            if edge.forms_cycle {
                output.push_str(" [cycle]");
            }

            output.push('\n');
        }
    }
    if !backend_plan.runtime_flow.cycles.is_empty() {
        output.push_str("cycle paths:\n");
        for (_, cycle) in backend_plan.runtime_flow.cycles.iter() {
            match backend_plan.runtime_flow.cycle_states.span(cycle.states) {
                Some(states) => {
                    let path = states
                        .iter()
                        .map(|state| backend_state_name(backend_plan, state.key))
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
        backend_plan.state_dispatch.states.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        backend_plan.state_dispatch.edges.len()
    ));
    if backend_plan.state_dispatch.states.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, state) in backend_plan.state_dispatch.states.iter() {
            let machine_name = backend_plan
                .control_flow
                .machine_by_symbol(state.key.machine)
                .map(|machine| machine.name.as_str())
                .unwrap_or("<unknown>");
            let state_name = backend_plan
                .control_flow
                .state_by_key(state.key)
                .map(|state| state.name.as_str())
                .unwrap_or("<unknown>");
            output.push_str(&format!(
                "- #{} {}.{} label `{}`\n",
                state.dispatch_index, machine_name, state_name, state.label
            ));

            match backend_plan.state_dispatch.edges.span(state.edges) {
                Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                Some(edges) => {
                    output.push_str("  edges:\n");
                    for edge in edges {
                        output.push_str(&format!(
                            "    - -> #{} {} {}",
                            edge.target_dispatch_index,
                            runtime_transition_target_name(backend_plan, &edge.target),
                            transition_guard_name(&edge.guard)
                        ));

                        if edge.continuation != RuntimeTransitionTarget::None {
                            output.push_str(&format!(
                                " -> #{} {}",
                                edge.continuation_dispatch_index,
                                runtime_transition_target_name(backend_plan, &edge.continuation)
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
        backend_plan.state_guards.guards.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        backend_plan.state_guards.operands.len()
    ));
    if backend_plan.state_guards.guards.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, guard) in backend_plan.state_guards.guards.iter() {
            let machine_name = backend_plan
                .control_flow
                .machine_by_symbol(guard.source.machine)
                .map(|machine| machine.name.as_str())
                .unwrap_or("<unknown>");
            let state_name = backend_plan
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
                runtime_transition_target_name(backend_plan, &guard.target),
                guard.kind,
                guard.operator,
                guard.lowering
            ));

            if guard.has_expression {
                output.push_str(&format!(
                    " `{}`",
                    backend_plan
                        .state_guards
                        .expressions
                        .display_name(guard.expression)
                ));
            }

            if guard.continuation != RuntimeTransitionTarget::None {
                output.push_str(&format!(
                    " -> #{} {}",
                    guard.continuation_dispatch_index,
                    runtime_transition_target_name(backend_plan, &guard.continuation)
                ));
            }

            if guard.forms_cycle {
                output.push_str(" [cycle]");
            }

            output.push('\n');
            if let Some(operands) = backend_plan.state_guards.operands.span(guard.operands)
                && !operands.is_empty()
            {
                for operand in operands {
                    output.push_str(&format!(
                        "  - operand `{}` {:?} {:?} offset {} bytes {}\n",
                        backend_plan
                            .state_guards
                            .expressions
                            .display_name(operand.expression),
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
        backend_plan.runtime_dispatch_loop.needed
    ));
    output.push_str(&format!(
        "entry dispatch index: #{}\n",
        backend_plan.runtime_dispatch_loop.entry_dispatch_index
    ));
    output.push_str(&format!(
        "terminal dispatch index: #{}\n",
        backend_plan.runtime_dispatch_loop.terminal_dispatch_index
    ));
    output.push_str(&format!(
        "current state slot: `{}`\n",
        backend_plan.runtime_dispatch_loop.current_state_slot
    ));
    output.push_str(&format!(
        "next state slot: `{}`\n",
        backend_plan.runtime_dispatch_loop.next_state_slot
    ));
    output.push_str(&format!(
        "cases: {}\n",
        backend_plan.runtime_dispatch_loop.cases.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        backend_plan.runtime_dispatch_loop.edges.len()
    ));
    if backend_plan.runtime_dispatch_loop.cases.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, dispatch_case) in backend_plan.runtime_dispatch_loop.cases.iter() {
            let machine_name = backend_plan
                .control_flow
                .machine_by_symbol(dispatch_case.key.machine)
                .map(|machine| machine.name.as_str())
                .unwrap_or("<unknown>");
            let state_name = backend_plan
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

            match backend_plan
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
                            runtime_transition_target_name(backend_plan, &edge.target),
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
                                runtime_transition_target_name(backend_plan, &edge.continuation)
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
        backend_plan.runtime_bodies.bodies.len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        backend_plan.runtime_bodies.operations.len()
    ));
    if backend_plan.runtime_bodies.bodies.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, body) in backend_plan.runtime_bodies.bodies.iter() {
            let source_name = backend_state_name(backend_plan, body.key);
            output.push_str(&format!("- #{} {}\n", body.dispatch_index, source_name));

            match backend_plan.runtime_bodies.operations.span(body.operations) {
                Some(operations) if operations.is_empty() => {
                    output.push_str("  operations: none\n");
                }
                Some(operations) => {
                    output.push_str("  operations:\n");
                    for operation in operations {
                        let source_name = backend_state_name(backend_plan, operation.source_key);
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
        backend_plan.runtime_branching_calls.calls.len()
    ));
    output.push_str(&format!(
        "edges: {}\n",
        backend_plan.runtime_branching_calls.edges.len()
    ));
    if backend_plan.runtime_branching_calls.calls.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, call) in backend_plan.runtime_branching_calls.calls.iter() {
            let source_name = backend_state_name(backend_plan, call.source_key);
            let target_name = backend_state_name(backend_plan, call.target_key);
            output.push_str(&format!(
                "- #{} {} statement {} -> {} args {}\n",
                call.dispatch_index,
                source_name,
                call.statement_index,
                target_name,
                call.argument_count
            ));

            match backend_plan.runtime_branching_calls.edges.span(call.edges) {
                Some(edges) if edges.is_empty() => output.push_str("  edges: none\n"),
                Some(edges) => {
                    output.push_str(&format!("  expansion: {:?}\n", call.expansion));
                    output.push_str("  edges:\n");
                    for edge in edges {
                        output.push_str(&format!(
                            "    - #{} -> {} {:?} {:?} {}",
                            edge.order,
                            runtime_transition_target_name(backend_plan, &edge.target),
                            edge.lowering,
                            edge.guard_kind,
                            transition_guard_name(&edge.guard)
                        ));

                        let target_arguments = backend_plan
                            .runtime_branching_calls
                            .target_arguments
                            .span_or_empty(edge.target_arguments);
                        if !target_arguments.is_empty() {
                            output.push_str(&format!(
                                " args ({})",
                                target_arguments
                                    .iter()
                                    .map(|argument| backend_plan
                                        .runtime_branching_calls
                                        .expressions
                                        .display_name(*argument))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                        }

                        if edge.continuation != RuntimeTransitionTarget::None {
                            output.push_str(&format!(
                                " -> {}",
                                runtime_transition_target_name(backend_plan, &edge.continuation)
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
        backend_plan.runtime_branching_calls.leaf_expansions.len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        backend_plan.runtime_branching_calls.leaf_operations.len()
    ));
    output.push_str(&format!(
        "bindings: {}\n",
        backend_plan.runtime_branching_calls.leaf_bindings.len()
    ));
    if backend_plan
        .runtime_branching_calls
        .leaf_expansions
        .is_empty()
    {
        output.push_str("none\n");
    } else {
        for (_, expansion) in backend_plan.runtime_branching_calls.leaf_expansions.iter() {
            let source_name = backend_state_name(backend_plan, expansion.source_key);
            let branch_name = backend_state_name(backend_plan, expansion.branch_key);
            let leaf_name = backend_state_name(backend_plan, expansion.leaf_key);
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

            match backend_plan
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
                            backend_plan
                                .runtime_branching_calls
                                .expressions
                                .display_name(binding.expression)
                        ));
                    }
                }
                None => output.push_str("  bindings: invalid span\n"),
            }

            match backend_plan
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
                        write_runtime_leaf_branch_operation(&mut output, backend_plan, operation);
                    }
                }
                None => output.push_str("  operations: invalid span\n"),
            }
        }
    }

    output.push_str("\n## Runtime Straight-Line Branch Expansions\n");
    output.push_str(&format!(
        "expansions: {}\n",
        backend_plan
            .runtime_branching_calls
            .straight_line_expansions
            .len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        backend_plan
            .runtime_branching_calls
            .straight_line_operations
            .len()
    ));
    output.push_str(&format!(
        "bindings: {}\n",
        backend_plan
            .runtime_branching_calls
            .straight_line_bindings
            .len()
    ));
    if backend_plan
        .runtime_branching_calls
        .straight_line_expansions
        .is_empty()
    {
        output.push_str("none\n");
    } else {
        for (_, expansion) in backend_plan
            .runtime_branching_calls
            .straight_line_expansions
            .iter()
        {
            let source_name = backend_state_name(backend_plan, expansion.source_key);
            let branch_name = backend_state_name(backend_plan, expansion.branch_key);
            let target_name = backend_state_name(backend_plan, expansion.target_key);
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

            match backend_plan
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
                            backend_plan
                                .runtime_branching_calls
                                .expressions
                                .display_name(binding.expression)
                        ));
                    }
                }
                None => output.push_str("  bindings: invalid span\n"),
            }

            match backend_plan
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
                            backend_plan,
                            operation,
                        );
                    }
                }
                None => output.push_str("  operations: invalid span\n"),
            }
        }
    }

    object::write_layout_object_sections(&mut output, backend_plan);
    output
}

fn write_runtime_leaf_branch_operation(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    operation: &RuntimeLeafBranchOperation,
) {
    let source_name = backend_state_name(backend_plan, operation.source_key);
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
                backend_plan
                    .runtime_branching_calls
                    .expressions
                    .display_name(*target),
                backend_plan
                    .runtime_branching_calls
                    .expressions
                    .display_name(*value)
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
    backend_plan: &BackendReportInput<'_>,
    operation: &RuntimeStraightLineBranchOperation,
) {
    let source_name = backend_state_name(backend_plan, operation.source_key);
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
                backend_plan
                    .runtime_branching_calls
                    .expressions
                    .display_name(*target),
                backend_plan
                    .runtime_branching_calls
                    .expressions
                    .display_name(*value)
            ));
        }
        RuntimeStraightLineBranchOperationKind::StateCall {
            target_key,
            argument_count,
            lowering,
            ..
        } => {
            let target_name = backend_state_name(backend_plan, *target_key);
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
    backend_plan: &BackendReportInput<'_>,
    target: &RuntimeTransitionTarget,
) -> String {
    match target {
        RuntimeTransitionTarget::State { key } => backend_state_name(backend_plan, *key),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}

fn backend_state_name(backend_plan: &BackendReportInput<'_>, key: StateKey) -> String {
    backend_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
