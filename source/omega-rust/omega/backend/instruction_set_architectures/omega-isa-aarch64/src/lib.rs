//! Clean AArch64 encoders owned by the terminal-Psi realization lane.
//!
//! Only normalized target and terminal-installation facts enter this crate;
//! source-shaped representations and legacy operation graphs are absent.

mod machine_effects;
mod native_fuel_runtime;
mod post_handoff_writer;
mod ranked_u32_countdown;
mod register_model;
mod selected_form_encoding;
pub use machine_effects::{
    Aarch64MachineEffectCatalogValidationError, aarch64_machine_effect_catalog,
    validate_aarch64_machine_effect_catalog,
};
pub use native_fuel_runtime::{
    Aarch64NativeFuelTransferRuntimeEncoding, encode_native_fuel_transfer_runtime,
};
pub use post_handoff_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
pub use ranked_u32_countdown::*;
pub use register_model::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_I64, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT,
    AARCH64_INLINE_ASSEMBLY_DEFAULT, AARCH64_LINUX_SYSTEM_CALL, AARCH64_MATERIALIZE_I64,
    AARCH64_REQUIRED_REGISTER_CONSTRAINTS, AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
    Aarch64RegisterConstraintCatalogValidationError, aarch64_fixed_register_view,
    aarch64_physical_register_model, aarch64_preservation_convention_for_target,
    aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
};
pub use selected_form_encoding::{
    Aarch64MovkPatch, Aarch64MovnSeed, Aarch64SelectedFormEncodingError,
    Aarch64SelectedFormFootprint, Aarch64ShortestMovnMaterializationRecipe,
    ValidatedAarch64SelectedFormEncoding, aarch64_shortest_movn_materialization_recipe,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_form, encode_aarch64_selected_nonzero_branch_form,
    encode_aarch64_shortest_movn_materialization,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_form_encoding, validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_shortest_movn_materialization,
};

use omega_calling_conventions::{MachineRegister, RegisterSet};
use omega_installation_evidence::{
    FuelAttributionSite, NativeFuelTargetPlanProjection, SponsorContextTransport,
};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

pub const AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT: usize = 36;
/// Offset from charge start to the `B.LO` instruction address.
pub const AARCH64_NATIVE_FUEL_FAILURE_BRANCH_OFFSET: usize = 24;
pub const AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT: usize = 88;

pub fn native_fuel_charge_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
}

pub fn native_fuel_cold_dispatch_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16)])
}

/// Exact import-free Linux AArch64 realization of `exit_process(i32)`.
pub fn encode_linux_exit_group_i32(value: i32) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    append_unsigned_immediate(&mut bytes, 0, i64::from(value) as u64);
    append_unsigned_immediate(&mut bytes, 8, 94);
    bytes.extend(encode_svc(0));
    bytes.extend(encode_brk(0));
    Ok(bytes)
}

/// Import-free Linux `write_line` over one immutable literal.
pub fn encode_linux_write_line_literal(
    literal: &[u8],
) -> Result<(Vec<u8>, std::ops::Range<usize>), Diagnostic> {
    let payload_len = literal
        .len()
        .checked_add(1)
        .and_then(|len| u64::try_from(len).ok())
        .ok_or_else(|| Diagnostic::error("Linux AArch64 write_line literal is too large"))?;
    if payload_len >= (1 << 20) {
        return Err(Diagnostic::error(
            "Linux AArch64 write_line literal exceeds the PC-relative carrier",
        ));
    }
    let mut bytes = Vec::new();
    let adr_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // adr x1, data
    append_unsigned_immediate(&mut bytes, 2, payload_len);
    bytes.extend(encode_movz(8, 64)); // x8 = SYS_write
    let loop_offset = bytes.len();
    bytes.extend(encode_movz(0, 1)); // x0 = STDOUT_FILENO
    bytes.extend(encode_svc(0));
    bytes.extend(encode_compare_x_immediate(0, 0)?);
    let trap_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // b.le trap
    bytes.extend(encode_add_x_register(1, 1, 0));
    bytes.extend(encode_subs_x_register(2, 2, 0));
    let loop_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // b.ne loop
    let data_skip_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // b after_data
    let trap_offset = bytes.len();
    bytes.extend(encode_brk(0));
    let data_offset = bytes.len();
    bytes.extend_from_slice(literal);
    bytes.push(b'\n');
    let data_end = bytes.len();
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    let after_data = bytes.len();

    let adr_distance = i32::try_from(data_offset as i128 - adr_offset as i128)
        .map_err(|_| Diagnostic::error("Linux AArch64 write_line ADR is out of range"))?;
    if !(-(1 << 20)..(1 << 20)).contains(&adr_distance) {
        return Err(Diagnostic::error(
            "Linux AArch64 write_line ADR is out of range",
        ));
    }
    let immediate = adr_distance as u32 & 0x1f_ffff;
    let adr = 0x1000_0000 | ((immediate & 0x3) << 29) | (((immediate >> 2) & 0x7ffff) << 5) | 1;
    bytes[adr_offset..adr_offset + 4].copy_from_slice(&adr.to_le_bytes());
    bytes[trap_branch_offset..trap_branch_offset + 4].copy_from_slice(
        &encode_conditional_branch_less_or_equal(
            isize::try_from(trap_offset).unwrap() - isize::try_from(trap_branch_offset).unwrap(),
        )?,
    );
    bytes[loop_branch_offset..loop_branch_offset + 4].copy_from_slice(
        &encode_conditional_branch_not_equal(
            isize::try_from(loop_offset).unwrap() - isize::try_from(loop_branch_offset).unwrap(),
        )?,
    );
    bytes[data_skip_offset..data_skip_offset + 4].copy_from_slice(&encode_unconditional_branch(
        isize::try_from(after_data).unwrap() - isize::try_from(data_skip_offset).unwrap(),
    )?);
    Ok((bytes, data_offset..data_end))
}

/// Encode one exact compare-before-charge sequence.
pub fn encode_native_fuel_charge(
    plan: &NativeFuelTargetPlanProjection,
    required_units: u64,
    cold_failure_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    validate_aarch64_plan(plan)?;
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel charge requires a nonzero logical-unit cost",
        ));
    }
    let remaining = plan.context.remaining_units_offset as usize;

    let mut bytes = Vec::with_capacity(AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    bytes.extend(encode_load_x_from_x(16, 28, remaining)?);
    append_unsigned_immediate_padded(&mut bytes, 17, required_units);
    bytes.extend(encode_compare_x_register(16, 17));
    bytes.extend(encode_conditional_branch_lower(cold_failure_distance)?);
    bytes.extend(encode_sub_x_register(16, 16, 17));
    bytes.extend(encode_store_x_to_x(16, 28, remaining)?);
    debug_assert_eq!(bytes.len(), AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT);
    Ok(bytes)
}

/// Record one unpaid semantic site and tail-branch through the admitted
/// transfer entry without exposing a source-visible continuation.
pub fn encode_native_fuel_cold_dispatch(
    plan: &NativeFuelTargetPlanProjection,
    site: FuelAttributionSite,
    required_units: u64,
    retry_code_offset: u64,
) -> Result<Vec<u8>, Diagnostic> {
    validate_aarch64_plan(plan)?;
    if required_units == 0 {
        return Err(Diagnostic::error(
            "native fuel cold dispatch requires a nonzero logical-unit cost",
        ));
    }
    let (site_kind, site_identity) = match site {
        FuelAttributionSite::Operation(operation) => (0, operation.get()),
        FuelAttributionSite::Edge(edge) => (1, edge.get()),
    };
    let mut bytes = Vec::with_capacity(AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
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
    bytes.extend(encode_load_x_from_x(
        16,
        28,
        plan.context.transfer_entry_offset as usize,
    )?);
    bytes.extend((0xD61F0000_u32 | (16_u32 << 5)).to_le_bytes()); // br x16
    debug_assert_eq!(bytes.len(), AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
    Ok(bytes)
}

fn validate_aarch64_plan(plan: &NativeFuelTargetPlanProjection) -> Result<(), Diagnostic> {
    if plan.target.architecture != Architecture::Aarch64
        || !matches!(
            plan.transport,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28)
            }
        )
    {
        return Err(Diagnostic::error(
            "AArch64 native fuel charging requires the admitted X28 context transport",
        ));
    }
    if plan.profile.native_target() != plan.target {
        return Err(Diagnostic::error(
            "AArch64 native fuel charging rejects target-profile drift",
        ));
    }
    Ok(())
}

fn append_context_u64_store(
    bytes: &mut Vec<u8>,
    byte_offset: u32,
    value: u64,
) -> Result<(), Diagnostic> {
    append_unsigned_immediate_padded(bytes, 16, value);
    bytes.extend(encode_store_x_to_x(16, 28, byte_offset as usize)?);
    Ok(())
}

fn instruction(word: u32) -> [u8; 4] {
    word.to_le_bytes()
}

fn encode_movz(register: u8, immediate: u16) -> [u8; 4] {
    instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
}

fn encode_movk(register: u8, immediate: u16, halfword_shift: u8) -> [u8; 4] {
    instruction(
        0xF2800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

fn append_unsigned_immediate(bytes: &mut Vec<u8>, register: u8, value: u64) {
    bytes.extend(encode_movz(register, halfword(value, 0)));
    for halfword_shift in 1..4 {
        let immediate = halfword(value, halfword_shift);
        if immediate != 0 {
            bytes.extend(encode_movk(register, immediate, halfword_shift));
        }
    }
}

fn append_unsigned_immediate_padded(bytes: &mut Vec<u8>, register: u8, value: u64) {
    bytes.extend(encode_movz(register, halfword(value, 0)));
    for halfword_shift in 1..4 {
        bytes.extend(encode_movk(
            register,
            halfword(value, halfword_shift),
            halfword_shift,
        ));
    }
}

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}

fn encode_svc(immediate: u16) -> [u8; 4] {
    instruction(0xD4000001 | (u32::from(immediate) << 5))
}

fn encode_brk(immediate: u16) -> [u8; 4] {
    instruction(0xD4200000 | (u32::from(immediate) << 5))
}

fn encode_compare_x_immediate(register: u8, value: u32) -> Result<[u8; 4], Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare value `{value}` yet"
        )));
    }
    Ok(instruction(
        0xF100001F | (value << 10) | (u32::from(register) << 5),
    ))
}

fn encode_compare_x_register(left_register: u8, right_register: u8) -> [u8; 4] {
    instruction(0xEB00001F | (u32::from(right_register) << 16) | (u32::from(left_register) << 5))
}

fn encode_add_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    instruction(
        0x8B000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

fn encode_subs_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    instruction(
        0xEB000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

fn encode_sub_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    instruction(
        0xCB000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

fn encode_store_x_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store u64 at offset `{byte_offset}` yet"
        )));
    }
    Ok(instruction(
        0xF9000000
            | (((byte_offset / 8) as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

fn encode_load_x_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095 {
        return Ok(instruction(
            0xF9400000
                | (((byte_offset / 8) as u32) << 10)
                | (u32::from(base_register) << 5)
                | u32::from(destination_register),
        ));
    }
    if byte_offset <= 255 {
        return Ok(instruction(
            0xF8400000
                | ((byte_offset as u32) << 12)
                | (u32::from(base_register) << 5)
                | u32::from(destination_register),
        ));
    }
    Err(Diagnostic::error(format!(
        "AArch64 MVP encoder cannot load u64 from x{base_register} at offset `{byte_offset}` yet"
    )))
}

fn encode_conditional_branch_not_equal(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_conditional_branch(byte_distance, 0x1, "b.ne")
}

fn encode_conditional_branch_less_or_equal(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_conditional_branch(byte_distance, 0xd, "b.le")
}

fn encode_conditional_branch_lower(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_conditional_branch(byte_distance, 0x3, "b.lo")
}

fn encode_conditional_branch(
    byte_distance: isize,
    condition: u32,
    instruction_name: &str,
) -> Result<[u8; 4], Diagnostic> {
    let distance = checked_instruction_distance(byte_distance, 19, instruction_name)?;
    Ok(instruction(
        0x54000000 | ((distance as u32 & 0x7ffff) << 5) | condition,
    ))
}

fn encode_unconditional_branch(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    let distance = checked_instruction_distance(byte_distance, 26, "b")?;
    Ok(instruction(0x14000000 | (distance as u32 & 0x03ff_ffff)))
}

fn checked_instruction_distance(
    byte_distance: isize,
    immediate_bits: u8,
    instruction_name: &str,
) -> Result<isize, Diagnostic> {
    if byte_distance % 4 != 0 {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is not instruction aligned: {byte_distance} byte(s)"
        )));
    }
    let distance = byte_distance / 4;
    let min = -(1isize << (immediate_bits - 1));
    let max = (1isize << (immediate_bits - 1)) - 1;
    if distance < min || distance > max {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is out of range: {distance} instruction(s)"
        )));
    }
    Ok(distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_installation_evidence::NativeFuelContextLayout;
    use omega_target::TargetProfile;

    fn plan() -> NativeFuelTargetPlanProjection {
        NativeFuelTargetPlanProjection {
            profile: TargetProfile::LinuxArm64,
            target: TargetProfile::LinuxArm64.native_target(),
            transport: SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28),
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
        let bytes = encode_linux_exit_group_i32(37).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..4], &0xd280_04a0_u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &0xd280_0bc8_u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &0xd400_0001_u32.to_le_bytes());
        assert_eq!(&bytes[12..16], &0xd420_0000_u32.to_le_bytes());

        let (bytes, data) = encode_linux_write_line_literal(&[0, 0x80, 0xff]).unwrap();
        assert_eq!(&bytes[data.clone()], &[0, 0x80, 0xff, b'\n']);
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(
            &bytes[data.start - 4..data.start],
            &0xd420_0000_u32.to_le_bytes()
        );
    }

    #[test]
    fn native_fuel_charge_bytes_and_fail_closed_checks_are_preserved() {
        let bytes = encode_native_fuel_charge(&plan(), u64::MAX, -36).unwrap();
        assert_eq!(bytes.len(), AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT);
        assert_eq!(&bytes[24..28], &0x54ff_fee3_u32.to_le_bytes());

        assert!(encode_native_fuel_charge(&plan(), 0, 0).is_err());
        let mut wrong_transport = plan();
        wrong_transport.transport = SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::Aarch64X(27),
        };
        assert!(encode_native_fuel_charge(&wrong_transport, 1, 0).is_err());
        let mut large_offset = plan();
        large_offset.context.remaining_units_offset = u32::MAX;
        assert!(encode_native_fuel_charge(&large_offset, 1, 0).is_err());
    }

    #[test]
    fn native_fuel_cold_dispatch_records_exact_site_and_tail_branch() {
        let cold = encode_native_fuel_cold_dispatch(
            &plan(),
            FuelAttributionSite::Operation(psi_core::OperationId::new(9).unwrap()),
            u64::MAX,
            0x1020,
        )
        .unwrap();
        assert_eq!(cold.len(), AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT);
        let mut kind = Vec::new();
        append_unsigned_immediate_padded(&mut kind, 16, 0);
        kind.extend(encode_store_x_to_x(16, 28, 32).unwrap());
        assert_eq!(&cold[0..20], kind);
        let mut identity = Vec::new();
        append_unsigned_immediate_padded(&mut identity, 16, 9);
        identity.extend(encode_store_x_to_x(16, 28, 40).unwrap());
        assert_eq!(&cold[20..40], identity);
        assert_eq!(&cold[80..84], &encode_load_x_from_x(16, 28, 56).unwrap());
        assert_eq!(&cold[84..], &0xd61f_0200_u32.to_le_bytes());

        let edge = encode_native_fuel_cold_dispatch(
            &plan(),
            FuelAttributionSite::Edge(psi_core::EdgeId::new(9).unwrap()),
            1,
            0,
        )
        .unwrap();
        let mut edge_kind = Vec::new();
        append_unsigned_immediate_padded(&mut edge_kind, 16, 1);
        assert_eq!(&edge[0..16], edge_kind);
    }
}
