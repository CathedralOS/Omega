//! Clean AArch64 encoders owned by the terminal-Psi realization lane.
//!
//! Only normalized target and terminal-installation facts enter this crate;
//! source-shaped representations and legacy operation graphs are absent.

mod floating_control;
mod frame_protocol;
mod hosted_sequences;
mod machine_effects;
mod post_handoff_writer;
mod preservation_storage;
mod register_model;
mod saturating_forms;
pub use register_model::AARCH64_DIVIDE_U64;
pub use register_model::AARCH64_LOAD8;
pub use register_model::AARCH64_LOAD16;
pub use register_model::AARCH64_LOAD32;
pub use register_model::AARCH64_MATERIALIZE_BOOLEAN;
pub use register_model::AARCH64_MULTIPLY_I64;
pub use register_model::AARCH64_REMAINDER_I64;
pub use register_model::AARCH64_SATURATING_ADD_CLAMPED;
pub use register_model::AARCH64_SATURATING_ADD_U64;
pub use register_model::AARCH64_SATURATING_DIVIDE_SIGNED;
pub use register_model::AARCH64_SATURATING_SUBTRACT_CLAMPED;
pub use register_model::AARCH64_SATURATING_SUBTRACT_UNSIGNED;
pub use register_model::{
    AARCH64_BITS_TO_FLOAT32, AARCH64_BITS_TO_FLOAT64, AARCH64_DARWIN_HOSTED_READ_BYTE,
    AARCH64_FLOAT32_TO_BITS, AARCH64_FLOAT64_TO_BITS, AARCH64_HOSTED_READ_BYTE,
};
pub use register_model::{
    aarch64_aapcs64_mixed_unit_call_keys, aarch64_darwin_mixed_unit_call_keys,
};
mod selected_form_encoding;
pub use floating_control::{
    encode_restore_fpcr_from_sp_displacement, encode_save_fpcr_to_sp_displacement,
};
pub use frame_protocol::{
    Aarch64FrameProtocolError, Aarch64FrameSlot, Aarch64StackProbe, encode_aapcs64_frame_protocol,
};
pub use machine_effects::{
    Aarch64MachineEffectCatalogValidationError, aarch64_machine_effect_catalog,
    validate_aarch64_machine_effect_catalog,
};
pub use post_handoff_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
pub use preservation_storage::{
    Aarch64PreservationStorageCatalogError, aarch64_preservation_storage_catalog,
};
pub use register_model::aarch64_aapcs64_register_call_keys;
pub use register_model::aarch64_darwin_register_call_keys;
pub use register_model::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
    AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64,
    AARCH64_COMPARE_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_BYTES, AARCH64_COPY_I64, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT, AARCH64_INLINE_ASSEMBLY_DEFAULT, AARCH64_JUMP,
    AARCH64_LINUX_SYSTEM_CALL, AARCH64_MATERIALIZE_I64, AARCH64_REQUIRED_REGISTER_CONSTRAINTS,
    AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
    Aarch64RegisterConstraintCatalogValidationError, aarch64_fixed_register_view,
    aarch64_physical_register_model, aarch64_preservation_convention_for_target,
    aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
};
pub use register_model::{
    AARCH64_ADDRESS_OFFSET, AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32, AARCH64_FRAME_ADDRESS,
    AARCH64_HOSTED_WRITE_BYTE_I32, AARCH64_LOAD_PACKED, AARCH64_LOAD8_INDEXED, AARCH64_LOAD64,
    AARCH64_STORE, AARCH64_STORE_PACKED, AARCH64_STORE64,
};
pub use register_model::{
    aarch64_aapcs64_register_unit_call_keys, aarch64_darwin_register_unit_call_keys,
};
pub use register_model::{
    aarch64_mixed_aggregate_call_keys, aarch64_register_aggregate_call_keys,
    aarch64_register_aggregate_return_keys,
};
pub use selected_form_encoding::hosted_read_byte::{
    decode_aarch64_selected_hosted_read_byte, encode_aarch64_selected_hosted_read_byte_form,
    validate_aarch64_selected_hosted_read_byte_form,
};
pub use selected_form_encoding::hosted_write_byte::decode_aarch64_selected_hosted_write_byte_i32;
pub use selected_form_encoding::{
    AARCH64_SCALAR_CALL_OPCODE_OFFSET, AARCH64_SCALAR_CALL_PATCH_OFFSET,
    AARCH64_SCALAR_CALL_PATCH_WIDTH, AARCH64_SCALAR_CALL_REFERENCE_OFFSET,
    AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT, Aarch64MovkPatch, Aarch64MovnSeed,
    Aarch64ScalarCallFixup, Aarch64ScalarCallFixupKind, Aarch64ScalarCallFixupState,
    Aarch64ScalarCallTemplateError, Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    Aarch64ShortestMovnMaterializationRecipe, ValidatedAarch64SelectedFormEncoding,
    ValidatedAarch64SelectedScalarCallTemplate, aarch64_shortest_movn_materialization_recipe,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_form, encode_aarch64_selected_i64_less_than_branch_form,
    encode_aarch64_selected_jump_form, encode_aarch64_selected_nonzero_branch_form,
    encode_aarch64_selected_scalar_call_template,
    encode_aarch64_selected_u64_less_than_branch_form,
    encode_aarch64_shortest_movn_materialization,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_form_encoding, validate_aarch64_selected_i64_less_than_branch_form,
    validate_aarch64_selected_jump_form, validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_scalar_call_template,
    validate_aarch64_selected_u64_less_than_branch_form,
    validate_aarch64_shortest_movn_materialization,
};
pub use selected_form_encoding::{
    encode_aarch64_selected_hosted_write_byte_form,
    validate_aarch64_selected_hosted_write_byte_form,
};
pub use selected_form_encoding::{
    encode_aarch64_selected_memory_form, validate_aarch64_selected_memory_form,
};

pub use register_model::{AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32, AARCH64_HOSTED_EXIT_PROCESS_I32};
pub use selected_form_encoding::hosted_exit_process::{
    decode_aarch64_selected_hosted_exit_process_i32,
    encode_aarch64_selected_hosted_exit_process_form,
    validate_aarch64_selected_hosted_exit_process_form,
};

pub use hosted_sequences::{
    encode_hosted_exit_process_i32, encode_hosted_write_byte_i32_from_w9,
    encode_linux_read_byte_to_stack, encode_linux_write_line_literal,
    encode_macos_read_byte_to_stack,
};
pub use register_model::aarch64_indirect_aggregate_call_keys;
pub use register_model::{aarch64_float_scalar_call_keys, aarch64_float_scalar_return_keys};
