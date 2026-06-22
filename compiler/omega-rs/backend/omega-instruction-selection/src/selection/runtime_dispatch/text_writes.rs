mod builder;
mod descriptor;

pub(super) use builder::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_with_handle_resolver_emit,
    runtime_text_builder_write_with_scratch_emit,
};
pub(super) use descriptor::{select_runtime_string_descriptor_write, string_literal_data_handle};
