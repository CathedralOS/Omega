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
    ValidatedX86_64SelectedStructuralUnitCallTemplate,
    X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
    X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
    X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT,
    X86_64ResolvedStructuralUnitInternalControlFixup, X86_64SelectedFormEncodingError,
    X86_64SelectedFormFootprint, X86_64SelectedStructuralUnitCallFootprint,
    X86_64StructuralUnitArgumentPointerWrite, X86_64StructuralUnitCallTemplateError,
    X86_64StructuralUnitCallerCopyWrite, X86_64StructuralUnitInternalControlFixup,
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
    X86_64StructuralUnitInternalControlResolutionError,
    X86_64StructuralUnitInternalControlResolutionState, X86_64StructuralUnitRootRead,
    encode_x86_64_selected_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_structural_unit_call_template,
    encode_x86_64_selected_u64_less_than_branch_form, resolve_x86_64_structural_unit_internal_call,
    validate_x86_64_resolved_structural_unit_internal_call, validate_x86_64_selected_form_encoding,
    validate_x86_64_selected_nonzero_branch_form,
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
    }
}
