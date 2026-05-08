mod bindings;
mod buffers;
mod line_read;
mod literals;

pub(super) use buffers::find_runtime_text_input_buffer_data_object;
pub(in crate::instructions) use buffers::runtime_text_input_buffer_for_text_place;
pub(in crate::instructions) use line_read::runtime_machine_string_descriptor_offset;
pub(super) use line_read::runtime_text_line_read;
pub(super) use literals::runtime_text_literal_for_host_call;
pub(in crate::instructions) use literals::runtime_text_literal_write_for_host_call;
