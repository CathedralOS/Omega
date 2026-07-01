mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;
mod wire_decode;
mod wire_encode;

pub use wire_decode::{
    encode_read_wire_byte_slice, encode_read_wire_expected_byte, encode_read_wire_nested_close,
    encode_read_wire_nested_open, encode_read_wire_repeated_scalar_varint,
    encode_read_wire_scalar_varint,
};
pub use wire_encode::{
    encode_append_wire_literal_byte, encode_append_wire_repeated_scalar_varint,
    encode_append_wire_scalar_varint, encode_append_wire_text_bytes,
};

pub use dispatch::{
    encode_dispatch_case_enter_bytes, encode_dispatch_case_leave_bytes,
    encode_dispatch_guard_compare_static_bytes, encode_dispatch_loop_enter_bytes,
    encode_dispatch_state_write_bytes,
};
pub use host::{
    encode_function_enter_bytes, encode_host_call_sequence, encode_return_bytes,
    encode_return_register_integer_write_bytes,
    encode_runtime_storage_copy_to_return_register_bytes, encode_syscall_sequence,
};
pub use runtime_storage::{
    encode_atomic_compare_exchange, encode_atomic_fetch_add,
    encode_runtime_frame_base_indexed_address_to_runtime_frame_write,
    encode_runtime_frame_base_indexed_binary_write,
    encode_runtime_frame_base_indexed_integer_write,
    encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write,
    encode_runtime_frame_indexed_address_to_runtime_frame_write,
    encode_runtime_frame_indexed_binary_write, encode_runtime_frame_indexed_integer_write,
    encode_runtime_frame_indexed_string_write, encode_runtime_frame_string_write,
    encode_runtime_machine_bounded_buffer_literal_append,
    encode_runtime_machine_bounded_buffer_source_append,
    encode_runtime_machine_bounded_buffer_write, encode_runtime_machine_indexed_binary_write,
    encode_runtime_machine_indexed_integer_write,
    encode_runtime_machine_indexed_string_write, encode_runtime_machine_integer_write,
    encode_runtime_machine_string_write,
    encode_runtime_pointee_address_to_runtime_frame_write, encode_runtime_pointee_binary_write,
    encode_runtime_pointee_bounded_buffer_write, encode_runtime_pointee_integer_write,
    encode_runtime_pointee_string_write,
    encode_runtime_storage_address_to_runtime_frame_write, encode_runtime_storage_binary_write,
    encode_runtime_storage_compare_bytes, encode_runtime_storage_convert,
    encode_runtime_storage_copy,
    encode_runtime_storage_copy_from_runtime_frame_fixed_indexed,
    encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee,
    encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage,
    encode_runtime_storage_copy_from_runtime_frame_indexed,
    encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee,
    encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage,
    encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage,
    encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame,
    encode_runtime_storage_copy_to_runtime_frame_indexed,
    encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage,
    encode_runtime_storage_copy_to_runtime_pointee, encode_runtime_storage_value_compare_bytes,
    encode_runtime_value_compare,
};
pub use runtime_text::{
    encode_runtime_text_buffer_materialize,
    encode_runtime_text_buffer_materialize_to_runtime_frame_indexed,
    encode_runtime_text_buffer_materialize_to_runtime_pointee, encode_runtime_text_line_read,
    encode_runtime_text_literal_append,
    encode_runtime_text_literal_append_to_runtime_frame_indexed,
    encode_runtime_text_literal_append_to_runtime_pointee, encode_runtime_text_literal_compare,
    encode_runtime_text_literal_segment_write, encode_runtime_text_literal_write,
    encode_runtime_text_storage_compare_bytes, encode_runtime_text_stored_place_append,
    encode_runtime_text_stored_place_append_to_runtime_frame_indexed,
    encode_runtime_text_stored_place_append_to_runtime_pointee,
    encode_runtime_text_stored_suffix_append,
};
