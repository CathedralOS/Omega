mod builder;
mod descriptor;

pub(super) use builder::{
    runtime_text_builder_write_emit, runtime_text_builder_write_with_resolver_emit,
    runtime_text_builder_write_without_aliases_emit,
};
pub(super) use descriptor::{select_runtime_string_descriptor_write, string_literal_data_handle};
