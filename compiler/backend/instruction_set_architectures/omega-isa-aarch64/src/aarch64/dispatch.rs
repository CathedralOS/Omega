use omega_core::diagnostics::Diagnostic;
use omega_target_operations::StateGuardOperator;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate_padded, append_unsigned_immediate_w_padded,
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_compare_w_immediate,
    encode_compare_w_register, encode_compare_x_register, encode_conditional_branch_equal,
    encode_conditional_branch_greater, encode_conditional_branch_greater_or_equal,
    encode_conditional_branch_higher, encode_conditional_branch_higher_or_same,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_lower, encode_conditional_branch_lower_or_same,
    encode_conditional_branch_not_equal, encode_float_compare, encode_float_move_from_gpr,
    encode_load_w_from_x, encode_load_x_from_x, encode_move_x_register, encode_movz_w,
    encode_sign_extend_byte_to_w, encode_sign_extend_halfword_to_w, encode_unconditional_branch,
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

/// Narrow a guard's float `expected_value` (stored as f64 bits) to the operand's
/// width: for a 4-byte float operand the comparison runs in single precision, so
/// the materialized bits must be the f32 bit pattern. Exact for any value
/// representable in f32 (which a constant compared against an f32 field always
/// is). Mirrors the x86_64 backend's `float_compare_expected_bits`.
fn float_compare_expected_bits(expected_value: i64, byte_size: usize) -> u64 {
    if byte_size == 4 {
        u64::from((f64::from_bits(expected_value as u64) as f32).to_bits())
    } else {
        expected_value as u64
    }
}

pub fn encode_dispatch_guard_compare_static_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width(
        byte_offset,
        byte_size,
        is_float,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if is_float {
        if !matches!(byte_size, 4 | 8) {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte float dispatch guards yet"
            )));
        }
        // Load the field's raw float bits, materialize the expected bits
        // (narrowed to f32 for 4-byte operands), move both into the FP bank and
        // FCMP. The NZCV flags then drive the same equal/unsigned skip branches
        // as the x86 `ucomis*` path (NaN handling is a documented first-cut
        // limitation there too).
        append_guard_load(&mut bytes, byte_offset, byte_size)?;
        let expected_bits = float_compare_expected_bits(expected_value, byte_size);
        if byte_size == 4 {
            append_unsigned_immediate_w_padded(&mut bytes, 18, expected_bits as u32);
        } else {
            append_unsigned_immediate_padded(&mut bytes, 18, expected_bits);
        }
        bytes.extend(encode_float_move_from_gpr(byte_size, 0, 17)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 1, 18)?);
        bytes.extend(encode_float_compare(byte_size, 0, 1)?);
    } else {
        // The expected value is materialized into a register (fixed-width, so
        // the width function stays magnitude-independent) and compared register
        // to register, which handles negative values and full-width patterns
        // that do not fit the 12-bit compare immediate. Narrow operands are
        // sign-extended on BOTH sides: exact for signed conditions and order-
        // preserving for unsigned ones (sign extension is monotone per width).
        match byte_size {
            1 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                bytes.extend(encode_sign_extend_byte_to_w(17, 17));
                append_unsigned_immediate_w_padded(
                    &mut bytes,
                    18,
                    expected_value as i8 as i32 as u32,
                );
                bytes.extend(encode_compare_w_register(17, 18));
            }
            2 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                bytes.extend(encode_sign_extend_halfword_to_w(17, 17));
                append_unsigned_immediate_w_padded(
                    &mut bytes,
                    18,
                    expected_value as i16 as i32 as u32,
                );
                bytes.extend(encode_compare_w_register(17, 18));
            }
            4 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                append_unsigned_immediate_w_padded(&mut bytes, 18, expected_value as u32);
                bytes.extend(encode_compare_w_register(17, 18));
            }
            8 => {
                append_guard_load(&mut bytes, byte_offset, byte_size)?;
                append_unsigned_immediate_padded(&mut bytes, 18, expected_value as u64);
                bytes.extend(encode_compare_x_register(17, 18));
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
                )));
            }
        }
    }
    bytes.extend(match operator {
        StateGuardOperator::Equal => encode_conditional_branch_not_equal(skip_byte_distance)?,
        StateGuardOperator::NotEqual => encode_conditional_branch_equal(skip_byte_distance)?,
        StateGuardOperator::Greater => encode_conditional_branch_less_or_equal(skip_byte_distance)?,
        StateGuardOperator::GreaterOrEqual => encode_conditional_branch_less(skip_byte_distance)?,
        StateGuardOperator::Less => encode_conditional_branch_greater_or_equal(skip_byte_distance)?,
        StateGuardOperator::LessOrEqual => encode_conditional_branch_greater(skip_byte_distance)?,
        // Unsigned comparisons skip on the negated UNSIGNED condition (cf. the x86
        // jcc skip-branches: LessUnsigned->jae, GreaterUnsigned->jbe, etc.).
        StateGuardOperator::LessUnsigned => {
            encode_conditional_branch_higher_or_same(skip_byte_distance)?
        }
        StateGuardOperator::GreaterUnsigned => {
            encode_conditional_branch_lower_or_same(skip_byte_distance)?
        }
        StateGuardOperator::LessOrEqualUnsigned => {
            encode_conditional_branch_higher(skip_byte_distance)?
        }
        StateGuardOperator::GreaterOrEqualUnsigned => {
            encode_conditional_branch_lower(skip_byte_distance)?
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot lower dispatch guard operator `{operator:?}` yet"
            )));
        }
    });
    debug_assert_eq!(
        bytes.len(),
        dispatch_guard_compare_static_width(byte_offset, byte_size, is_float)
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
        2 => byte_offset.is_multiple_of(2) && byte_offset / 2 <= 4095,
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
        1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
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
