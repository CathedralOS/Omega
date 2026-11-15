/*
We provide the AArch64 physical register model, register-constraint catalog,
machine-effect catalog, frame and FPCR custody protocols, hosted syscall
sequences, post-handoff writer lowering, and selected-form instruction encoders
and validators for the native realization pipeline. Every interface in this
crate consumes only normalized target, register-home, and terminal-installation
facts, with zero dependency on source-shaped Psi representations.
*/

/*
We pair every AArch64 byte producer with an independent structural decoder and
validator inside the same domain boundary so that no downstream layout,
relaxation, or image-emission stage ever trusts an unchecked instruction word
stream or a claimed byte width. By deriving register constraints, machine
effects, encoder word sequences, and decoded footprints from shared AArch64
realization tables (`register_model`, `machine_effects`, `saturating_forms`, and
`selected_form_encoding`), we guarantee that instruction selection, register
allocation, and binary emission agree on every register read, clobber, flag
write, memory access, and trap edge.
*/

/*
Our domain modules serve the AArch64 backend across selection, allocation, and
emission:
- `register_model` and `preservation_storage`: physical AArch64 registers,
  AAPCS64 and Darwin call/operation constraint keys, and callee-saved storage
  catalogs.
- `machine_effects` and `saturating_forms`: per-alternative encoded effect
  declarations and saturating arithmetic word realizations.
- `frame_protocol`, `immediate_offset`, and `floating_control`: fixed-frame
  prologue/epilogue save/restore, stack-commit probing, 64-bit immediate
  addressing steps, and FPCR save/restore sequences.
- `selected_form_encoding`: encoding, structural decoding, and validation for
  all AArch64 selected instruction forms, branches, and call templates.
- `generated_entry`, `hosted_sequences`, and `post_handoff_writer`: generated
  entry replay, import-free Linux/Darwin syscall leaves, and address-free
  post-handoff writer code generation.
*/

pub(crate) use optimization_core::codec_bytes;
mod floating_control;
mod frame_protocol;
mod generated_entry;
pub use generated_entry::decode_aarch64_generated_entry;
mod hosted_sequences;
mod immediate_offset;
mod machine_effects;
mod post_handoff_writer;
mod preservation_storage;
mod register_model;
pub use register_model::aarch64_linux_system_call_keys;
pub use register_model::{AARCH64_LOAD_ENCODED_SCALAR, AARCH64_STORE_ENCODED_SCALAR};
mod saturating_forms;
#[cfg(test)]
pub(crate) mod test_support {
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{MachineAlternativeKey, SelectedInstructionKind};

    pub(crate) fn rejects_every_bit_flip(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) {
        reject_every_bit_flip(bytes, &format!("{kind:?}"), |changed| {
            crate::validate_aarch64_selected_form_encoding(
                physical,
                kind,
                alternative,
                operands,
                changed,
            )
            .is_err()
        });
    }

    pub(crate) fn reject_every_bit_flip(
        bytes: &[u8],
        description: &str,
        reject: impl Fn(&[u8]) -> bool,
    ) {
        for bit in 0..bytes.len() * 8 {
            let mut changed = bytes.to_vec();
            changed[bit / 8] ^= 1 << (bit % 8);
            assert!(reject(&changed), "{description} accepted changed bit {bit}");
        }
    }
}
pub use register_model::AARCH64_DIVIDE_I64;
pub use register_model::AARCH64_DIVIDE_U64;
pub use register_model::AARCH64_LOAD8;
pub use register_model::AARCH64_LOAD16;
pub use register_model::AARCH64_LOAD32;
pub use register_model::AARCH64_MATERIALIZE_BOOLEAN;
pub use register_model::AARCH64_MULTIPLY_I64;
pub use register_model::AARCH64_REMAINDER_I64;
pub use register_model::AARCH64_REMAINDER_U64;
pub use register_model::AARCH64_SATURATING_ADD_CLAMPED;
pub use register_model::AARCH64_SATURATING_ADD_U64;
pub use register_model::AARCH64_SATURATING_DIVIDE_SIGNED;
pub use register_model::AARCH64_SATURATING_MULTIPLY_CLAMPED;
pub use register_model::AARCH64_SATURATING_MULTIPLY_U64;
pub use register_model::AARCH64_SATURATING_SUBTRACT_CLAMPED;
pub use register_model::AARCH64_SATURATING_SUBTRACT_UNSIGNED;
pub use register_model::AARCH64_SHIFT_I64;
pub use register_model::{
    AARCH64_ATOMIC_COMPARE_EXCHANGE, AARCH64_ATOMIC_COMPARE_EXCHANGE_ONCE, AARCH64_ATOMIC_FENCE,
    AARCH64_ATOMIC_LOAD, AARCH64_ATOMIC_STORE, AARCH64_ATOMIC_UPDATE, AARCH64_BITS_TO_FLOAT32,
    AARCH64_BITS_TO_FLOAT64, AARCH64_CRASH, AARCH64_DARWIN_HOSTED_READ_BYTE,
    AARCH64_FLOAT32_TO_BITS, AARCH64_FLOAT64_TO_BITS, AARCH64_HOSTED_READ_BYTE,
    AARCH64_MEMORY_FENCE,
};
pub use register_model::{
    AARCH64_TRAPPING_BINARY, AARCH64_TRAPPING_CONVERT, AARCH64_TRAPPING_FIXED_PAIR,
    AARCH64_TRAPPING_SHIFT,
};
pub use register_model::{
    aarch64_aapcs64_mixed_unit_call_keys, aarch64_darwin_mixed_unit_call_keys,
};
mod selected_form_encoding;
pub const ORDINARY_DIRECT_CALL_BYTES: [u8; 4] = 0x9400_0000_u32.to_le_bytes();
pub const ORDINARY_RETURN_BYTES: [u8; 4] = 0xd65f_03c0_u32.to_le_bytes();
pub use floating_control::{
    encode_restore_fpcr_from_sp_displacement, encode_save_fpcr_to_sp_displacement,
};
pub use frame_protocol::{
    Aarch64FrameProtocolError, Aarch64FrameSlot, Aarch64StackProbe, encode_aapcs64_frame_protocol,
    encode_aarch64_aligned_frame_address, encode_aarch64_frame_address,
    encode_aarch64_frame_memory, encode_aarch64_integer_constant, encode_aarch64_pointer_memory,
};
pub use machine_effects::{
    Aarch64MachineEffectCatalogValidationError, aarch64_machine_effect_catalog,
    aarch64_normalized_foreign_call_declaration, aarch64_selected_constraint_keys,
    validate_aarch64_machine_effect_catalog,
};
pub use post_handoff_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
pub use preservation_storage::aarch64_preservation_storage_catalog;

#[cfg(test)]
pub(crate) use register_model::AARCH64_REQUIRED_REGISTER_CONSTRAINTS;
pub use register_model::aarch64_aapcs64_normalized_foreign_call_keys;
pub use register_model::aarch64_aapcs64_register_call_keys;
pub use register_model::aarch64_darwin_normalized_foreign_call_keys;
pub use register_model::aarch64_darwin_register_call_keys;
pub use register_model::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_BYTE_SEQUENCES, AARCH64_COMPARE_I64,
    AARCH64_COMPARE_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_BYTES, AARCH64_COPY_I64, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT, AARCH64_JUMP, AARCH64_MATERIALIZE_I64, AARCH64_SCAN_BYTE_SEQUENCE,
    AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE, AARCH64_ZERO_BYTES,
    Aarch64RegisterConstraintCatalogValidationError, aarch64_fixed_register_view,
    aarch64_physical_register_model, aarch64_preservation_convention_for_target,
    aarch64_register_constraint_catalog, prepare_aarch64_register_constraint_catalog,
    validate_aarch64_register_constraint_catalog, validated_aarch64_physical_register_model,
};
pub use register_model::{
    AARCH64_ADDRESS_OFFSET, AARCH64_BYTE_LITERAL_IMAGE_ADDRESS,
    AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32, AARCH64_FRAME_ADDRESS, AARCH64_HOSTED_WRITE_BYTE_I32,
    AARCH64_LOAD_PACKED, AARCH64_LOAD8_INDEXED, AARCH64_LOAD64, AARCH64_STORE,
    AARCH64_STORE_PACKED, AARCH64_STORE64,
};
pub use register_model::{
    AARCH64_FLOAT_RESULT_SATURATING, AARCH64_FLOAT_RESULT_TRAPPING, AARCH64_IEEE_FLOAT_ARITHMETIC,
    AARCH64_IEEE_FLOAT_TERNARY, AARCH64_IEEE_FLOAT_UNARY, AARCH64_RESTORE_FLOATING_CONTROL,
    AARCH64_SAVE_FLOATING_CONTROL,
};
pub use register_model::{
    aarch64_aapcs64_register_unit_call_keys, aarch64_darwin_register_unit_call_keys,
};
pub use register_model::{
    aarch64_mixed_aggregate_call_keys, aarch64_register_aggregate_call_keys,
    aarch64_register_aggregate_return_keys,
};
pub(crate) use register_model::{
    aarch64_physical_register_model_ref, aarch64_register_constraint_catalog_for,
    canonical_aarch64_physical_register_model_identity,
};
pub use selected_form_encoding::hosted_read_byte::{
    decode_aarch64_selected_hosted_read_byte, encode_aarch64_selected_hosted_read_byte_form,
    validate_aarch64_selected_hosted_read_byte_form,
};
pub use selected_form_encoding::hosted_write_byte::{
    aarch64_hosted_write_byte_i32_byte_count, decode_aarch64_selected_hosted_write_byte_i32,
};
pub use selected_form_encoding::system_call::{
    encode_aarch64_selected_system_call_form, validate_aarch64_selected_system_call_form,
};
pub use selected_form_encoding::{
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, AARCH64_CONDITIONAL_BRANCH_BYTE_REACH,
    Aarch64CallbackAddressMaterialization, Aarch64CallbackMaterializationDestination,
    Aarch64ConditionalBranchForm, Aarch64NormalizedForeignCallFixup,
    Aarch64NormalizedForeignCallFixupKind, Aarch64NormalizedForeignCallFixupState,
    Aarch64NormalizedForeignCallTemplateError, Aarch64ScalarCallFixup, Aarch64ScalarCallFixupKind,
    Aarch64ScalarCallFixupState, Aarch64ScalarCallTemplateError, Aarch64SelectedFormEncodingError,
    ValidatedAarch64SelectedFormEncoding,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_conditional_branch_form, encode_aarch64_selected_dynamic_call_template,
    encode_aarch64_selected_form, encode_aarch64_selected_jump_form,
    encode_aarch64_selected_normalized_foreign_call_template,
    encode_aarch64_selected_scalar_call_template,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_conditional_branch_form, validate_aarch64_selected_form_encoding,
    validate_aarch64_selected_jump_form,
    validate_aarch64_selected_normalized_foreign_call_template,
    validate_aarch64_selected_scalar_call_template,
};

pub use selected_form_encoding::encode_aarch64_selected_floating_control_form;
pub use selected_form_encoding::{
    encode_aarch64_selected_table_call_form, validate_aarch64_selected_table_call_form,
};

pub use selected_form_encoding::{
    encode_aarch64_selected_frame_form, encode_aarch64_selected_memory_form,
    validate_aarch64_selected_frame_form, validate_aarch64_selected_memory_form,
};
pub use selected_form_encoding::{
    encode_aarch64_selected_hosted_write_byte_form,
    validate_aarch64_selected_hosted_write_byte_form,
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

pub use register_model::{AARCH64_ACCESSOR_READ, AARCH64_ACCESSOR_WRITE};
pub use register_model::{AARCH64_SYSTEM_REGISTER_READ, AARCH64_SYSTEM_REGISTER_WRITE};

pub use selected_form_encoding::callback_group_bytes as encode_private_callback_materialization;
