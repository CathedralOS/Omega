mod atomics;
mod caller_frame;
mod dispatch;
mod encoding_primitives;
mod fma;
mod function_boundary;
mod function_frame;
mod generated_writer;
mod host_calls;
mod native_fuel;
mod place_copy;
mod privileged_effects;
mod runtime_storage;
mod runtime_text;
mod syscalls;
mod wire;

pub use atomics::*;
pub use caller_frame::*;
pub use dispatch::*;
pub(crate) use encoding_primitives::*;
pub use fma::*;
pub use function_boundary::*;
pub use function_frame::*;
pub use generated_writer::*;
pub use host_calls::*;
pub use native_fuel::*;
pub use omega_terminal_isa_x86_64::{
    X86_64_ADD_I64, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64,
    X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_LINUX_SYSTEM_CALL, X86_64_MATERIALIZE_I64,
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN, X86_64_REQUIRED_REGISTER_CONSTRAINTS,
    X86_64_SUBTRACT_I64, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN,
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
pub use place_copy::{
    PLACE_COPY_MAX_SITES, PlaceCopySide, PlaceCopySites, copy_places_clobbers,
    copy_places_direct_clobbers, copy_places_from_frame_base_double_indexed_clobbers,
    copy_places_from_frame_base_indexed_clobbers, copy_places_from_indexed_clobbers,
    copy_places_from_machine_double_indexed_clobbers, copy_places_from_machine_indexed_clobbers,
    copy_places_from_pointee_clobbers, copy_places_indexed_to_pointee_clobbers,
    copy_places_machine_indexed_pair_clobbers, copy_places_pointee_pair_clobbers,
    copy_places_to_indexed_clobbers, copy_places_to_machine_double_indexed_clobbers,
    copy_places_to_machine_indexed_clobbers, copy_places_to_pointee_clobbers, encode_copy_places,
    encode_place_address_write, encode_place_binary_write,
    encode_place_bounded_buffer_literal_append, encode_place_bounded_buffer_source_append,
    encode_place_bounded_buffer_write, encode_place_compare, encode_place_convert_write,
    encode_place_copy, encode_place_copy_shared_base, encode_place_integer_write,
    encode_place_string_write, encode_place_text_buffer_materialize,
    encode_place_text_literal_append, encode_place_text_stored_append, encode_place_value_compare,
    encode_runtime_frame_data_address_write, encode_runtime_storage_function_address_write,
    place_address_write_additional_machine_state, place_address_write_register_writes,
    place_binary_index_base_positions, place_binary_operand_start_width,
    place_bounded_buffer_literal_append_additional_machine_state,
    place_bounded_buffer_literal_append_register_writes,
    place_bounded_buffer_source_append_additional_machine_state,
    place_bounded_buffer_source_append_register_writes,
    place_bounded_buffer_write_additional_machine_state,
    place_bounded_buffer_write_register_writes, place_compare_additional_machine_state,
    place_compare_register_writes, place_integer_write_clobbers,
    place_string_write_additional_machine_state, place_string_write_register_writes,
    place_text_buffer_materialize_additional_machine_state,
    place_text_buffer_materialize_register_writes, place_text_literal_append_register_writes,
    place_text_stored_append_register_writes, place_value_compare_additional_machine_state,
    place_value_compare_register_writes,
};
pub use privileged_effects::*;
pub use runtime_storage::*;
pub(crate) use runtime_storage::{
    append_runtime_binary_operation, append_runtime_convert_operation,
    append_runtime_value_operand, runtime_binary_operation_width,
};
pub use runtime_text::*;
pub use syscalls::*;
pub use wire::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64RelocationSiteKind {
    Absolute64,
    Relative32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64RelocationSite {
    pub operand_index: Option<usize>,
    pub byte_offset: usize,
    pub byte_width: usize,
    pub kind: X86_64RelocationSiteKind,
}

/// Relocation imm offset (pre-`+2`) of the frame base loaded for the target slot
/// store in `encode_runtime_frame_base_indexed_address_to_runtime_frame_write`.
pub const FRAME_BASE_INDEXED_ADDRESS_TARGET_FRAME_IMM_OFFSET: usize = 34;

/// Relocation imm offset (pre-`+2`) of the TARGET region base `mov` in the
/// fixed-indexed slice-element copy (the materializer's canonical shape:
/// source base mov (10) + descriptor deref (7)).
pub const FRAME_FIXED_INDEXED_COPY_TARGET_IMM_OFFSET: usize = 17;

/// Relocation imm offset (pre-`+2`) of the TARGET region base `mov` in the
/// runtime-indexed slice-element copy (the materializer's canonical shape:
/// frame base (10) + index load (7) + imul (7) + descriptor deref (7) +
/// add (3)).
pub const FRAME_INDEXED_COPY_TARGET_IMM_OFFSET: usize = 34;
