use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_control_flow_expression_strings;
use crate::identity::targets::count_runtime_target_strings;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;

pub(in crate::identity) fn count_runtime_flow_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, edge) in backend_plan.runtime_flow.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
}

pub(in crate::identity) fn count_state_dispatch_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, edge) in backend_plan.state_dispatch.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
}

pub(in crate::identity) fn count_runtime_body_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, operation) in backend_plan.runtime_bodies.operations.iter() {
        match &operation.kind {
            RuntimeDispatchBodyOperationKind::HostCall => {}
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
            | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
            | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
            RuntimeDispatchBodyOperationKind::LocalStorage {
                name,
                type_reference,
                ..
            } => {
                storage.count_program_name_identity(name);
                storage.count_identity(
                    &backend_plan
                        .runtime_bodies
                        .type_references
                        .display_name(*type_reference),
                );
            }
            RuntimeDispatchBodyOperationKind::Mutation { .. }
            | RuntimeDispatchBodyOperationKind::StateCallResult { .. }
            | RuntimeDispatchBodyOperationKind::Other => {}
        }
    }
}

pub(in crate::identity) fn count_state_guard_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, guard) in backend_plan.state_guards.guards.iter() {
        count_runtime_target_strings(&guard.target, storage);
        count_runtime_target_strings(&guard.continuation, storage);
        if guard.has_expression {
            count_control_flow_expression_strings(
                &backend_plan.state_guards.expressions,
                guard.expression,
                storage,
            );
        }
    }
    for (_, operand) in backend_plan.state_guards.operands.iter() {
        count_control_flow_expression_strings(
            &backend_plan.state_guards.expressions,
            operand.expression,
            storage,
        );
    }
}

pub(in crate::identity) fn count_runtime_dispatch_loop_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, edge) in backend_plan.runtime_dispatch_loop.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
        if edge.guard_has_expression {
            count_control_flow_expression_strings(
                &backend_plan.state_guards.expressions,
                edge.guard_expression,
                storage,
            );
        }
    }
}
