use crate::identity::NativeStringStorage;
use crate::identity::expressions::{
    count_expression_span_strings, count_expression_strings, count_guard_strings,
};
use crate::identity::targets::count_runtime_target_strings;
use omega_native::plan::NativePlan;
use omega_runtime_branching::{
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperationKind,
};

pub(in crate::identity) fn count_runtime_branching_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, edge) in native_plan.runtime_branching_calls.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
        count_guard_strings(&edge.guard, storage);
        count_expression_span_strings(edge.target_arguments, native_plan, storage);
    }
    for (_, expansion) in native_plan.runtime_branching_calls.leaf_expansions.iter() {
        count_guard_strings(&expansion.guard, storage);
        count_guard_strings(&expansion.resolved_guard, storage);
    }
    for (_, binding) in native_plan.runtime_branching_calls.leaf_bindings.iter() {
        storage.count_program_name_identity(&binding.parameter_name);
        count_expression_strings(&binding.expression, storage);
    }
    for (_, operation) in native_plan.runtime_branching_calls.leaf_operations.iter() {
        match &operation.kind {
            RuntimeLeafBranchOperationKind::HostCall { platform_call } => {
                storage.count_identity(platform_call)
            }
            RuntimeLeafBranchOperationKind::Mutation { target, value, .. } => {
                count_expression_strings(target, storage);
                count_expression_strings(value, storage);
            }
            RuntimeLeafBranchOperationKind::Other => {}
        }
    }
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
    {
        count_guard_strings(&expansion.guard, storage);
        count_guard_strings(&expansion.resolved_guard, storage);
    }
    for (_, binding) in native_plan
        .runtime_branching_calls
        .straight_line_bindings
        .iter()
    {
        storage.count_program_name_identity(&binding.parameter_name);
        count_expression_strings(&binding.expression, storage);
    }
    for (_, operation) in native_plan
        .runtime_branching_calls
        .straight_line_operations
        .iter()
    {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::HostCall { platform_call } => {
                storage.count_identity(platform_call)
            }
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                count_expression_strings(target, storage);
                count_expression_strings(value, storage);
            }
            RuntimeStraightLineBranchOperationKind::StateCall { .. } => {}
            RuntimeStraightLineBranchOperationKind::LocalData
            | RuntimeStraightLineBranchOperationKind::Other => {}
        }
    }
}
