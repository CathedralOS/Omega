mod branching;
mod expressions;
mod plan_sections;
mod runtime_sections;
mod storage;
mod targets;

use crate::control_flow::OperationKind;
use crate::plan::NativePlan;
use branching::count_runtime_branching_strings;
use expressions::count_expression_strings;
use plan_sections::{
    count_alias_flow_strings, count_host_call_strings, count_instruction_strings,
    count_layout_strings, count_object_strings, count_phase_timing_strings,
    count_runtime_storage_strings, count_runtime_text_strings, count_state_call_strings,
    count_state_storage_strings, count_state_value_strings,
};
use runtime_sections::{
    count_runtime_body_strings, count_runtime_dispatch_loop_strings, count_runtime_flow_strings,
    count_state_dispatch_strings, count_state_guard_strings,
};
pub use storage::NativeStringStorage;
use targets::count_planned_target_strings;

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
