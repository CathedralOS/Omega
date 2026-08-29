//! Clean x86-64 encoders owned by the terminal-Psi realization lane.
//!
//! This crate deliberately consumes only normalized target and terminal
//! installation facts. It does not depend on either legacy operation graph or
//! any source-shaped Psi representation.

mod fma;
mod machine_effects;
mod mov_r32_imm32_i64_materialization;
mod native_fuel_runtime;
mod native_fuel_validation;
mod post_handoff_writer;
mod ranked_u32_countdown;
mod register_model;
mod selected_form_encoding;
mod semantic_unit_wrapper_encoding;
mod xor_zero_i64_materialization;

pub use fma::{encode_vfmadd132sd, encode_vfmadd132ss};
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
pub use native_fuel_runtime::{
    X86NativeFuelTransferRuntimeEncoding, encode_native_fuel_transfer_runtime,
};
pub use native_fuel_validation::{
    X86NativeFuelValidationError, validate_x86_native_fuel_charge,
    validate_x86_native_fuel_cold_dispatch,
};
pub use post_handoff_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
pub use ranked_u32_countdown::*;
pub use register_model::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_LINUX_SYSTEM_CALL,
    X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_REQUIRED_REGISTER_CONSTRAINTS,
    X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_CALL,
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
    resolve_x86_64_structural_unit_internal_call,
    validate_x86_64_resolved_structural_unit_internal_call, validate_x86_64_selected_form_encoding,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_structural_unit_call_template,
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

use omega_calling_conventions::{MachineRegister, RegisterSet};
use omega_installation_evidence::{
    FuelAttributionSite, NativeFuelTargetPlanProjection, SponsorContextTransport,
};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

pub const X86_NATIVE_FUEL_CHARGE_BYTE_COUNT: usize = 36;
/// Offset from charge start to the PC immediately after `JB rel32`.
pub const X86_NATIVE_FUEL_FAILURE_BRANCH_END_OFFSET: usize = 26;
pub const X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT: usize = 78;

pub fn native_fuel_charge_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R11])
}

pub fn native_fuel_cold_dispatch_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R10])
}

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

/// Encode one exact compare-before-charge sequence.
pub fn encode_native_fuel_charge(
    plan: &NativeFuelTargetPlanProjection,
    required_units: u64,
    cold_failure_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    validate_x86_plan(plan)?;
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel charge requires a nonzero logical-unit cost",
        ));
    }
    let remaining = disp32(plan.context.remaining_units_offset as usize)?;

    let mut bytes = Vec::with_capacity(X86_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    bytes.extend([0x4c, 0x8b, 0x93]); // mov r10, [rbx + disp32]
    bytes.extend(remaining.to_le_bytes());
    append_mov_r11_imm64(&mut bytes, required_units);
    bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
    append_jcc_rel32(&mut bytes, 0x82, cold_failure_distance)?; // jb
    bytes.extend([0x4d, 0x29, 0xda]); // sub r10, r11
    bytes.extend([0x4c, 0x89, 0x93]); // mov [rbx + disp32], r10
    bytes.extend(remaining.to_le_bytes());
    debug_assert_eq!(bytes.len(), X86_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    Ok(bytes)
}

/// Record one unpaid semantic site and tail-jump through the admitted transfer
/// entry without exposing a source-visible continuation.
pub fn encode_native_fuel_cold_dispatch(
    plan: &NativeFuelTargetPlanProjection,
    site: FuelAttributionSite,
    required_units: u64,
    retry_code_offset: u64,
) -> Result<Vec<u8>, Diagnostic> {
    validate_x86_plan(plan)?;
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel cold dispatch requires a nonzero logical-unit cost",
        ));
    }
    let (site_kind, site_identity) = match site {
        FuelAttributionSite::Operation(operation) => (0, operation.get()),
        FuelAttributionSite::Edge(edge) => (1, edge.get()),
    };
    let mut bytes = Vec::with_capacity(X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
    append_context_u64_store(&mut bytes, plan.context.unpaid_site_kind_offset, site_kind)?;
    append_context_u64_store(
        &mut bytes,
        plan.context.unpaid_site_identity_offset,
        site_identity,
    )?;
    append_context_u64_store(
        &mut bytes,
        plan.context.required_units_offset,
        required_units,
    )?;
    append_context_u64_store(
        &mut bytes,
        plan.context.retry_code_offset_offset,
        retry_code_offset,
    )?;
    let transfer_entry = disp32(plan.context.transfer_entry_offset as usize)?;
    bytes.extend([0x4c, 0x8b, 0x93]); // mov r10, [rbx + disp32]
    bytes.extend(transfer_entry.to_le_bytes());
    bytes.extend([0x41, 0xff, 0xe2]); // jmp r10
    debug_assert_eq!(bytes.len(), X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
    Ok(bytes)
}

fn validate_x86_plan(plan: &NativeFuelTargetPlanProjection) -> Result<(), Diagnostic> {
    if plan.target.architecture != Architecture::X86_64
        || !matches!(
            plan.transport,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx
            }
        )
    {
        return Err(Diagnostic::error(
            "x86-64 native fuel charging requires the admitted RBX context transport",
        ));
    }
    if plan.profile.native_target() != plan.target {
        return Err(Diagnostic::error(
            "x86-64 native fuel charging rejects target-profile drift",
        ));
    }
    Ok(())
}

fn append_context_u64_store(
    bytes: &mut Vec<u8>,
    byte_offset: u32,
    value: u64,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset as usize)?;
    append_mov_r10_imm64(bytes, value);
    bytes.extend([0x4c, 0x89, 0x93]); // mov [rbx + disp32], r10
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_r10_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xba]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_r11_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbb]);
    bytes.extend(value.to_le_bytes());
}

fn append_jcc_rel32(
    bytes: &mut Vec<u8>,
    opcode: u8,
    byte_distance: isize,
) -> Result<(), Diagnostic> {
    let displacement = i32::try_from(byte_distance).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 branch target is out of rel32 range: {byte_distance} byte(s)"
        ))
    })?;
    bytes.extend([0x0f, opcode]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn disp32(value: usize) -> Result<i32, Diagnostic> {
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot address displacement `{value}`"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_installation_evidence::NativeFuelContextLayout;
    use omega_target::TargetProfile;

    fn plan() -> NativeFuelTargetPlanProjection {
        NativeFuelTargetPlanProjection {
            profile: TargetProfile::LinuxX64,
            target: TargetProfile::LinuxX64.native_target(),
            transport: SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
            context: NativeFuelContextLayout {
                byte_size: 256,
                alignment: 16,
                remaining_units_offset: 24,
                unpaid_site_kind_offset: 32,
                unpaid_site_identity_offset: 40,
                required_units_offset: 48,
                transfer_entry_offset: 56,
                retry_code_offset_offset: 64,
                sponsor_stack_top_offset: 72,
                activation_state_offset: 80,
                activation_state_byte_count: 176,
            },
            transfer_plan_identity: 1,
        }
    }

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

    #[test]
    fn native_fuel_charge_bytes_and_fail_closed_checks_are_preserved() {
        let bytes = encode_native_fuel_charge(&plan(), u64::MAX, -36).unwrap();
        assert_eq!(bytes.len(), X86_NATIVE_FUEL_CHARGE_BYTE_COUNT);
        assert_eq!(&bytes[0..3], &[0x4c, 0x8b, 0x93]);
        assert_eq!(&bytes[17..20], &[0x4d, 0x39, 0xda]);
        assert_eq!(&bytes[20..22], &[0x0f, 0x82]);
        assert_eq!(&bytes[26..29], &[0x4d, 0x29, 0xda]);

        assert!(encode_native_fuel_charge(&plan(), 0, 0).is_err());
        let mut wrong_transport = plan();
        wrong_transport.transport = SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::X86R15,
        };
        assert!(encode_native_fuel_charge(&wrong_transport, 1, 0).is_err());
        let mut large_offset = plan();
        large_offset.context.remaining_units_offset = u32::MAX;
        assert!(encode_native_fuel_charge(&large_offset, 1, 0).is_err());
    }

    #[test]
    fn native_fuel_cold_dispatch_records_exact_site_and_tail_jump() {
        let cold = encode_native_fuel_cold_dispatch(
            &plan(),
            FuelAttributionSite::Operation(psi_core::OperationId::new(9).unwrap()),
            u64::MAX,
            0x1020,
        )
        .unwrap();
        assert_eq!(cold.len(), X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
        assert_eq!(&cold[2..10], &0_u64.to_le_bytes());
        assert_eq!(&cold[19..27], &9_u64.to_le_bytes());
        assert_eq!(&cold[36..44], &u64::MAX.to_le_bytes());
        assert_eq!(&cold[53..61], &0x1020_u64.to_le_bytes());
        assert_eq!(&cold[75..], &[0x41, 0xff, 0xe2]);

        let edge = encode_native_fuel_cold_dispatch(
            &plan(),
            FuelAttributionSite::Edge(psi_core::EdgeId::new(9).unwrap()),
            1,
            0,
        )
        .unwrap();
        assert_eq!(&edge[2..10], &1_u64.to_le_bytes());
    }
}
