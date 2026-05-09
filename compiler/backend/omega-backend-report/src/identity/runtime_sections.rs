use crate::BackendReportInput;
use crate::identity::NativeStringStorage;
use crate::identity::expressions::count_expression_strings;
use crate::identity::targets::count_runtime_target_strings;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_typed_program::statement::TransitionGuard;

pub(in crate::identity) fn count_runtime_flow_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut NativeStringStorage,
) {
    for (_, edge) in backend_plan.runtime_flow.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
}

pub(in crate::identity) fn count_state_dispatch_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut NativeStringStorage,
) {
    for (_, state) in backend_plan.state_dispatch.states.iter() {
        storage.count_generated_symbol(&state.label);
    }
    for (_, edge) in backend_plan.state_dispatch.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
}

pub(in crate::identity) fn count_runtime_body_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut NativeStringStorage,
) {
    for (_, operation) in backend_plan.runtime_bodies.operations.iter() {
        match &operation.kind {
            RuntimeDispatchBodyOperationKind::HostCall { platform_call } => {
                storage.count_identity(platform_call);
            }
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
            | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
            | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
            RuntimeDispatchBodyOperationKind::LocalStorage {
                name, type_name, ..
            } => {
                storage.count_program_name_identity(name);
                storage.count_identity(type_name);
            }
            RuntimeDispatchBodyOperationKind::Mutation { .. }
            | RuntimeDispatchBodyOperationKind::Other => {}
        }
    }
}

pub(in crate::identity) fn count_state_guard_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut NativeStringStorage,
) {
    for (_, guard) in backend_plan.state_guards.guards.iter() {
        count_runtime_target_strings(&guard.target, storage);
        count_runtime_target_strings(&guard.continuation, storage);
        count_expression_strings(&guard.expression, storage);
    }
    for (_, operand) in backend_plan.state_guards.operands.iter() {
        count_expression_strings(&operand.expression, storage);
    }
}

pub(in crate::identity) fn count_runtime_dispatch_loop_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut NativeStringStorage,
) {
    storage.count_generated_symbol(&backend_plan.runtime_dispatch_loop.current_state_slot);
    storage.count_generated_symbol(&backend_plan.runtime_dispatch_loop.next_state_slot);
    for (_, dispatch_case) in backend_plan.runtime_dispatch_loop.cases.iter() {
        storage.count_generated_symbol(&dispatch_case.label);
    }
    for (_, edge) in backend_plan.runtime_dispatch_loop.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
        if let TransitionGuard::When(expression) = &edge.guard {
            count_expression_strings(&expression, storage);
        }
    }
}
