use omega_core::diagnostics::Diagnostic;
use omega_target_operations::StateGuardOperator;

use super::primitives::{
    append_add_x_constant, encode_add_page_offset_placeholder, encode_adrp_placeholder,
    encode_compare_w_immediate, encode_compare_w17_immediate, encode_compare_x17_immediate,
    encode_conditional_branch_equal, encode_conditional_branch_greater,
    encode_conditional_branch_greater_or_equal, encode_conditional_branch_less,
    encode_conditional_branch_less_or_equal, encode_conditional_branch_not_equal,
    encode_load_w_from_x, encode_load_x_from_x, encode_move_x_register, encode_movz_w,
    encode_unconditional_branch,
};
use super::widths::dispatch_guard_compare_static_width;

const DISPATCH_STATE_REGISTER: u8 = 28;

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
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if is_float {
        return Err(Diagnostic::error(
            "aarch64 float dispatch-guard comparison is not implemented yet".to_string(),
        ));
    }
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width(byte_offset, byte_size));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_size {
        1 | 4 => {
            let expected_value = u32::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative guard value `{expected_value}` yet"
                ))
            })?;
            append_guard_load(&mut bytes, byte_offset, byte_size)?;
            bytes.extend(encode_compare_w17_immediate(expected_value)?);
        }
        8 => {
            let expected_value = u64::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative guard value `{expected_value}` yet"
                ))
            })?;
            append_guard_load(&mut bytes, byte_offset, byte_size)?;
            bytes.extend(encode_compare_x17_immediate(expected_value)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
            )));
        }
    }
    bytes.extend(match operator {
        StateGuardOperator::Equal => encode_conditional_branch_not_equal(skip_byte_distance)?,
        StateGuardOperator::NotEqual => encode_conditional_branch_equal(skip_byte_distance)?,
        StateGuardOperator::Greater => encode_conditional_branch_less_or_equal(skip_byte_distance)?,
        StateGuardOperator::GreaterOrEqual => encode_conditional_branch_less(skip_byte_distance)?,
        StateGuardOperator::Less => encode_conditional_branch_greater_or_equal(skip_byte_distance)?,
        StateGuardOperator::LessOrEqual => encode_conditional_branch_greater(skip_byte_distance)?,
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot lower dispatch guard operator `{operator:?}` yet"
            )));
        }
    });
    debug_assert_eq!(
        bytes.len(),
        dispatch_guard_compare_static_width(byte_offset, byte_size)
    );
    Ok(bytes)
}

fn append_guard_load(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let direct = match byte_size {
        1 => byte_offset <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    };

    let base_register = if direct {
        16
    } else {
        bytes.extend(encode_move_x_register(18, 16));
        append_add_x_constant(bytes, 18, 18, byte_offset, 19)?;
        18
    };

    match byte_size {
        1 | 4 => bytes.extend(encode_load_w_from_x(
            17,
            base_register,
            if direct { byte_offset } else { 0 },
            byte_size,
        )?),
        8 => bytes.extend(encode_load_x_from_x(
            17,
            base_register,
            if direct { byte_offset } else { 0 },
        )?),
        _ => unreachable!("dispatch guard byte_size was validated by caller"),
    }

    Ok(())
}

fn two_instructions(first: [u8; 4], second: [u8; 4]) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&first);
    bytes[4..].copy_from_slice(&second);
    bytes
}
