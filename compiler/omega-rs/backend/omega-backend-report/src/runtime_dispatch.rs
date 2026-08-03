use crate::{
    BackendReportInput, backend_state_name, runtime_transition_target_name,
    transition_guard_expression_name,
};
use omega_state_dispatch::state_dispatch_label;
use omega_state_graph::RuntimeTransitionTarget;
use psi_checked_trees::expression::ExpressionHandle;

pub(super) fn write_runtime_dispatch_sections(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    write_runtime_guards(output, backend_plan);
    write_runtime_dispatch_loop(output, backend_plan);
}

fn write_runtime_guards(output: &mut String, backend_plan: &BackendReportInput<'_>) {
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
        return;
    }

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
                    output.push_str(&format!("    resolved value: {}\n", operand.resolved_value));
                }
            }
        }
    }
}

fn write_runtime_dispatch_loop(output: &mut String, backend_plan: &BackendReportInput<'_>) {
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
        return;
    }

    for (_, dispatch_case) in backend_plan.runtime_dispatch_loop.cases.iter() {
        let source_name = backend_state_name(backend_plan, dispatch_case.key);
        output.push_str(&format!(
            "- #{} {} label `{}` operations {}\n",
            dispatch_case.dispatch_index,
            source_name,
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
                            edge.guard_byte_offset, edge.guard_byte_size, edge.guard_expected_value
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
