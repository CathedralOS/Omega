use crate::{
    BackendReportInput, backend_state_name, host_call_name_for_statement,
    runtime_transition_target_name, transition_guard_expression_name,
};
use omega_runtime_branching::{
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};
use omega_state_graph::RuntimeTransitionTarget;

pub(super) fn write_runtime_branching_sections(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    write_runtime_branching_calls(output, backend_plan);
    write_runtime_leaf_branch_expansions(output, backend_plan);
    write_runtime_straight_line_branch_expansions(output, backend_plan);
}

fn write_runtime_branching_calls(output: &mut String, backend_plan: &BackendReportInput<'_>) {
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
        return;
    }

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

fn write_runtime_leaf_branch_expansions(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
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
        return;
    }

    for (_, expansion) in backend_plan.runtime_branching_calls.leaf_expansions.iter() {
        let source_name = backend_state_name(backend_plan, expansion.source_key);
        let branch_name = backend_state_name(backend_plan, expansion.branch_key);
        let leaf_name = backend_state_name(backend_plan, expansion.leaf_key);
        output.push_str(&format!(
            "- #{} {} statement {} {} edge {} -> {} {:?} {} call #{}\n",
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
            ),
            expansion.call_ordinal
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

        write_leaf_expansion_bindings(output, backend_plan, expansion.bindings);

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
                    write_runtime_leaf_branch_operation(output, backend_plan, operation);
                }
            }
            None => output.push_str("  operations: invalid span\n"),
        }
    }
}

fn write_runtime_straight_line_branch_expansions(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
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
        return;
    }

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

        write_straight_line_expansion_bindings(output, backend_plan, expansion.bindings);

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
                    write_runtime_straight_line_branch_operation(output, backend_plan, operation);
                }
            }
            None => output.push_str("  operations: invalid span\n"),
        }
    }
}

fn write_leaf_expansion_bindings(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    bindings: psi_arena::HandleSpan<omega_runtime_branching::RuntimeLeafBranchBinding>,
) {
    match backend_plan
        .runtime_branching_calls
        .leaf_bindings
        .span(bindings)
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
}

fn write_straight_line_expansion_bindings(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    bindings: psi_arena::HandleSpan<omega_runtime_branching::RuntimeStraightLineBranchBinding>,
) {
    match backend_plan
        .runtime_branching_calls
        .straight_line_bindings
        .span(bindings)
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
