mod calls;
mod layout;
mod output;
mod runtime_text;
mod storage;

pub(in crate::identity) use calls::{
    count_alias_flow_strings, count_host_call_strings, count_state_call_strings,
};
pub(in crate::identity) use layout::count_layout_strings;
pub(in crate::identity) use output::{
    count_instruction_strings, count_object_strings, count_phase_timing_strings,
};
pub(in crate::identity) use runtime_text::count_runtime_text_strings;
pub(in crate::identity) use storage::{
    count_runtime_storage_strings, count_state_storage_strings, count_state_value_strings,
};
