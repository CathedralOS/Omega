//! Clean x86-64 encoders owned by the terminal-Psi realization lane.
//!
//! This crate deliberately consumes only normalized target and terminal
//! installation facts. It does not depend on either legacy operation graph or
//! any source-shaped Psi representation.

mod fma;
mod ieee_float;
mod machine_effects;
mod mov_r32_imm32_i64_materialization;
mod mov_r64_imm32_sign_extended_i64_materialization;
mod post_handoff_writer;
mod preservation_storage;
mod ranked_u32_countdown;
mod register_model;
mod selected_form_encoding;
mod semantic_unit_wrapper_encoding;
mod xor_zero_i64_materialization;

pub use fma::{
    DecodedScalarFmaFormat, DecodedVfmadd132Scalar, decode_vfmadd132_scalar, encode_vfmadd132sd,
    encode_vfmadd132ss,
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
pub use mov_r32_imm32_i64_materialization::{
    ValidatedX86_64MovR32Imm32I64Materialization, X86_64DecodedMovR32Imm32I64Materialization,
    X86_64MovR32Imm32I64MaterializationError, X86_64MovR32Imm32I64MaterializationFootprint,
    decode_x86_64_mov_r32_imm32_i64_materialization,
    encode_x86_64_mov_r32_imm32_i64_materialization,
    validate_x86_64_mov_r32_imm32_i64_materialization,
};
pub use mov_r64_imm32_sign_extended_i64_materialization::{
    ValidatedX86_64MovR64Imm32SignExtendedI64Materialization,
    X86_64DecodedMovR64Imm32SignExtendedI64Materialization,
    X86_64MovR64Imm32SignExtendedI64MaterializationError,
    X86_64MovR64Imm32SignExtendedI64MaterializationFootprint,
    decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization,
    encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization,
    validate_x86_64_mov_r64_imm32_sign_extended_i64_materialization,
};
pub use post_handoff_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
pub use preservation_storage::{
    X86_64PreservationStorageCatalogError, x86_64_preservation_storage_catalog,
};
pub use ranked_u32_countdown::*;
pub use register_model::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_ZERO,
    X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_INLINE_ASSEMBLY_DEFAULT,
    X86_64_LINUX_SYSTEM_CALL, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL,
    X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_REQUIRED_REGISTER_CONSTRAINTS, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
    X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_fixed_register_view, x86_64_physical_register_model,
    x86_64_preservation_convention_for_target, x86_64_register_constraint_catalog,
};
pub use selected_form_encoding::{
    ValidatedX86_64ResolvedStructuralUnitCall, ValidatedX86_64SelectedFormEncoding,
    ValidatedX86_64SelectedScalarCallTemplate, ValidatedX86_64SelectedStructuralUnitCallTemplate,
    X86_64_SCALAR_CALL_OPCODE_OFFSET, X86_64_SCALAR_CALL_PATCH_OFFSET,
    X86_64_SCALAR_CALL_PATCH_WIDTH, X86_64_SCALAR_CALL_REFERENCE_OFFSET,
    X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT, X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
    X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
    X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH, X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT,
    X86_64ResolvedStructuralUnitInternalControlFixup, X86_64ScalarCallFixup,
    X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState, X86_64ScalarCallTemplateError,
    X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitArgumentPointerWrite,
    X86_64StructuralUnitCallTemplateError, X86_64StructuralUnitCallerCopyWrite,
    X86_64StructuralUnitInternalControlFixup, X86_64StructuralUnitInternalControlFixupKind,
    X86_64StructuralUnitInternalControlFixupState,
    X86_64StructuralUnitInternalControlResolutionError,
    X86_64StructuralUnitInternalControlResolutionState, X86_64StructuralUnitRootRead,
    encode_x86_64_selected_form, encode_x86_64_selected_i64_less_than_branch_form,
    encode_x86_64_selected_nonzero_branch_form, encode_x86_64_selected_scalar_call_template,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_structural_unit_call_template,
    encode_x86_64_selected_u64_less_than_branch_form, resolve_x86_64_structural_unit_internal_call,
    validate_x86_64_resolved_structural_unit_internal_call, validate_x86_64_selected_form_encoding,
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form, validate_x86_64_selected_scalar_call_template,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_structural_unit_call_template,
    validate_x86_64_selected_u64_less_than_branch_form,
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
pub use xor_zero_i64_materialization::{
    ValidatedX86_64XorZeroI64Materialization, X86_64DecodedXorZeroI64Materialization,
    decode_x86_64_xor_zero_i64_materialization, encode_x86_64_xor_zero_i64_materialization,
    validate_x86_64_xor_zero_i64_materialization,
};

use psi_diagnostics::Diagnostic;

/// Exact import-free Linux x86-64 realization of `exit_process(i32)`.
pub fn encode_linux_exit_group_i32(value: i32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(14);
    bytes.push(0xbf); // mov edi, imm32
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend_from_slice(&231_u32.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2 if exit_group returns
    bytes
}

/// Import-free Linux `write(1, &byte, 1)` realization. The caller places the
/// low byte of the exact `i32` source in `r11b`; this closed encoder owns the
/// private stack slot and traps if the kernel does not consume that byte.
pub fn encode_linux_write_byte_i32_from_r11() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44);
    bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x10]);
    bytes.extend_from_slice(&[0x44, 0x88, 0x1c, 0x24]);
    bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0x48, 0x89, 0xe6]);
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0xb8, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0x0f, 0x05, 0x48, 0x85, 0xc0]);
    bytes.extend_from_slice(&[0x7e, 0x06]);
    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]);
    bytes.extend_from_slice(&[0xeb, 0x02, 0x0f, 0x0b]);
    bytes
}

/// Import-free Linux `read(0, &byte, 1)` realization into one canonical
/// `ByteRead = Eof | Byte(i32)` stack home. The home is zeroed first, so an
/// exact zero-byte read leaves the ordinal-zero `Eof` value; an exact one-byte
/// read writes ordinal one after the kernel has filled the payload byte.
pub fn encode_linux_read_byte_to_stack(
    home_byte_offset: u32,
    payload_byte_offset: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(72);
    bytes.extend_from_slice(&[0x31, 0xc0]); // xor eax, eax
    for offset in [home_byte_offset, payload_byte_offset] {
        bytes.extend_from_slice(&[0x89, 0x84, 0x24]); // mov [rsp+disp32], eax
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(&[0x31, 0xff]); // xor edi, edi (stdin)
    bytes.extend_from_slice(&[0x48, 0x8d, 0xb4, 0x24]); // lea rsi, [rsp+disp32]
    bytes.extend_from_slice(&payload_byte_offset.to_le_bytes());
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0]); // mov edx, 1
    bytes.extend_from_slice(&[0x31, 0xc0]); // xor eax, eax (SYS_read)
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 0]); // cmp rax, 0
    let eof_branch = bytes.len();
    bytes.extend_from_slice(&[0x74, 0]); // je done
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 1]); // cmp rax, 1
    let trap_branch = bytes.len();
    bytes.extend_from_slice(&[0x75, 0]); // jne trap
    bytes.extend_from_slice(&[0xc7, 0x84, 0x24]); // mov dword [rsp+disp32], 1
    bytes.extend_from_slice(&home_byte_offset.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    let done_branch = bytes.len();
    bytes.extend_from_slice(&[0xeb, 0]); // jmp done
    let trap = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2
    let done = bytes.len();

    let rel8 = |target: usize, end: usize| {
        i8::try_from(target as i128 - end as i128)
            .map(|value| value as u8)
            .map_err(|_| Diagnostic::error("Linux x86-64 read-byte branch is out of range"))
    };
    bytes[eof_branch + 1] = rel8(done, eof_branch + 2)?;
    bytes[trap_branch + 1] = rel8(trap, trap_branch + 2)?;
    bytes[done_branch + 1] = rel8(done, done_branch + 2)?;
    Ok(bytes)
}

/// Import-free Linux `write_line` over one immutable literal.
pub fn encode_linux_write_line_literal(
    literal: &[u8],
) -> Result<(Vec<u8>, std::ops::Range<usize>), Diagnostic> {
    let payload_len = literal
        .len()
        .checked_add(1)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or_else(|| Diagnostic::error("Linux x86-64 write_line literal is too large"))?;
    let mut bytes = Vec::with_capacity(52 + payload_len as usize);
    bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0]); // mov edi, STDOUT_FILENO
    let lea_offset = bytes.len();
    bytes.extend_from_slice(&[0x48, 0x8d, 0x35, 0, 0, 0, 0]); // lea rsi, [rip+data]
    bytes.push(0xba); // mov edx, payload_len
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    let loop_offset = bytes.len();
    bytes.extend_from_slice(&[0xb8, 1, 0, 0, 0]); // mov eax, SYS_write on every retry
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x48, 0x85, 0xc0]); // test rax, rax
    let trap_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x8e, 0, 0, 0, 0]); // jle trap
    bytes.extend_from_slice(&[0x48, 0x01, 0xc6]); // add rsi, rax
    bytes.extend_from_slice(&[0x48, 0x29, 0xc2]); // sub rdx, rax
    let loop_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x85, 0, 0, 0, 0]); // jne loop
    let data_skip_offset = bytes.len();
    bytes.extend_from_slice(&[0xe9, 0, 0, 0, 0]); // jmp after_data
    let trap_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2
    let data_offset = bytes.len();
    bytes.extend_from_slice(literal);
    bytes.push(b'\n');
    let data_end = bytes.len();

    let relative = |target: usize, instruction_end: usize| -> Result<[u8; 4], Diagnostic> {
        i32::try_from(target as i128 - instruction_end as i128)
            .map(i32::to_le_bytes)
            .map_err(|_| Diagnostic::error("Linux x86-64 write_line branch is out of range"))
    };
    bytes[lea_offset + 3..lea_offset + 7].copy_from_slice(&relative(data_offset, lea_offset + 7)?);
    bytes[trap_branch_offset + 2..trap_branch_offset + 6]
        .copy_from_slice(&relative(trap_offset, trap_branch_offset + 6)?);
    bytes[loop_branch_offset + 2..loop_branch_offset + 6]
        .copy_from_slice(&relative(loop_offset, loop_branch_offset + 6)?);
    bytes[data_skip_offset + 1..data_skip_offset + 5]
        .copy_from_slice(&relative(data_end, data_skip_offset + 5)?);
    Ok((bytes, data_offset..data_end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_exit_and_write_literal_keep_exact_bytes() {
        assert_eq!(
            encode_linux_exit_group_i32(0x1234_5678),
            [
                0xbf, 0x78, 0x56, 0x34, 0x12, 0xb8, 0xe7, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x0f, 0x0b,
            ]
        );
        let (bytes, data) = encode_linux_write_line_literal(&[0, 0x80, 0xff]).unwrap();
        assert_eq!(&bytes[data.clone()], &[0, 0x80, 0xff, b'\n']);
        assert_eq!(&bytes[data.start - 2..data.start], &[0x0f, 0x0b]);
        let retry = bytes
            .windows(2)
            .position(|window| window == [0x0f, 0x85])
            .expect("retry branch");
        let displacement = i32::from_le_bytes(bytes[retry + 2..retry + 6].try_into().unwrap());
        let retry_target = i64::try_from(retry + 6).unwrap() + i64::from(displacement);
        let loop_start = bytes
            .windows(7)
            .position(|window| window == [0xb8, 1, 0, 0, 0, 0x0f, 0x05])
            .unwrap();
        assert_eq!(retry_target, i64::try_from(loop_start).unwrap());

        let byte_write = encode_linux_write_byte_i32_from_r11();
        assert_eq!(&byte_write[..4], &[0x48, 0x83, 0xec, 0x10]);
        assert_eq!(&byte_write[33..37], &[0x48, 0x83, 0xc4, 0x10]);
        assert_eq!(&byte_write[39..], &[0x0f, 0x0b]);
        let trap_target = 33_i64 + i64::from(byte_write[32] as i8);
        assert_eq!(trap_target, 39);
        let success_target = 39_i64 + i64::from(byte_write[38] as i8);
        assert_eq!(success_target, i64::try_from(byte_write.len()).unwrap());

        let byte_read = encode_linux_read_byte_to_stack(16, 20).unwrap();
        assert_eq!(&byte_read[..2], &[0x31, 0xc0]);
        assert_eq!(&byte_read[5..9], &16_u32.to_le_bytes());
        assert!(byte_read.windows(2).any(|window| window == [0x0f, 0x05]));
        assert_eq!(&byte_read[byte_read.len() - 2..], &[0x0f, 0x0b]);
    }
}
