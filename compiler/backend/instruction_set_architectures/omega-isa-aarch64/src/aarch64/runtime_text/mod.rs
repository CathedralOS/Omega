mod append;
mod compare;
mod read;
mod write;

pub use append::{
    encode_runtime_text_buffer_materialize,
    encode_runtime_text_buffer_materialize_to_runtime_frame_indexed,
    encode_runtime_text_buffer_materialize_to_runtime_pointee, encode_runtime_text_literal_append,
    encode_runtime_text_literal_append_to_runtime_frame_indexed,
    encode_runtime_text_literal_append_to_runtime_pointee, encode_runtime_text_stored_place_append,
    encode_runtime_text_stored_place_append_to_runtime_frame_indexed,
    encode_runtime_text_stored_place_append_to_runtime_pointee,
    encode_runtime_text_stored_suffix_append,
};
pub use compare::{encode_runtime_text_literal_compare, encode_runtime_text_storage_compare};
pub use read::{encode_runtime_text_line_read_import, encode_runtime_text_line_read_syscall};
pub use write::{encode_runtime_text_literal_segment_write, encode_runtime_text_literal_write};
