use omega_core::diagnostics::Diagnostic;
use omega_target_operations::StateGuardOperator;

use super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_compare_w17_immediate,
    encode_compare_w19_immediate, encode_compare_x17_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_greater, encode_conditional_branch_greater_or_equal,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_not_equal, encode_load_w17_from_x16, encode_load_x_from_x,
    encode_movz_w, encode_unconditional_branch,
};
use super::widths::{dispatch_guard_compare_static_width, dispatch_state_write_width};

pub fn encode_dispatch_loop_enter(entry_dispatch_index: u32) -> Result<Vec<u8>, Diagnostic> {
    let immediate = u16::try_from(entry_dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{entry_dispatch_index}` yet"
        ))
    })?;
    Ok(Vec::from(encode_movz_w(19, immediate)))
}

pub fn encode_dispatch_case_enter(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(8);
    bytes.extend(encode_compare_w19_immediate(dispatch_index)?);
    bytes.extend(encode_conditional_branch_not_equal(skip_byte_distance)?);
    Ok(bytes)
}

pub fn encode_dispatch_state_write(
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let immediate = u16::try_from(dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{dispatch_index}` yet"
        ))
    })?;
    let mut bytes = Vec::with_capacity(dispatch_state_write_width());
    bytes.extend(encode_movz_w(19, immediate));
    bytes.extend(encode_unconditional_branch(case_leave_byte_distance)?);
    Ok(bytes)
}

pub fn encode_dispatch_case_leave(loop_byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    Ok(Vec::from(encode_unconditional_branch(loop_byte_distance)?))
}

pub fn encode_dispatch_guard_compare_static(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width());
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_size {
        1 | 4 => {
            let expected_value = u32::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative guard value `{expected_value}` yet"
                ))
            })?;
            bytes.extend(encode_load_w17_from_x16(byte_offset, byte_size)?);
            bytes.extend(encode_compare_w17_immediate(expected_value)?);
        }
        8 => {
            let expected_value = u64::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative guard value `{expected_value}` yet"
                ))
            })?;
            bytes.extend(encode_load_x_from_x(17, 16, byte_offset)?);
            bytes.extend(encode_compare_x17_immediate(expected_value)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
            )));
        }
    }
    bytes.extend(match operator {
        StateGuardOperator::Equal => encode_conditional_branch_equal(skip_byte_distance)?,
        StateGuardOperator::NotEqual => encode_conditional_branch_not_equal(skip_byte_distance)?,
        StateGuardOperator::Greater => encode_conditional_branch_greater(skip_byte_distance)?,
        StateGuardOperator::GreaterOrEqual => {
            encode_conditional_branch_greater_or_equal(skip_byte_distance)?
        }
        StateGuardOperator::Less => encode_conditional_branch_less(skip_byte_distance)?,
        StateGuardOperator::LessOrEqual => {
            encode_conditional_branch_less_or_equal(skip_byte_distance)?
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot lower dispatch guard operator `{operator:?}` yet"
            )));
        }
    });
    Ok(bytes)
}
