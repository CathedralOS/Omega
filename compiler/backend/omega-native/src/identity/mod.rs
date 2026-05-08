mod branching;
mod expressions;
mod plan_sections;
mod storage;
mod targets;

use crate::control_flow::OperationKind;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use branching::count_runtime_branching_strings;
use expressions::count_expression_strings;
use plan_sections::{
    count_alias_flow_strings, count_host_call_strings, count_instruction_strings,
    count_layout_strings, count_object_strings, count_phase_timing_strings,
    count_runtime_storage_strings, count_runtime_text_strings, count_state_call_strings,
    count_state_storage_strings, count_state_value_strings,
};
pub use storage::NativeStringStorage;
use targets::{count_planned_target_strings, count_runtime_target_strings};
use omega_typed_program::statement::TransitionGuard;

pub fn count_native_string_storage(native_plan: &NativePlan) -> NativeStringStorage {
    let mut storage = NativeStringStorage::default();

    storage.count_identity(&native_plan.entry_machine);
    storage.count_identity(&native_plan.entry_state);

    count_control_flow_strings(native_plan, &mut storage);
    count_runtime_flow_strings(native_plan, &mut storage);
    count_state_dispatch_strings(native_plan, &mut storage);
    count_runtime_body_strings(native_plan, &mut storage);
    count_runtime_branching_strings(native_plan, &mut storage);
    count_state_guard_strings(native_plan, &mut storage);
    count_runtime_dispatch_loop_strings(native_plan, &mut storage);
    count_host_call_strings(native_plan, &mut storage);
    count_state_call_strings(native_plan, &mut storage);
    count_alias_flow_strings(native_plan, &mut storage);
    count_state_storage_strings(native_plan, &mut storage);
    count_state_value_strings(native_plan, &mut storage);
    count_runtime_storage_strings(native_plan, &mut storage);
    count_runtime_text_strings(native_plan, &mut storage);
    count_layout_strings(native_plan, &mut storage);
    count_instruction_strings(native_plan, &mut storage);
    count_object_strings(native_plan, &mut storage);
    count_phase_timing_strings(native_plan, &mut storage);

    storage
}

fn count_control_flow_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, machine) in native_plan.control_flow.machines.iter() {
        storage.count_program_name_identity(&machine.name);
        for contained in &machine.contains {
            storage.count_program_name_identity(&contained.name);
            storage.count_program_name_identity(&contained.type_name);
        }
    }

    for (_, state) in native_plan.control_flow.states.iter() {
        storage.count_program_name_identity(&state.name);
        for parameter in &state.parameters {
            storage.count_program_name_identity(parameter);
        }
    }

    for (_, operation) in native_plan.control_flow.operations.iter() {
        match &operation.kind {
            OperationKind::Assignment { target, value }
            | OperationKind::StaticAssignment { target, value } => {
                count_expression_strings(target, storage);
                count_expression_strings(value, storage);
            }
            OperationKind::Call {
                receiver: _,
                target: _,
                arguments,
            } => {
                for argument in arguments {
                    count_expression_strings(argument, storage);
                }
            }
            OperationKind::ConstantIntegerAssignment
            | OperationKind::Expression
            | OperationKind::LocalData => {}
        }
    }

    for (_, transition) in native_plan.control_flow.transitions.iter() {
        count_planned_target_strings(&transition.target, storage);
        if let Some(continuation) = &transition.continuation {
            count_planned_target_strings(continuation, storage);
        }
    }
}

fn count_runtime_flow_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
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

fn count_state_dispatch_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, state) in native_plan.state_dispatch.states.iter() {
        storage.count_program_name_identity(&state.machine);
        storage.count_program_name_identity(&state.state);
        storage.count_generated_symbol(&state.label);
    }
    for (_, edge) in native_plan.state_dispatch.edges.iter() {
        count_runtime_target_strings(&edge.target, storage);
        count_runtime_target_strings(&edge.continuation, storage);
    }
}

fn count_runtime_body_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
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

fn count_state_guard_strings(native_plan: &NativePlan, storage: &mut NativeStringStorage) {
    for (_, guard) in native_plan.state_guards.guards.iter() {
        storage.count_program_name_identity(&guard.source_machine);
        storage.count_program_name_identity(&guard.source_state);
        count_runtime_target_strings(&guard.target, storage);
        count_runtime_target_strings(&guard.continuation, storage);
        count_expression_strings(&guard.expression, storage);
    }
    for (_, operand) in native_plan.state_guards.operands.iter() {
        count_expression_strings(&operand.expression, storage);
    }
}

fn count_runtime_dispatch_loop_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    storage.count_generated_symbol(&native_plan.runtime_dispatch_loop.current_state_slot);
    storage.count_generated_symbol(&native_plan.runtime_dispatch_loop.next_state_slot);
    for (_, dispatch_case) in native_plan.runtime_dispatch_loop.cases.iter() {
        storage.count_program_name_identity(&dispatch_case.machine);
        storage.count_program_name_identity(&dispatch_case.state);
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
