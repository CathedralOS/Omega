use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

use super::{
    RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS, RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
    append_load_data_from_x_offset, append_runtime_value_operand, data_offset_encodable,
};
use crate::aarch64::primitives::{
    append_unsigned_immediate_padded, append_unsigned_immediate_w_padded,
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_compare_w_register,
    encode_compare_x_register, encode_conditional_branch_equal, encode_conditional_branch_greater,
    encode_conditional_branch_greater_or_equal, encode_conditional_branch_higher,
    encode_conditional_branch_higher_or_same, encode_conditional_branch_less,
    encode_conditional_branch_less_or_equal, encode_conditional_branch_lower,
    encode_conditional_branch_lower_or_same, encode_conditional_branch_not_equal,
    encode_conditional_branch_plus, encode_float_compare, encode_float_move_from_gpr,
    encode_sign_extend_byte_to_w, encode_sign_extend_halfword_to_w, encode_zero_extend_byte_to_w,
    encode_zero_extend_halfword_to_w,
};
use crate::aarch64::widths::{
    runtime_storage_compare_width, runtime_storage_value_compare_width, runtime_value_operand_width,
};

pub fn encode_runtime_storage_compare_bytes(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_compare_width(
        left_offset,
        right_offset,
        byte_size,
        is_float,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    if is_float {
        if !matches!(byte_size, 4 | 8) {
            return Err(Diagnostic::error(format!(
                "AArch64 encoder cannot compare {byte_size}-byte runtime float guard operands yet"
            )));
        }
        // Load the raw float bits into GPRs (reusing the integer load path), move
        // them into the FP bank, then FCMP. The result NZCV drives the same
        // signed conditional branch as the integer path; for ordered (non-NaN)
        // operands the signed conditions are exact (NaN handling is a documented
        // first-cut limitation, matching the x86 `ucomis*` path).
        append_load_data_from_x_offset(&mut bytes, 26, 16, left_offset, byte_size, 20)?;
        append_load_data_from_x_offset(&mut bytes, 19, 17, right_offset, byte_size, 21)?;
        bytes.extend(encode_float_move_from_gpr(byte_size, 0, 26)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 1, 19)?);
        bytes.extend(encode_float_compare(byte_size, 0, 1)?);
    } else {
        match byte_size {
            1 | 4 => {
                append_load_data_from_x_offset(&mut bytes, 26, 16, left_offset, byte_size, 20)?;
                append_load_data_from_x_offset(&mut bytes, 19, 17, right_offset, byte_size, 21)?;
                bytes.extend(encode_compare_w_register(26, 19));
            }
            2 => {
                // Halfword loads zero-extend; sign-extend BOTH sides so the
                // 32-bit compare orders correctly for signed operands (and, since
                // sign extension is monotone over the unsigned u16 range too, for
                // unsigned conditions as well).
                append_load_data_from_x_offset(&mut bytes, 26, 16, left_offset, byte_size, 20)?;
                append_load_data_from_x_offset(&mut bytes, 19, 17, right_offset, byte_size, 21)?;
                bytes.extend(encode_sign_extend_halfword_to_w(26, 26));
                bytes.extend(encode_sign_extend_halfword_to_w(19, 19));
                bytes.extend(encode_compare_w_register(26, 19));
            }
            8 => {
                append_load_data_from_x_offset(&mut bytes, 26, 16, left_offset, byte_size, 20)?;
                append_load_data_from_x_offset(&mut bytes, 19, 17, right_offset, byte_size, 21)?;
                bytes.extend(encode_compare_x_register(26, 19));
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare {byte_size}-byte runtime guard operands yet"
                )));
            }
        }
    }
    bytes.extend(encode_conditional_branch_for_operator_bytes(
        operator,
        failure_branch_distance,
        is_float,
    )?);
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_compare_width(left_offset, right_offset, byte_size, is_float)
    );
    Ok(bytes)
}

/// Exact register writes of the direct place-pair guard encoder. Large or
/// unscaled offsets additionally use the caller-supplied x20/x21 address
/// scratches; float comparisons stage raw operands in v0/v1.
pub fn runtime_storage_compare_register_writes(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(26),
    ];
    if !data_offset_encodable(left_offset, byte_size) {
        registers.push(MachineRegister::Aarch64X(20));
    }
    if !data_offset_encodable(right_offset, byte_size) {
        registers.push(MachineRegister::Aarch64X(21));
    }
    if is_float {
        registers.extend([MachineRegister::Aarch64V(0), MachineRegister::Aarch64V(1)]);
    }
    RegisterSet::new(registers)
}

pub fn runtime_storage_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn encode_runtime_storage_value_compare_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_value_compare_width(byte_offset, byte_size));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    // The expected value is materialized into a register (fixed-width, so the
    // width function stays magnitude-independent) and compared register-to-
    // register, which handles negative values and full-width bit patterns that
    // do not fit the 12-bit compare immediate. Narrow operands are sign-extended
    // on BOTH sides: exact for signed conditions, and order-preserving for
    // unsigned conditions because sign extension is monotone per source width.
    match byte_size {
        1 => {
            append_load_data_from_x_offset(&mut bytes, 17, 16, byte_offset, byte_size, 26)?;
            bytes.extend(encode_sign_extend_byte_to_w(17, 17));
            append_unsigned_immediate_w_padded(&mut bytes, 26, expected_value as i8 as i32 as u32);
            bytes.extend(encode_compare_w_register(17, 26));
        }
        2 => {
            append_load_data_from_x_offset(&mut bytes, 17, 16, byte_offset, byte_size, 26)?;
            bytes.extend(encode_sign_extend_halfword_to_w(17, 17));
            append_unsigned_immediate_w_padded(&mut bytes, 26, expected_value as i16 as i32 as u32);
            bytes.extend(encode_compare_w_register(17, 26));
        }
        4 => {
            append_load_data_from_x_offset(&mut bytes, 17, 16, byte_offset, byte_size, 26)?;
            append_unsigned_immediate_w_padded(&mut bytes, 26, expected_value as u32);
            bytes.extend(encode_compare_w_register(17, 26));
        }
        8 => {
            append_load_data_from_x_offset(&mut bytes, 17, 16, byte_offset, byte_size, 26)?;
            append_unsigned_immediate_padded(&mut bytes, 26, expected_value as u64);
            bytes.extend(encode_compare_x_register(17, 26));
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte runtime guard values yet"
            )));
        }
    }
    bytes.extend(encode_conditional_branch_for_operator_bytes(
        operator,
        failure_branch_distance,
        false,
    )?);
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_value_compare_width(byte_offset, byte_size)
    );
    Ok(bytes)
}

/// Exact register writes of the direct place-vs-immediate guard encoder. x26
/// is both the large-offset address scratch and the expected-value register.
pub fn runtime_storage_value_compare_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn runtime_storage_value_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn encode_runtime_value_compare(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + 8,
    );
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        left,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        right,
    )?;
    // aarch64 has no sub-word CMP (x86 compares `r10b`/`r10w` directly), so a
    // 1/2-byte compare must normalize BOTH registers to the compare width
    // first -- a convert-wrapped operand (`self.big as u8 == 44`) leaves the
    // untruncated source in the register (int->int narrowing is a no-op in
    // the shared convert op because the WRITE path's store truncates).
    // Sign-extend for the signed ordered operators, zero-extend otherwise
    // (equality and the unsigned family); one instruction per side either
    // way, so the width is operator-independent (+8 for narrow compares).
    if matches!(byte_size, 1 | 2) {
        let signed = matches!(
            operator,
            StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
        );
        for register in [17u8, 26u8] {
            bytes.extend(match (byte_size, signed) {
                (1, true) => encode_sign_extend_byte_to_w(register, register),
                (1, false) => encode_zero_extend_byte_to_w(register, register),
                (2, true) => encode_sign_extend_halfword_to_w(register, register),
                _ => encode_zero_extend_halfword_to_w(register, register),
            });
        }
    }
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_compare_w_register(17, 26)),
        8 => bytes.extend(encode_compare_x_register(17, 26)),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare computed runtime values of width `{byte_size}` yet"
            )));
        }
    }
    bytes.extend(encode_conditional_branch_for_operator_bytes(
        operator,
        failure_branch_distance,
        false,
    )?);
    Ok(bytes)
}

/// Closed may-write ceiling of the recursive runtime-value comparison
/// encoder. Individual operand trees use subsets of these fixed destination,
/// scratch-pool, address-helper, and FP registers.
pub fn runtime_value_compare_register_write_ceiling() -> RegisterSet {
    let mut registers = (9..=15)
        .chain([17])
        .chain(19..=21)
        .chain([26])
        .map(MachineRegister::Aarch64X)
        .collect::<Vec<_>>();
    registers.extend([MachineRegister::Aarch64V(0), MachineRegister::Aarch64V(1)]);
    RegisterSet::new(registers)
}

/// Closed may-write ceiling of a direct place-shaped binary write. Recursive
/// operand evaluation owns the runtime-value bank; x16 additionally retains
/// the relocated destination base through evaluation and the final store.
pub(super) fn encode_conditional_branch_for_operator_bytes(
    operator: StateGuardOperator,
    failure_branch_distance: isize,
    is_float: bool,
) -> Result<[u8; 4], Diagnostic> {
    Ok(match operator {
        StateGuardOperator::Equal => encode_conditional_branch_not_equal(failure_branch_distance)?,
        StateGuardOperator::NotEqual => encode_conditional_branch_equal(failure_branch_distance)?,
        StateGuardOperator::Greater => {
            encode_conditional_branch_less_or_equal(failure_branch_distance)?
        }
        StateGuardOperator::GreaterOrEqual => {
            encode_conditional_branch_less(failure_branch_distance)?
        }
        // IEEE: every ordered comparison with NaN is false. After an FCMP,
        // unordered sets C+V (NZCV 0011), so the INTEGER skip negations GE/GT
        // are false on NaN -- the guard would wrongly take its true arm (LE/LT
        // for Greater/GreaterOrEqual above fire on N!=V, so those are already
        // unordered-correct, as are EQ/NE). Float `<` skips on PL (N clear)
        // and `<=` on HI (C set, Z clear), both true on unordered -- matching
        // the x86 `ucomis*` + parity-aware failure jumps and the interpreter.
        StateGuardOperator::Less if is_float => {
            encode_conditional_branch_plus(failure_branch_distance)?
        }
        StateGuardOperator::LessOrEqual if is_float => {
            encode_conditional_branch_higher(failure_branch_distance)?
        }
        StateGuardOperator::Less => {
            encode_conditional_branch_greater_or_equal(failure_branch_distance)?
        }
        StateGuardOperator::LessOrEqual => {
            encode_conditional_branch_greater(failure_branch_distance)?
        }
        // Unsigned orderings branch to the failure target on the negated
        // UNSIGNED condition (cf. the x86 jae/ja/jbe/jb failure jumps). After
        // an FCMP these conditions are also exact for ordered float operands,
        // matching the x86 `ucomis*` + unsigned-jcc pairing.
        StateGuardOperator::LessUnsigned => {
            encode_conditional_branch_higher_or_same(failure_branch_distance)?
        }
        StateGuardOperator::LessOrEqualUnsigned => {
            encode_conditional_branch_higher(failure_branch_distance)?
        }
        // Float comparisons ride the UNSIGNED pairing (the x86 `ucomis*`
        // convention). After FCMP, unordered sets C+V: the `<`/`<=` skips
        // (HS/HI) fire on C and are unordered-correct for free, but the
        // `>`/`>=` skips (LS/LO) are FALSE on unordered -- the guard wrongly
        // took its true arm on NaN. The SIGNED complements LE/LT fire on
        // N != V, which unordered sets, so floats skip on those instead.
        StateGuardOperator::GreaterUnsigned if is_float => {
            encode_conditional_branch_less_or_equal(failure_branch_distance)?
        }
        StateGuardOperator::GreaterOrEqualUnsigned if is_float => {
            encode_conditional_branch_less(failure_branch_distance)?
        }
        StateGuardOperator::GreaterUnsigned => {
            encode_conditional_branch_lower_or_same(failure_branch_distance)?
        }
        StateGuardOperator::GreaterOrEqualUnsigned => {
            encode_conditional_branch_lower(failure_branch_distance)?
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot lower runtime compare operator `{operator:?}` yet"
        )))?,
    })
}
