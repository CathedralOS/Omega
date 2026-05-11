mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;

pub use dispatch::{
    encode_dispatch_case_enter, encode_dispatch_case_leave, encode_dispatch_guard_compare_static,
    encode_dispatch_loop_enter, encode_dispatch_state_write,
};
pub use host::{encode_host_call_sequence, encode_return, encode_syscall_sequence};
pub use runtime_storage::{
    encode_runtime_frame_indexed_integer_write, encode_runtime_machine_integer_write,
    encode_runtime_machine_string_write, encode_runtime_storage_compare,
    encode_runtime_storage_copy, encode_runtime_storage_copy_to_runtime_frame_indexed,
    encode_runtime_storage_value_compare,
};
pub use runtime_text::{
    encode_runtime_text_buffer_materialize, encode_runtime_text_line_read,
    encode_runtime_text_literal_append, encode_runtime_text_literal_compare,
    encode_runtime_text_literal_segment_write, encode_runtime_text_literal_write,
    encode_runtime_text_storage_compare, encode_runtime_text_stored_place_append,
    encode_runtime_text_stored_suffix_append,
};
