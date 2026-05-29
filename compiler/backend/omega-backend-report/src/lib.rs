mod codegen;
mod format;
mod host;
mod identity;
mod input;
mod object;
mod runtime_text;
mod state_calls;
mod stats;
mod storage;

use omega_artifacts::BackendSurfaceReport;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_control_flow::{ProofObligationOwner, StateKey};
use omega_runtime_branching::{
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};
use omega_state_calls::StateCall;
use omega_state_dispatch::state_dispatch_label;
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_schedule::{
    StateScheduleContext, build_entry_state_schedule, scheduled_state_flow,
};

use crate::host::host_call_display_name;

pub use input::{BackendReportInput, BackendReportPhaseTiming};

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
        omega_object_file::object_entry_symbol_name(backend_plan.object)
    ));

    stats::write_backend_phase_timings(&mut output, backend_plan);
    stats::write_backend_string_storage(&mut output, backend_plan);
    write_checked_semantics_section(&mut output, backend_plan);

    host::write_host_sections(&mut output, backend_plan);
    state_calls::write_state_call_sections(&mut output, backend_plan);
    storage::write_storage_sections(&mut output, backend_plan);

    runtime_text::write_runtime_text_sections(&mut output, backend_plan);

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
    let schedule_context = StateScheduleContext::new(
        &backend_plan.control_flow,
        &backend_plan.host_calls,
        &backend_plan.state_calls,
    );
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
                transition_guard_expression_name(
                    &backend_plan.control_flow.expressions,
                    edge.expressions.guard,
                )
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
                state.dispatch_index,
                machine_name,
                state_name,
                state_dispatch_label(state.key)
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
                            transition_guard_expression_name(
                                &backend_plan.control_flow.expressions,
                                edge.expressions.guard,
                            )
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
                state_dispatch_label(dispatch_case.key),
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
                            transition_guard_expression_name(
                                &backend_plan.state_guards.expressions,
                                if edge.guard_has_expression {
                                    edge.guard_expression
                                } else {
                                    ExpressionHandle::invalid()
                                },
                            )
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
                            transition_guard_expression_name(
                                &backend_plan.runtime_branching_calls.expressions,
                                edge.guard
                            )
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
                transition_guard_expression_name(
                    &backend_plan.runtime_branching_calls.expressions,
                    expansion.guard
                )
            ));
            if expansion.resolved_guard != expansion.guard {
                output.push_str(&format!(
                    "  resolved guard: {}\n",
                    transition_guard_expression_name(
                        &backend_plan.runtime_branching_calls.expressions,
                        expansion.resolved_guard
                    )
                ));
            }
            if expansion.target_value.is_valid() {
                output.push_str(&format!(
                    "  target value: {}\n",
                    backend_plan
                        .runtime_branching_calls
                        .expressions
                        .display_name(expansion.target_value)
                ));
            } else {
                output.push_str("  target value: none\n");
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
                transition_guard_expression_name(
                    &backend_plan.runtime_branching_calls.expressions,
                    expansion.guard
                )
            ));
            if expansion.resolved_guard != expansion.guard {
                output.push_str(&format!(
                    "  resolved guard: {}\n",
                    transition_guard_expression_name(
                        &backend_plan.runtime_branching_calls.expressions,
                        expansion.resolved_guard
                    )
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

fn write_checked_semantics_section(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    output.push_str("## Checked Semantics\n");
    output.push_str(&format!(
        "proof obligations: {}\n",
        backend_plan.control_flow.proof_obligations.len()
    ));
    if backend_plan.control_flow.proof_obligations.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, obligation) in backend_plan.control_flow.proof_obligations.iter() {
            output.push_str(&format!(
                "- {:?}: {}\n",
                obligation.kind,
                proof_obligation_owner_display_name(backend_plan, &obligation.owner)
            ));
        }
    }

    output.push_str(&format!(
        "invariants: {}\n",
        backend_plan.control_flow.invariants.len()
    ));
    if backend_plan.control_flow.invariants.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, invariant) in backend_plan.control_flow.invariants.iter() {
            output.push_str(&format!(
                "- `{}` constraints {}\n",
                invariant.name, invariant.constraint_count
            ));
        }
    }
    output.push('\n');
}

fn write_runtime_leaf_branch_operation(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    operation: &RuntimeLeafBranchOperation,
) {
    let source_name = backend_state_name(backend_plan, operation.source_key);
    match &operation.kind {
        RuntimeLeafBranchOperationKind::HostCall => {
            let platform_call = host_call_name_for_statement(
                backend_plan,
                operation.source_key,
                operation.statement_index,
            );
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
        RuntimeStraightLineBranchOperationKind::HostCall => {
            let platform_call = host_call_name_for_statement(
                backend_plan,
                operation.source_key,
                operation.statement_index,
            );
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
            role,
            target_key,
            argument_count,
            lowering,
            ..
        } => {
            let target_name = backend_state_name(backend_plan, *target_key);
            output.push_str(&format!(
                "    - {} statement {} {:?} state call {} args {} {:?}\n",
                source_name, operation.statement_index, role, target_name, argument_count, lowering
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

fn transition_guard_expression_name(
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> String {
    if guard.is_valid() {
        format!("when {}", expressions.display_name(guard))
    } else {
        "always".to_owned()
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
        RuntimeTransitionTarget::Unknown => "unknown".to_owned(),
    }
}

fn proof_obligation_owner_display_name(
    backend_plan: &BackendReportInput<'_>,
    owner: &ProofObligationOwner,
) -> String {
    match owner {
        ProofObligationOwner::Unknown => "unknown".to_owned(),
        ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            format!("machine `{machine_name}` state `{state_name}`")
        }
        ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
            ..
        } => {
            let machine_name = proof_machine_name(backend_plan, *machine_symbol);
            let data_name = backend_plan
                .control_flow
                .machine_owned_data_by_symbol(*machine_symbol, *data_symbol)
                .map(|data| data.name.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned());
            format!("machine `{machine_name}` owned data `{data_name}`")
        }
        ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            let parameter_name = proof_state_parameter_name(
                backend_plan,
                *machine_symbol,
                *state_symbol,
                *parameter_symbol,
            );
            format!("machine `{machine_name}` state `{state_name}` parameter `{parameter_name}`")
        }
        ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            format!("machine `{machine_name}` state `{state_name}` return")
        }
        ProofObligationOwner::CallParameter {
            machine_symbol,
            state_symbol,
            target_symbol,
            parameter_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            let target_name = proof_state_name(backend_plan, *machine_symbol, *target_symbol);
            let parameter_name = proof_state_parameter_name(
                backend_plan,
                *machine_symbol,
                *target_symbol,
                *parameter_symbol,
            );
            format!(
                "machine `{machine_name}` state `{state_name}` call `{target_name}` parameter `{parameter_name}`"
            )
        }
        ProofObligationOwner::TransitionParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
            ..
        } => {
            let (machine_name, state_name) =
                proof_state_names(backend_plan, *machine_symbol, *state_symbol);
            let parameter_name = proof_state_parameter_name(
                backend_plan,
                *machine_symbol,
                *state_symbol,
                *parameter_symbol,
            );
            format!(
                "machine `{machine_name}` state `{state_name}` transition parameter `{parameter_name}`"
            )
        }
    }
}

fn proof_machine_name(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: omega_core::symbols::SymbolHandle,
) -> String {
    backend_plan
        .control_flow
        .machine_by_symbol(machine_symbol)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn proof_state_names(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: omega_core::symbols::SymbolHandle,
    state_symbol: omega_core::symbols::SymbolHandle,
) -> (String, String) {
    backend_plan
        .control_flow
        .state_key_by_symbols(machine_symbol, state_symbol)
        .and_then(|key| backend_plan.control_flow.state_names_by_key(key))
        .map(|(machine, state)| (machine.to_string(), state.to_string()))
        .unwrap_or_else(|| ("<unknown>".to_owned(), "<unknown>".to_owned()))
}

fn proof_state_name(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: omega_core::symbols::SymbolHandle,
    state_symbol: omega_core::symbols::SymbolHandle,
) -> String {
    proof_state_names(backend_plan, machine_symbol, state_symbol).1
}

fn proof_state_parameter_name(
    backend_plan: &BackendReportInput<'_>,
    machine_symbol: omega_core::symbols::SymbolHandle,
    state_symbol: omega_core::symbols::SymbolHandle,
    parameter_symbol: omega_core::symbols::SymbolHandle,
) -> String {
    backend_plan
        .control_flow
        .state_key_by_symbols(machine_symbol, state_symbol)
        .and_then(|key| backend_plan.control_flow.state_by_key(key))
        .and_then(|state| {
            backend_plan
                .control_flow
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == parameter_symbol)
        })
        .map(|parameter| parameter.name.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn backend_state_name(backend_plan: &BackendReportInput<'_>, key: StateKey) -> String {
    backend_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}

fn backend_state_call_receiver_name(
    backend_plan: &BackendReportInput<'_>,
    call: &StateCall,
) -> String {
    backend_plan
        .control_flow
        .receiver_name_by_symbol(call.source_key, call.receiver_symbol)
        .or_else(|| {
            backend_plan
                .control_flow
                .call_receiver_name_by_statement(call.source_key, call.statement_index)
        })
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            if call.receiver_symbol.is_valid() {
                "<unknown>".to_owned()
            } else {
                "self".to_owned()
            }
        })
}

fn host_call_name_for_statement<'plan>(
    backend_plan: &'plan BackendReportInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> String {
    backend_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            state_key_matches_statement_source(host_call.source_key, source_key)
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call_display_name(backend_plan, host_call))
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}
