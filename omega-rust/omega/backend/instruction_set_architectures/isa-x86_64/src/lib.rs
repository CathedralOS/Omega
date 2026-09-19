//! Clean x86-64 encoders owned by the terminal-Psi realization lane.
//!
//! This crate deliberately consumes only normalized target and terminal
//! installation facts. It does not depend on either legacy operation graph or
//! any source-shaped Psi representation.

mod fma;
mod frame_protocol;
mod hosted_linux_encoding;
mod ieee_float;
mod machine_effects;
mod post_handoff_writer;
mod preservation_storage;
mod register_model;
pub use register_model::X86_64_COPY_BYTES;
pub use register_model::X86_64_DIVIDE_U64;
pub use register_model::X86_64_LOAD8;
pub use register_model::X86_64_LOAD16;
pub use register_model::X86_64_LOAD32;
pub use register_model::X86_64_MATERIALIZE_BOOLEAN;
pub use register_model::X86_64_MULTIPLY_I64;
pub use register_model::X86_64_REMAINDER_I64;
pub use register_model::X86_64_SATURATING_ADD_CLAMPED;
pub use register_model::X86_64_SATURATING_ADD_U64;
pub use register_model::X86_64_SATURATING_DIVIDE_SIGNED;
pub use register_model::X86_64_SATURATING_SUBTRACT_CLAMPED;
pub use register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED;
pub use register_model::{
    X86_64_BITS_TO_FLOAT32, X86_64_BITS_TO_FLOAT64, X86_64_FLOAT32_TO_BITS, X86_64_FLOAT64_TO_BITS,
    X86_64_HOSTED_READ_BYTE,
};
pub use register_model::{
    x86_64_microsoft_mixed_unit_call_keys, x86_64_system_v_mixed_unit_call_keys,
};
mod selected_form_encoding;
mod semantic_unit_wrapper_encoding;

pub use fma::{
    DecodedScalarFmaFormat, DecodedVfmadd132Scalar, decode_vfmadd132_scalar, encode_vfmadd132sd,
    encode_vfmadd132ss,
};
pub use frame_protocol::{
    X86_64_STACK_PROBE_INTERVAL_BYTES, X86_64FrameProtocolError, X86_64FrameSlot, X86_64StackProbe,
    encode_system_v_amd64_frame_protocol,
};
pub use ieee_float::{
    OMEGA_CANONICAL_MXCSR, encode_binary32_bits_to_xmm, encode_binary64_bits_to_xmm,
    encode_ldmxcsr_rsp_displacement, encode_stmxcsr_rsp_displacement,
    encode_store_mxcsr_constant_rsp_displacement,
};
pub use machine_effects::{
    X86_64MachineEffectCatalogValidationError, validate_x86_64_machine_effect_catalog,
    x86_64_machine_effect_catalog,
};
pub use post_handoff_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
pub use preservation_storage::{
    X86_64PreservationStorageCatalogError, x86_64_preservation_storage_catalog,
};
pub use register_model::X86_64_HOSTED_WRITE_BYTE_I32;
pub use register_model::x86_64_microsoft_normalized_foreign_call_keys;
pub use register_model::x86_64_microsoft_register_call_keys;
pub use register_model::x86_64_system_v_normalized_foreign_call_keys;
pub use register_model::x86_64_system_v_register_call_keys;
pub use register_model::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64,
    X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_JUMP, X86_64_LINUX_SYSTEM_CALL, X86_64_MATERIALIZE_I64,
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT,
    X86_64_REQUIRED_REGISTER_CONSTRAINTS, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE,
    X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64, X86_64_SYSTEM_V_RETURN,
    X86_64_SYSTEM_V_RETURN_UNIT, X86_64RegisterConstraintCatalogValidationError,
    validate_x86_64_register_constraint_catalog, x86_64_fixed_register_view,
    x86_64_physical_register_model, x86_64_preservation_convention_for_target,
    x86_64_register_constraint_catalog,
};
pub use register_model::{
    X86_64_LOAD_PACKED, X86_64_STORE_PACKED, x86_64_microsoft_aggregate_call_keys,
    x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_mixed_aggregate_call_keys,
    x86_64_system_v_mixed_aggregate_call_keys,
};
pub use register_model::{
    x86_64_microsoft_register_unit_call_keys, x86_64_system_v_register_unit_call_keys,
};
pub use register_model::{
    x86_64_system_v_aggregate_call_keys, x86_64_system_v_aggregate_return_keys,
};
pub use selected_form_encoding::hosted_read_byte::{
    decode_x86_64_selected_hosted_read_byte, encode_x86_64_selected_hosted_read_byte_form,
    validate_x86_64_selected_hosted_read_byte_form,
};
pub use selected_form_encoding::hosted_write_byte::decode_x86_64_selected_hosted_write_byte_i32;
pub use selected_form_encoding::materialization::mov_r32_imm32::{
    ValidatedX86_64MovR32Imm32I64Materialization, X86_64DecodedMovR32Imm32I64Materialization,
    X86_64MovR32Imm32I64MaterializationError, X86_64MovR32Imm32I64MaterializationFootprint,
    decode_x86_64_mov_r32_imm32_i64_materialization,
    encode_x86_64_mov_r32_imm32_i64_materialization,
    validate_x86_64_mov_r32_imm32_i64_materialization,
};
pub use selected_form_encoding::materialization::mov_r64_imm32_sign_extended::{
    ValidatedX86_64MovR64Imm32SignExtendedI64Materialization,
    X86_64DecodedMovR64Imm32SignExtendedI64Materialization,
    X86_64MovR64Imm32SignExtendedI64MaterializationError,
    X86_64MovR64Imm32SignExtendedI64MaterializationFootprint,
    decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization,
    encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization,
    validate_x86_64_mov_r64_imm32_sign_extended_i64_materialization,
};
pub use selected_form_encoding::materialization::xor_zero::{
    ValidatedX86_64XorZeroI64Materialization, X86_64DecodedXorZeroI64Materialization,
    decode_x86_64_xor_zero_i64_materialization, encode_x86_64_xor_zero_i64_materialization,
    validate_x86_64_xor_zero_i64_materialization,
};
pub use selected_form_encoding::{
    ValidatedX86_64SelectedFormEncoding, ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    ValidatedX86_64SelectedScalarCallTemplate, X86_64_NORMALIZED_FOREIGN_CALL_OPCODE_OFFSET,
    X86_64_NORMALIZED_FOREIGN_CALL_PATCH_OFFSET, X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
    X86_64_NORMALIZED_FOREIGN_CALL_REFERENCE_OFFSET,
    X86_64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT, X86_64_SCALAR_CALL_OPCODE_OFFSET,
    X86_64_SCALAR_CALL_PATCH_OFFSET, X86_64_SCALAR_CALL_PATCH_WIDTH,
    X86_64_SCALAR_CALL_REFERENCE_OFFSET, X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT,
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState, X86_64NormalizedForeignCallTemplateError,
    X86_64ScalarCallFixup, X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    X86_64ScalarCallTemplateError, X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    encode_x86_64_selected_form, encode_x86_64_selected_i64_less_than_branch_form,
    encode_x86_64_selected_jump_form, encode_x86_64_selected_memory_form,
    encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_normalized_foreign_call_template,
    encode_x86_64_selected_scalar_call_template, encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form, validate_x86_64_selected_form_encoding,
    validate_x86_64_selected_i64_less_than_branch_form, validate_x86_64_selected_jump_form,
    validate_x86_64_selected_memory_form, validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_normalized_foreign_call_template,
    validate_x86_64_selected_scalar_call_template,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
pub use selected_form_encoding::{
    encode_x86_64_selected_hosted_write_byte_form, validate_x86_64_selected_hosted_write_byte_form,
};
pub use semantic_unit_wrapper_encoding::{
    ValidatedX86_64ResolvedSemanticUnitWrapper, ValidatedX86_64SemanticUnitWrapperTemplate,
    X86_64_SEMANTIC_UNIT_WRAPPER_CALL_BUNDLE_BYTE_COUNT,
    X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT,
    X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH, X86_64_SEMANTIC_UNIT_WRAPPER_RETURN_OFFSET,
    X86_64SemanticUnitWrapperArgumentBinding, X86_64SemanticUnitWrapperCallEffect,
    X86_64SemanticUnitWrapperCleanupEffect, X86_64SemanticUnitWrapperCopy,
    X86_64SemanticUnitWrapperEncodingError, X86_64SemanticUnitWrapperEncodingPolicy,
    X86_64SemanticUnitWrapperEncodingRequest, X86_64SemanticUnitWrapperFootprint,
    X86_64SemanticUnitWrapperRelocation, X86_64SemanticUnitWrapperRelocationKind,
    X86_64SemanticUnitWrapperRelocationState, X86_64SemanticUnitWrapperResolution,
    X86_64SemanticUnitWrapperResolutionError, X86_64SemanticUnitWrapperResolutionState,
    X86_64SemanticUnitWrapperTrapBehavior, canonical_x86_64_semantic_unit_wrapper_encoding_request,
    encode_x86_64_semantic_unit_wrapper_template,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
    validate_x86_64_resolved_semantic_unit_wrapper, validate_x86_64_semantic_unit_wrapper_template,
};

pub use hosted_linux_encoding::{
    encode_hosted_exit_process_i32, encode_hosted_write_byte_i32_from_r11,
    encode_linux_read_byte_to_stack, encode_linux_write_line_literal,
};
pub use register_model::X86_64_HOSTED_EXIT_PROCESS_I32;
pub use register_model::x86_64_indirect_aggregate_call_keys;
pub use register_model::{
    X86_64_ADDRESS_OFFSET, X86_64_FRAME_ADDRESS, X86_64_LOAD8_INDEXED, X86_64_LOAD64,
    X86_64_MICROSOFT_CALL_UNIT, X86_64_STORE, X86_64_STORE64,
};
pub use register_model::{x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys};
pub use selected_form_encoding::hosted_exit_process::{
    decode_x86_64_selected_hosted_exit_process_i32,
    encode_x86_64_selected_hosted_exit_process_form,
    validate_x86_64_selected_hosted_exit_process_form,
};
