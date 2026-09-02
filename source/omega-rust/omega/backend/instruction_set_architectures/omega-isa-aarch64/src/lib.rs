//! Clean AArch64 encoders owned by the terminal-Psi realization lane.
//!
//! Only normalized target and terminal-installation facts enter this crate;
//! source-shaped representations and legacy operation graphs are absent.

mod floating_control;
mod machine_effects;
mod post_handoff_writer;
mod ranked_u32_countdown;
mod register_model;
mod selected_form_encoding;
pub use floating_control::{
    encode_restore_fpcr_from_sp_displacement, encode_save_fpcr_to_sp_displacement,
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
pub use ranked_u32_countdown::*;
pub use register_model::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
    AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64,
    AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_DARWIN_CALL,
    AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT, AARCH64_INLINE_ASSEMBLY_DEFAULT,
    AARCH64_LINUX_SYSTEM_CALL, AARCH64_MATERIALIZE_I64, AARCH64_REQUIRED_REGISTER_CONSTRAINTS,
    AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
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
    encode_aarch64_selected_u64_less_than_branch_form,
    encode_aarch64_shortest_movn_materialization,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_form_encoding, validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_u64_less_than_branch_form,
    validate_aarch64_shortest_movn_materialization,
};

use psi_diagnostics::Diagnostic;

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

fn encode_conditional_branch_not_equal(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_conditional_branch(byte_distance, 0x1, "b.ne")
}

fn encode_conditional_branch_less_or_equal(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_conditional_branch(byte_distance, 0xd, "b.le")
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
}
