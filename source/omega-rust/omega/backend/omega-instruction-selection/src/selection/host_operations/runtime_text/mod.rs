mod buffers;
mod byte_io;
mod line_read;
mod literals;

pub(super) use byte_io::{runtime_byte_read, runtime_byte_write};

pub(super) use buffers::find_runtime_text_input_buffer_data_object;
pub(in crate::selection) use buffers::{
    runtime_text_input_buffer_data_for_text_place,
    runtime_text_input_buffer_data_for_text_place_in_table,
};
pub(in crate::selection) use line_read::runtime_string_descriptor_place;
pub(super) use line_read::runtime_text_line_read;
pub(super) use literals::runtime_text_literal_for_host_call;
pub(in crate::selection) use literals::runtime_text_literal_write_for_host_call;
