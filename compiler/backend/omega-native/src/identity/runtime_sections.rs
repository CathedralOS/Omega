use crate::identity::NativeStringStorage;
use crate::identity::expressions::count_expression_strings;
use crate::identity::targets::count_runtime_target_strings;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use omega_typed_program::statement::TransitionGuard;

pub(in crate::identity) fn count_runtime_flow_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, state) in native_plan.runtime_flow.states.iter() {
        storage.count_program_name_identity(&state.machine);
        storage.count_program_name_identity(&state.state);
    }
    for (_, edge) in native_plan.runtime_flow.edges.iter() {
        storage.count_program_name_identity(&edge.from_machine);
        storage.count_program_name_identity(&edge.from_state);
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
    for (_, state) in native_plan.runtime_flow.cycle_states.iter() {
        storage.count_program_name_identity(&state.machine);
        storage.count_program_name_identity(&state.state);
    }
}

pub(in crate::identity) fn count_state_dispatch_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, state) in native_plan.state_dispatch.states.iter() {
        storage.count_generated_symbol(&state.label);
    }
    for (_, edge) in native_plan.state_dispatch.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
}

pub(in crate::identity) fn count_runtime_body_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        storage.count_program_name_identity(&body.machine);
        storage.count_program_name_identity(&body.state);
    }
    for (_, operation) in native_plan.runtime_bodies.operations.iter() {
        storage.count_program_name_identity(&operation.source_machine);
        storage.count_program_name_identity(&operation.source_state);
        match &operation.kind {
            RuntimeDispatchBodyOperationKind::HostCall { platform_call } => {
                storage.count_identity(platform_call);
            }
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                target_machine,
                target_state,
                ..
            }
            | RuntimeDispatchBodyOperationKind::InlineStateCall {
                target_machine,
                target_state,
                ..
            }
            | RuntimeDispatchBodyOperationKind::StateCall {
                target_machine,
                target_state,
                ..
            } => {
                storage.count_program_name_identity(target_machine);
                storage.count_program_name_identity(target_state);
            }
            RuntimeDispatchBodyOperationKind::LocalStorage { name, type_name } => {
                storage.count_program_name_identity(name);
                storage.count_identity(type_name);
            }
            RuntimeDispatchBodyOperationKind::Mutation { .. }
            | RuntimeDispatchBodyOperationKind::Other => {}
        }
    }
}

pub(in crate::identity) fn count_state_guard_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, guard) in native_plan.state_guards.guards.iter() {
        count_runtime_target_strings(&guard.target, storage);
        count_runtime_target_strings(&guard.continuation, storage);
        count_expression_strings(&guard.expression, storage);
    }
    for (_, operand) in native_plan.state_guards.operands.iter() {
        count_expression_strings(&operand.expression, storage);
    }
}

pub(in crate::identity) fn count_runtime_dispatch_loop_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    storage.count_generated_symbol(&native_plan.runtime_dispatch_loop.current_state_slot);
    storage.count_generated_symbol(&native_plan.runtime_dispatch_loop.next_state_slot);
    for (_, dispatch_case) in native_plan.runtime_dispatch_loop.cases.iter() {
        storage.count_generated_symbol(&dispatch_case.label);
    }
    for (_, edge) in native_plan.runtime_dispatch_loop.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
        if let TransitionGuard::When(expression) = &edge.guard {
            count_expression_strings(expression, storage);
        }
    }
}
