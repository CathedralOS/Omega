use omega_core::diagnostics::Diagnostic;
use omega_target_operations::StateGuardOperator;

use super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_compare_w_immediate,
    encode_compare_w17_immediate, encode_compare_x17_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_greater, encode_conditional_branch_greater_or_equal,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_not_equal, encode_load_w17_from_x16, encode_load_x_from_x,
    encode_movz_w, encode_unconditional_branch,
};
use super::widths::dispatch_guard_compare_static_width;

const DISPATCH_STATE_REGISTER: u8 = 26;

pub fn encode_dispatch_loop_enter_bytes(entry_dispatch_index: u32) -> Result<[u8; 4], Diagnostic> {
    let immediate = u16::try_from(entry_dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{entry_dispatch_index}` yet"
        ))
    })?;
    Ok(encode_movz_w(DISPATCH_STATE_REGISTER, immediate))
}

pub fn encode_dispatch_case_enter_bytes(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<[u8; 8], Diagnostic> {
    Ok(two_instructions(
        encode_compare_w_immediate(DISPATCH_STATE_REGISTER, dispatch_index)?,
        encode_conditional_branch_not_equal(skip_byte_distance)?,
    ))
}

pub fn encode_dispatch_state_write_bytes(
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<[u8; 8], Diagnostic> {
    let immediate = u16::try_from(dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{dispatch_index}` yet"
        ))
    })?;
    Ok(two_instructions(
        encode_movz_w(DISPATCH_STATE_REGISTER, immediate),
        encode_unconditional_branch(case_leave_byte_distance)?,
    ))
}

pub fn encode_dispatch_case_leave_bytes(loop_byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_unconditional_branch(loop_byte_distance)
}

pub fn encode_dispatch_guard_compare_static_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
) -> Result<[u8; 20], Diagnostic> {
    let mut bytes = [0; 20];
    let mut cursor = 0usize;
    append_instruction(&mut bytes, &mut cursor, encode_adrp_placeholder(16));
    append_instruction(
        &mut bytes,
        &mut cursor,
        encode_add_page_offset_placeholder(16),
    );
    match byte_size {
        1 | 4 => {
            let expected_value = u32::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative guard value `{expected_value}` yet"
                ))
            })?;
            append_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_w17_from_x16(byte_offset, byte_size)?,
            );
            append_instruction(
                &mut bytes,
                &mut cursor,
                encode_compare_w17_immediate(expected_value)?,
            );
        }
        8 => {
            let expected_value = u64::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative guard value `{expected_value}` yet"
                ))
            })?;
            append_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_x_from_x(17, 16, byte_offset)?,
            );
            append_instruction(
                &mut bytes,
                &mut cursor,
                encode_compare_x17_immediate(expected_value)?,
            );
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
            )));
        }
    }
    append_instruction(
        &mut bytes,
        &mut cursor,
        match operator {
            StateGuardOperator::Equal => encode_conditional_branch_not_equal(skip_byte_distance)?,
            StateGuardOperator::NotEqual => encode_conditional_branch_equal(skip_byte_distance)?,
            StateGuardOperator::Greater => {
                encode_conditional_branch_less_or_equal(skip_byte_distance)?
            }
            StateGuardOperator::GreaterOrEqual => {
                encode_conditional_branch_less(skip_byte_distance)?
            }
            StateGuardOperator::Less => {
                encode_conditional_branch_greater_or_equal(skip_byte_distance)?
            }
            StateGuardOperator::LessOrEqual => {
                encode_conditional_branch_greater(skip_byte_distance)?
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot lower dispatch guard operator `{operator:?}` yet"
                )));
            }
        },
    );
    debug_assert_eq!(cursor, dispatch_guard_compare_static_width());
    Ok(bytes)
}

fn append_instruction(bytes: &mut [u8], cursor: &mut usize, instruction: [u8; 4]) {
    bytes[*cursor..*cursor + 4].copy_from_slice(&instruction);
    *cursor += 4;
}

fn two_instructions(first: [u8; 4], second: [u8; 4]) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&first);
    bytes[4..].copy_from_slice(&second);
    bytes
}
