use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::{
    count_control_flow_expression_strings, count_expression_span_strings,
};
use omega_runtime_branching::{
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperationKind,
};

pub(in crate::identity) fn count_runtime_branching_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, edge) in backend_plan.runtime_branching_calls.edges.iter() {
        if edge.guard.is_valid() {
            count_control_flow_expression_strings(
                &backend_plan.runtime_branching_calls.expressions,
                edge.guard,
                storage,
            );
        }
        count_expression_span_strings(edge.target_arguments, backend_plan, storage);
    }
    for (_, expansion) in backend_plan.runtime_branching_calls.leaf_expansions.iter() {
        if expansion.guard.is_valid() {
            count_control_flow_expression_strings(
                &backend_plan.runtime_branching_calls.expressions,
                expansion.guard,
                storage,
            );
        }
        if expansion.resolved_guard.is_valid() {
            count_control_flow_expression_strings(
                &backend_plan.runtime_branching_calls.expressions,
                expansion.resolved_guard,
                storage,
            );
        }
    }
    for (_, binding) in backend_plan.runtime_branching_calls.leaf_bindings.iter() {
        storage.count_program_name_identity(&binding.parameter_name);
        count_control_flow_expression_strings(
            &backend_plan.runtime_branching_calls.expressions,
            binding.expression,
            storage,
        );
    }
    for (_, operation) in backend_plan.runtime_branching_calls.leaf_operations.iter() {
        match &operation.kind {
            RuntimeLeafBranchOperationKind::HostCall => {}
            RuntimeLeafBranchOperationKind::Mutation { target, value, .. } => {
                count_control_flow_expression_strings(
                    &backend_plan.runtime_branching_calls.expressions,
                    *target,
                    storage,
                );
                count_control_flow_expression_strings(
                    &backend_plan.runtime_branching_calls.expressions,
                    *value,
                    storage,
                );
            }
            RuntimeLeafBranchOperationKind::Other => {}
        }
    }
    for (_, expansion) in backend_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
    {
        if expansion.guard.is_valid() {
            count_control_flow_expression_strings(
                &backend_plan.runtime_branching_calls.expressions,
                expansion.guard,
                storage,
            );
        }
        if expansion.resolved_guard.is_valid() {
            count_control_flow_expression_strings(
                &backend_plan.runtime_branching_calls.expressions,
                expansion.resolved_guard,
                storage,
            );
        }
    }
    for (_, binding) in backend_plan
        .runtime_branching_calls
        .straight_line_bindings
        .iter()
    {
        storage.count_program_name_identity(&binding.parameter_name);
        count_control_flow_expression_strings(
            &backend_plan.runtime_branching_calls.expressions,
            binding.expression,
            storage,
        );
    }
    for (_, operation) in backend_plan
        .runtime_branching_calls
        .straight_line_operations
        .iter()
    {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::HostCall => {}
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                count_control_flow_expression_strings(
                    &backend_plan.runtime_branching_calls.expressions,
                    *target,
                    storage,
                );
                count_control_flow_expression_strings(
                    &backend_plan.runtime_branching_calls.expressions,
                    *value,
                    storage,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall { .. } => {}
            RuntimeStraightLineBranchOperationKind::LocalData
            | RuntimeStraightLineBranchOperationKind::Other => {}
        }
    }
}
