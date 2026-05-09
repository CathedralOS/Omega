mod branching;
mod control_flow_section;
mod expressions;
mod plan_sections;
mod runtime_sections;
mod storage;
mod targets;

use crate::BackendReportInput;
use branching::count_runtime_branching_strings;
use control_flow_section::count_control_flow_strings;
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

pub fn count_native_string_storage(native_plan: &BackendReportInput<'_>) -> NativeStringStorage {
    let mut storage = NativeStringStorage::default();

    storage.count_identity(native_plan.entry_machine_name());
    storage.count_identity(native_plan.entry_state_name());

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
