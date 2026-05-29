use super::{backend_state_name, runtime_transition_target_name, transition_guard_expression_name};

use crate::BackendReportInput;
use omega_state_dispatch::state_dispatch_label;
use omega_state_graph::RuntimeTransitionTarget;

pub(super) fn write_runtime_flow_sections(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
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
}
