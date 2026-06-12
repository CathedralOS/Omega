use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, append_unsigned_immediate_padded,
    append_unsigned_immediate_w_padded, encode_add_page_offset_placeholder, encode_add_x_register,
    encode_adrp_placeholder, encode_and_x_register, encode_cbz_x, encode_float_add,
    encode_float_compare, encode_load_byte_w_post_increment, encode_subs_x_immediate,
    encode_unconditional_branch,
    encode_float_convert_double_to_single, encode_float_convert_single_to_double,
    encode_float_divide, encode_float_move_from_gpr, encode_float_move_to_gpr,
    encode_float_multiply, encode_float_to_signed_int, encode_signed_int_to_float,
    encode_sign_extend_byte_to_w, encode_sign_extend_halfword_to_w, encode_sign_extend_word_to_x,
    encode_float_subtract, encode_compare_w_immediate, encode_compare_w_register,
    encode_compare_x_register, encode_load_byte_w_from_x,
    encode_conditional_branch_equal, encode_conditional_branch_greater,
    encode_conditional_branch_greater_or_equal, encode_conditional_branch_higher,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_higher_or_same, encode_conditional_branch_lower,
    encode_conditional_branch_lower_or_same, encode_conditional_branch_not_equal,
    encode_load_w_from_x, encode_load_x_from_x, encode_asrv_w_register, encode_asrv_x_register,
    encode_lslv_x_register, encode_lsrv_x_register, encode_move_x_register, encode_movz_w,
    encode_msub_w_register, encode_msub_x_register, encode_mul_x_register, encode_orr_x_register,
    encode_sdiv_w_register, encode_sdiv_x_register, encode_store_w_to_x, encode_store_w17_to_x16,
    encode_store_x_to_x, encode_store_x17_to_x16, encode_sub_x_register, encode_udiv_w_register,
    encode_udiv_x_register,
};
use super::widths::{
    runtime_frame_base_indexed_address_to_runtime_frame_write_width,
    runtime_frame_base_indexed_binary_write_width, runtime_frame_base_indexed_integer_write_width,
    runtime_frame_fixed_indexed_address_to_runtime_frame_write_width,
    runtime_frame_indexed_address_to_runtime_frame_write_width,
    runtime_frame_indexed_binary_write_width, runtime_frame_indexed_integer_write_width,
    runtime_frame_indexed_string_write_width, runtime_frame_string_write_width,
    runtime_machine_indexed_integer_write_width, runtime_machine_indexed_string_write_width,
    runtime_machine_integer_write_width, runtime_machine_string_write_width,
    runtime_pointee_address_to_runtime_frame_write_width, runtime_pointee_binary_write_width,
    runtime_pointee_integer_write_width, runtime_pointee_string_write_width,
    runtime_storage_address_to_runtime_frame_write_width, runtime_storage_binary_write_width,
    runtime_storage_compare_width, runtime_storage_convert_width,
    runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width,
    runtime_storage_copy_to_runtime_pointee_width, runtime_storage_copy_width,
    runtime_storage_value_compare_width, runtime_value_operand_width,
};

// x18 is NEVER used as a scratch register: it is the reserved platform register
// on Darwin arm64 and the kernel zeroes it on every kernel->user return, so any
// value held in x18 across an interrupt window is silently lost (this corrupted
// dungeon-crawler frame-slot copies nondeterministically). x26 takes its place.
const RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS: &[u8] = &[26, 15, 14, 13, 12, 11, 10, 9];
const RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS: &[u8] = &[15, 14, 13, 12, 11, 10, 9];

/// `target = source as T`: hold the target base in x16 (untouched by source
/// evaluation, which uses x17/x26/x19), load the source bits into x17, convert
/// them in place between integer/float representations, then store the result at
/// `target_offset`. Mirrors the x86_64 convert path (`cvttsd2si`/`cvtsi2sd`/
/// `cvtsd2ss`/`cvtss2sd` + sized int moves).
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_convert(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_convert_width(
        runtime_value_operands,
        target_offset,
        source,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
    ));
    // x16 = target base (held across operand evaluation, which uses x17/x26/x19).
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        source,
    )?;
    append_runtime_convert_operation(
        &mut bytes,
        17,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
    )?;
    append_runtime_storage_result_write(&mut bytes, target_offset, target_byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_convert_width(
            runtime_value_operands,
            target_offset,
            source,
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
        ),
        "convert encoder length must match its width"
    );
    Ok(bytes)
}

/// Convert the value whose raw bits are in `register` between integer/float
/// representations, leaving the converted result back in `register`. Uses FP
/// register 0 (`S0`/`D0`) as the scratch FP bank. See
/// `runtime_convert_operation_width` in `widths.rs` — the emitted length MUST
/// match it.
fn append_runtime_convert_operation(
    bytes: &mut Vec<u8>,
    register: u8,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> Result<(), Diagnostic> {
    match (source_is_float, target_is_float) {
        (false, true) => {
            // int -> float: SCVTF d0/s0, Xn/Wn (signed int in GPR -> FP), then
            // FMOV the float bits back into the GPR.
            bytes.extend(encode_signed_int_to_float(
                source_byte_size,
                target_byte_size,
                0,
                register,
            )?);
            bytes.extend(encode_float_move_to_gpr(target_byte_size, register, 0)?);
        }
        (true, false) => {
            // float -> int: FMOV the source bits into d0/s0, then FCVTZS Xn/Wn,
            // d0/s0 (round toward zero). The result write truncates to the target
            // width for i8/i16.
            let int_gpr_byte_size = if target_byte_size > 4 { 8 } else { 4 };
            bytes.extend(encode_float_move_from_gpr(source_byte_size, 0, register)?);
            bytes.extend(encode_float_to_signed_int(
                source_byte_size,
                int_gpr_byte_size,
                register,
                0,
            )?);
        }
        (true, true) => {
            if source_byte_size == target_byte_size {
                // same precision: bits already in the GPR, nothing to do.
            } else {
                // f32 <-> f64: FMOV into the FP bank, FCVT precision change, FMOV
                // back. FCVT reads/writes the source/target precision registers,
                // so the surrounding FMOVs use the matching widths.
                bytes.extend(encode_float_move_from_gpr(source_byte_size, 0, register)?);
                if source_byte_size > target_byte_size {
                    // double -> single
                    bytes.extend(encode_float_convert_double_to_single(0, 0));
                } else {
                    // single -> double
                    bytes.extend(encode_float_convert_single_to_double(0, 0));
                }
                bytes.extend(encode_float_move_to_gpr(target_byte_size, register, 0)?);
            }
        }
        (false, false) => {
            // Sign-extend a narrow signed source when widening; otherwise the load
            // already zero-extended and the store truncates.
            if target_byte_size > source_byte_size && source_signed && source_byte_size == 4 {
                bytes.extend(encode_sign_extend_word_to_x(register, register));
            }
        }
    }
    Ok(())
}

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
    )?);
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_compare_width(left_offset, right_offset, byte_size, is_float)
    );
    Ok(bytes)
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
    )?);
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_value_compare_width(byte_offset, byte_size)
    );
    Ok(bytes)
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
    )?);
    Ok(bytes)
}

pub fn encode_runtime_machine_integer_write(
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_integer_write_width(byte_offset, byte_size));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, byte_offset)?;
    match byte_size {
        // Negative values store their two's-complement bit pattern; the sized
        // store truncates to the target width.
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w17_to_x16(0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x17_to_x16(0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_integer_write_width(byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn encode_runtime_pointee_integer_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_integer_write_width(
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    match byte_size {
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime pointee integers yet"
            )));
        }
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_integer_write_width(pointer_byte_offset, field_byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        left,
        operator,
        right,
        is_float,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
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
    if is_float {
        append_runtime_float_binary_operation(&mut bytes, byte_size, 17, operator, 26)?;
    } else {
        append_runtime_binary_operation(
            &mut bytes,
            17,
            operator,
            26,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )?;
    }
    if runtime_value_operands.frame_indexed(left).is_some()
        || runtime_value_operands.frame_indexed(right).is_some()
        || runtime_value_operands.frame_base_indexed(left).is_some()
        || runtime_value_operands.frame_base_indexed(right).is_some()
    {
        bytes.extend(encode_move_x_register(16, 20));
    }
    append_runtime_storage_result_write(&mut bytes, target_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_binary_write_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            left,
            operator,
            right,
            is_float,
        )
    );
    Ok(bytes)
}

pub fn encode_runtime_pointee_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_binary_write_width(
        runtime_value_operands,
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
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
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(runtime_value_operands, operator, left, right, byte_size),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_machine_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_string_write_width(byte_length));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x17_to_x16(byte_offset)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x17_to_x16(byte_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_frame_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_string_write_width(byte_length));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x17_to_x16(byte_offset)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x17_to_x16(byte_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_pointee_string_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_string_write_width(
        pointer_byte_offset,
        field_byte_offset,
        byte_length,
    ));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_string_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_string_write_width(
        element_byte_size,
        field_byte_offset,
        byte_length,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_machine_indexed_string_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_string_write_width(
        base_byte_offset,
        element_byte_size,
        field_byte_offset,
        byte_length,
    ));
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_storage_address_to_runtime_frame_write(
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_address_to_runtime_frame_write_width(
        source_offset,
        target_offset,
    ));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_add_constant_to_x_register(&mut bytes, 17, source_offset)?;
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_store_x_to_x_offset(&mut bytes, 17, 16, target_offset)?;
    Ok(bytes)
}

pub fn encode_runtime_pointee_address_to_runtime_frame_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_address_to_runtime_frame_write_width(
        pointer_byte_offset,
        field_byte_offset,
        target_offset,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    append_runtime_storage_load(
        &mut bytes,
        17,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    append_add_constant_to_x_register(&mut bytes, 17, field_byte_offset)?;
    append_store_x_to_x_offset(&mut bytes, 17, 20, target_offset)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_address_to_runtime_frame_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_address_to_runtime_frame_write_width(
        element_byte_size,
        field_byte_offset,
        target_offset,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_store_x_to_x_offset(&mut bytes, 16, 20, target_offset)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
        ),
    );
    append_runtime_frame_fixed_index_target_address(
        &mut bytes,
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
    )?;
    append_store_x_to_x_offset(&mut bytes, 16, 20, target_offset)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_frame_base_indexed_address_to_runtime_frame_write_width(
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
        ),
    );
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_store_x_to_x_offset(&mut bytes, 16, 20, target_offset)?;
    Ok(bytes)
}

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_width(
        source_offset,
        target_offset,
        byte_count,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    append_add_constant_to_x_register(&mut bytes, 17, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 26, 16, offset, chunk_size, 19)?;
        append_store_data_to_x_offset(&mut bytes, 26, 17, offset, chunk_size, 20)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_integer_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_integer_write_width(
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    match byte_size {
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }

    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_integer_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_base_indexed_integer_write_width(
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    match byte_size {
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w17_to_x16(0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x17_to_x16(0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime indexed integers yet"
            )));
        }
    }
    Ok(bytes)
}

pub fn encode_runtime_machine_indexed_integer_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_integer_write_width(
        base_byte_offset,
        index_region,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    match byte_size {
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }

    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_binary_write_width(
        runtime_value_operands,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
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
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(runtime_value_operands, operator, left, right, byte_size),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_base_indexed_binary_write_width(
        runtime_value_operands,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
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
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(runtime_value_operands, operator, left, right, byte_size),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_runtime_frame_indexed(
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_to_runtime_frame_indexed_width(
            source_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_add_constant_to_x_register(&mut bytes, 20, source_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
            element_index,
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(source_field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    bytes.extend(encode_load_x_from_x(20, 20, pointer_byte_offset)?);
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
        ),
    );
    // x16 = element source-field address (`*(frame[descriptor]) + index*elem +
    // source_field`); leaves x20 = frame base.
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        source_field_byte_offset,
    )?;
    // x20 = target pointer value (`*(frame[pointer])`), then + target field.
    bytes.extend(encode_load_x_from_x(20, 20, pointer_byte_offset)?);
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_runtime_pointee(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_runtime_pointee_width(
        source_offset,
        pointer_byte_offset,
        field_byte_offset,
        byte_count,
    ));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 20, source_offset)?;
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

fn for_each_runtime_copy_chunk(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
    mut visit: impl FnMut(usize, usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut remaining = byte_count;
    let mut offset = 0usize;

    while remaining > 0 {
        let source_offset = source_base_offset + offset;
        let target_offset = target_base_offset + offset;
        let chunk_size =
            if remaining >= 8 && source_offset.is_multiple_of(8) && target_offset.is_multiple_of(8)
            {
                8
            } else if remaining >= 4
                && source_offset.is_multiple_of(4)
                && target_offset.is_multiple_of(4)
            {
                4
            } else {
                1
            };

        visit(offset, chunk_size)?;
        offset += chunk_size;
        remaining -= chunk_size;
    }

    if offset != byte_count {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot copy `{byte_count}` byte(s) of runtime storage yet"
        )));
    }

    Ok(())
}

fn append_runtime_frame_index_target_address(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_fixed_width_load_x_from_x_offset(bytes, 16, 20, descriptor_offset, 19);
    // Index is a 32-bit value: load it zero-extended so high bytes of the
    // adjacent slot can't be spliced into the index (see helper doc comment).
    append_fixed_width_load_index_w_from_x_offset(bytes, 17, 20, index_offset, 21);
    append_scale_x_register_by_constant(bytes, 26, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 26));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

fn append_runtime_frame_fixed_index_target_address(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    let scaled_index = element_index
        .checked_mul(element_byte_size)
        .ok_or_else(|| {
            Diagnostic::error(
                "AArch64 MVP encoder cannot address overflowing fixed indexed operand",
            )
        })?;
    let byte_offset = scaled_index.checked_add(field_byte_offset).ok_or_else(|| {
        Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed operand")
    })?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_load_data_from_x_offset(bytes, 16, 20, descriptor_offset, 8, 19)?;
    append_add_constant_to_x_register(bytes, 16, byte_offset)?;
    Ok(())
}

fn append_runtime_machine_index_target_address(
    bytes: &mut Vec<u8>,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    append_add_constant_to_x_register(bytes, 16, base_byte_offset)?;
    // Index is a 32-bit value: load it zero-extended (LDR Wt) so high bytes of
    // the adjacent slot can't be spliced into the index.
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            bytes.extend(encode_adrp_placeholder(20));
            bytes.extend(encode_add_page_offset_placeholder(20));
            bytes.extend(encode_load_w_from_x(17, 20, index_offset, 4)?);
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            bytes.extend(encode_load_w_from_x(17, 20, index_offset, 4)?);
        }
    }
    append_scale_x_register_by_constant(bytes, 26, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 26));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

fn append_runtime_frame_base_index_target_address(
    bytes: &mut Vec<u8>,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(16, 20));
    append_add_constant_to_x_register(bytes, 16, base_byte_offset)?;
    // Index is a 32-bit value: load it zero-extended so high bytes of the
    // adjacent slot can't be spliced into the index.
    append_load_data_from_x_offset(bytes, 17, 20, index_offset, 4, 19)?;
    append_scale_x_register_by_constant(bytes, 26, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 26));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

fn append_runtime_value_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination_register: u8,
    scratch_registers: &[u8],
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    if let Some(value) = runtime_value_operands.immediate_integer(operand) {
        // Negative immediates materialize as their full 64-bit two's-complement
        // bit pattern, mirroring the x86_64 backend's `mov reg, imm64`.
        append_unsigned_immediate(bytes, destination_register, value as u64);
        Ok(())
    } else if let Some((_, byte_offset, byte_size)) = runtime_value_operands.storage(operand) {
        bytes.extend(encode_adrp_placeholder(19));
        bytes.extend(encode_add_page_offset_placeholder(19));
        append_runtime_storage_load(
            bytes,
            destination_register,
            19,
            byte_offset,
            byte_size,
            "runtime operand",
        )?;
        Ok(())
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        runtime_value_operands.pointee(operand)
    {
        bytes.extend(encode_adrp_placeholder(19));
        bytes.extend(encode_add_page_offset_placeholder(19));
        append_runtime_storage_load(bytes, 19, 19, pointer_byte_offset, 8, "runtime pointee")?;
        if field_byte_offset > 0 {
            append_add_constant_to_x_register(bytes, 19, field_byte_offset)?;
        }
        append_runtime_storage_load(
            bytes,
            destination_register,
            19,
            0,
            byte_size,
            "runtime pointee operand",
        )?;
        Ok(())
    } else if let Some((
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_indexed(operand)
    {
        append_runtime_frame_index_target_address(
            bytes,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        )?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                16,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 16, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load runtime indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        append_runtime_frame_base_index_target_address(
            bytes,
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        )?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                16,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 16, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load runtime frame-base-indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_fixed_indexed(operand)
    {
        append_runtime_frame_fixed_index_target_address(
            bytes,
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
        )?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                16,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 16, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load runtime fixed indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((_, left_offset, _, right_offset)) =
        runtime_value_operands.text_equals(operand)
    {
        append_runtime_text_equals_operand(
            bytes,
            destination_register,
            scratch_registers,
            left_offset,
            right_offset,
        )?;
        Ok(())
    } else if let Some((place, literal)) = runtime_value_operands.text_equals_literal(operand) {
        append_runtime_text_equals_literal_operand(
            runtime_value_operands,
            bytes,
            destination_register,
            scratch_registers,
            place,
            &literal,
        )?;
        Ok(())
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let Some((&rhs_register, remaining_scratch)) = scratch_registers.split_first() else {
            return Err(Diagnostic::error(
                "AArch64 MVP encoder ran out of scratch registers for runtime arithmetic",
            ));
        };

        append_runtime_value_operand(
            runtime_value_operands,
            bytes,
            destination_register,
            scratch_registers,
            left,
        )?;
        append_runtime_value_operand(
            runtime_value_operands,
            bytes,
            rhs_register,
            remaining_scratch,
            right,
        )?;
        if runtime_value_operands.binary_is_float(operand) {
            // Float operands carry their IEEE bits in the GPRs; run the scalar
            // FP op on the bits (FADD/...) rather than an integer add over
            // them. Precision follows the operands' width (f64 by default).
            // MUST stay the fixed runtime_float_binary_operation_width().
            let byte_size = runtime_value_operand_value_byte_size(runtime_value_operands, left)
                .or_else(|| runtime_value_operand_value_byte_size(runtime_value_operands, right))
                .unwrap_or(8);
            append_runtime_float_binary_operation(
                bytes,
                byte_size,
                destination_register,
                operator,
                rhs_register,
            )?;
        } else {
            // Comparisons use the operand width; other nested binaries do not
            // carry their result width, so assume 64-bit (matches the x86_64
            // backend).
            append_runtime_binary_operation(
                bytes,
                destination_register,
                operator,
                rhs_register,
                runtime_binary_operation_byte_size(
                    runtime_value_operands,
                    operator,
                    left,
                    right,
                    8,
                ),
            )?;
        }
        Ok(())
    } else if let Some((source, source_byte_size, target_byte_size, source_is_float, target_is_float, source_signed)) =
        runtime_value_operands.convert(operand)
    {
        // Load the cast's source into the destination register, then convert it
        // in place (SCVTF / FCVTZS / FCVT / SXTW), mirroring the x86_64 backend.
        append_runtime_value_operand(
            runtime_value_operands,
            bytes,
            destination_register,
            scratch_registers,
            source,
        )?;
        append_runtime_convert_operation(
            bytes,
            destination_register,
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
        )?;
        Ok(())
    } else {
        Err(Diagnostic::error(
            "AArch64 runtime value operand is not implemented yet",
        ))
    }
}

/// Value-position text content equality: `destination = (left == right)` as
/// bool 0/1, where both sides are `{ptr @ +0, len @ +8}` text descriptors at
/// relocated region bases. FIXED-WIDTH (`runtime_text_equals_operand_width`):
/// the descriptor words load through `append_fixed_width_load_x_from_x_offset`
/// so the encoding never varies with the field offsets, keeping the relocation
/// offsets (left page at the operand start, right page at
/// `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET`) pinned.
///
/// Register use: x19 = descriptor page base, then the second byte scratch in
/// the loop; five pool registers carry left ptr/len, right ptr/len, and the
/// first byte scratch (doubling as the fixed-load offset scratch). x16/x20
/// are NOT touched: binary-write shapes hold their target address there
/// across operand evaluation.
fn append_runtime_text_equals_operand(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    scratch_registers: &[u8],
    left_offset: usize,
    right_offset: usize,
) -> Result<(), Diagnostic> {
    let [left_ptr, left_len, right_ptr, right_len, byte_scratch, ..] = *scratch_registers else {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder ran out of scratch registers for runtime text equality",
        ));
    };
    let operand_start = bytes.len();

    // Left descriptor: page (relocated at the operand start), then ptr and len.
    bytes.extend(encode_adrp_placeholder(19));
    bytes.extend(encode_add_page_offset_placeholder(19));
    append_fixed_width_load_x_from_x_offset(bytes, left_ptr, 19, left_offset, byte_scratch);
    append_fixed_width_load_x_from_x_offset(bytes, left_len, 19, left_offset + 8, byte_scratch);

    // Right descriptor: page relocated at the pinned right-base offset.
    debug_assert_eq!(
        bytes.len() - operand_start,
        super::widths::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
        "right descriptor page must sit at the pinned relocation offset"
    );
    bytes.extend(encode_adrp_placeholder(19));
    bytes.extend(encode_add_page_offset_placeholder(19));
    append_fixed_width_load_x_from_x_offset(bytes, right_ptr, 19, right_offset, byte_scratch);
    append_fixed_width_load_x_from_x_offset(bytes, right_len, 19, right_offset + 8, byte_scratch);

    // result = 0; unequal lengths are unequal text. The b.ne also means a
    // zero-length pair never enters the loop, so an all-zero (default)
    // descriptor's null pointer is never dereferenced.
    bytes.extend(encode_movz_w(destination_register, 0));
    bytes.extend(encode_compare_x_register(left_len, right_len));
    bytes.extend(encode_conditional_branch_not_equal(36)?);
    // Bounded byte loop (the value-position sibling of the wire encoder's
    // text byte copy); left_len counts down the remaining bytes:
    //   loop: cbz  left_len, equal    (+28)
    //         ldrb byte_scratch, [left_ptr], #1
    //         ldrb w19, [right_ptr], #1
    //         cmp  byte_scratch, w19
    //         b.ne done               (+16)
    //         subs left_len, left_len, #1
    //         b    loop               (-24)
    //  equal: movz destination, #1
    //  done:
    bytes.extend(encode_cbz_x(left_len, 28)?);
    bytes.extend(encode_load_byte_w_post_increment(byte_scratch, left_ptr, 1)?);
    bytes.extend(encode_load_byte_w_post_increment(19, right_ptr, 1)?);
    bytes.extend(encode_compare_w_register(byte_scratch, 19));
    bytes.extend(encode_conditional_branch_not_equal(16)?);
    bytes.extend(encode_subs_x_immediate(left_len, left_len, 1)?);
    bytes.extend(encode_unconditional_branch(-24)?);
    bytes.extend(encode_movz_w(destination_register, 1));

    debug_assert_eq!(
        bytes.len() - operand_start,
        super::widths::runtime_text_equals_operand_width(),
        "text-equals operand encoder length must match its width"
    );
    Ok(())
}

/// Guard-position text content equality against an inline literal:
/// `destination = (place == literal)` as bool 0/1, where `place` names the
/// String side's `{ptr @ +0, len @ +8}` text descriptor (a relocated storage
/// base, a pointee field behind a frame pointer slot, or a frame-indexed /
/// frame-base-indexed / frame-fixed-indexed element field) and the literal's
/// expected bytes are compared as inline immediates -- no rodata descriptor
/// exists for the literal side. Width is
/// `runtime_text_equals_literal_operand_width` (place-setup plus a fixed
/// head plus 12 bytes per literal byte).
///
/// Register use: the place address setup lands the descriptor address in x16
/// (clobbering x17/x19/x20/x21/x26 on the indexed paths, exactly like the
/// corresponding load operands); ptr/len/byte scratch come from the pool,
/// and `destination` is only written after the setup. The operand is built as
/// the LEFT side of its compare, so nothing live sits in those registers yet.
fn append_runtime_text_equals_literal_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination_register: u8,
    scratch_registers: &[u8],
    place: RuntimeValueOperandHandle,
    literal: &str,
) -> Result<(), Diagnostic> {
    let [ptr_register, len_register, byte_scratch, ..] = *scratch_registers else {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder ran out of scratch registers for runtime text literal equality",
        ));
    };
    let operand_start = bytes.len();

    // Descriptor address -> x16. The relocated page materialization sits at
    // the operand start (the relocation planner targets it there).
    if let Some((_, byte_offset, _)) = runtime_value_operands.storage(place) {
        bytes.extend(encode_adrp_placeholder(16));
        bytes.extend(encode_add_page_offset_placeholder(16));
        append_add_constant_to_x_register(bytes, 16, byte_offset)?;
    } else if let Some((pointer_byte_offset, field_byte_offset, _)) =
        runtime_value_operands.pointee(place)
    {
        // x16 = frame base (relocated page pair), then the stored pointer.
        // The descriptor sits in the POINTEE at the field offset -- never
        // read the pointer slot's own bytes as a descriptor.
        bytes.extend(encode_adrp_placeholder(16));
        bytes.extend(encode_add_page_offset_placeholder(16));
        append_runtime_storage_load(bytes, 16, 16, pointer_byte_offset, 8, "runtime text pointee")?;
        if field_byte_offset > 0 {
            append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
        }
    } else if let Some((descriptor_offset, index_offset, element_byte_size, field_byte_offset, _)) =
        runtime_value_operands.frame_indexed(place)
    {
        append_runtime_frame_index_target_address(
            bytes,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        )?;
    } else if let Some((base_byte_offset, index_offset, element_byte_size, field_byte_offset, _)) =
        runtime_value_operands.frame_base_indexed(place)
    {
        append_runtime_frame_base_index_target_address(
            bytes,
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        )?;
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_fixed_indexed(place)
    {
        append_runtime_frame_fixed_index_target_address(
            bytes,
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
        )?;
    } else {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder cannot compare this text place against a literal yet",
        ));
    }

    bytes.extend(encode_load_x_from_x(ptr_register, 16, 0)?);
    bytes.extend(encode_load_x_from_x(len_register, 16, 8)?);

    // result = 0; a length mismatch is unequal text. The b.ne also means an
    // all-zero (default) descriptor never has its null pointer dereferenced
    // when the literal is non-empty.
    bytes.extend(encode_movz_w(destination_register, 0));
    append_unsigned_immediate_padded(bytes, byte_scratch, literal.len() as u64);
    bytes.extend(encode_compare_x_register(len_register, byte_scratch));
    let literal_bytes = literal.as_bytes();
    // Forward distances to `done` (the instruction after the final movz):
    // each unrolled byte block is 12 bytes, plus the 4-byte equal-result movz.
    bytes.extend(encode_conditional_branch_not_equal(
        (12 * literal_bytes.len() + 8) as isize,
    )?);
    for (byte_index, expected_byte) in literal_bytes.iter().enumerate() {
        bytes.extend(encode_load_byte_w_from_x(
            byte_scratch,
            ptr_register,
            byte_index,
        )?);
        bytes.extend(encode_compare_w_immediate(
            byte_scratch,
            u32::from(*expected_byte),
        )?);
        let remaining_blocks = literal_bytes.len() - 1 - byte_index;
        bytes.extend(encode_conditional_branch_not_equal(
            (12 * remaining_blocks + 8) as isize,
        )?);
    }
    bytes.extend(encode_movz_w(destination_register, 1));

    debug_assert_eq!(
        bytes.len() - operand_start,
        super::widths::runtime_text_equals_literal_operand_width(
            runtime_value_operands,
            place,
            literal
        ),
        "text-equals-literal operand encoder length must match its width"
    );
    Ok(())
}

fn append_runtime_storage_load(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    context: &str,
) -> Result<(), Diagnostic> {
    if byte_offset > 0 {
        append_add_constant_to_x_register(bytes, base_register, byte_offset)?;
    }

    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
            destination_register,
            base_register,
            0,
            byte_size,
        )?),
        8 => bytes.extend(encode_load_x_from_x(
            destination_register,
            base_register,
            0,
        )?),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot load {context} width `{byte_size}` yet"
            )));
        }
    }

    Ok(())
}

/// Run `destination OP right` on the values already materialized in the GPRs,
/// leaving the result in `destination_register`.
///
/// `byte_size` is the OPERAND width (see `runtime_binary_operation_byte_size`):
/// signedness-sensitive operations (division, arithmetic right shift, min/max,
/// ordered comparisons) run in the 32-bit `W` forms when the operands are 4
/// bytes or narrower, so an i32 sign bit loaded zero-extended is honored —
/// mirroring how the x86_64 backend sizes `idiv`/`sar`/`cmp` to the operands.
/// Every arm emits the same byte count for either width, so
/// `runtime_binary_operation_width` stays width-independent.
fn append_runtime_binary_operation(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    operator: StateGuardOperator,
    right_register: u8,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let narrow = byte_size <= 4;
    match operator {
        StateGuardOperator::Add => {
            bytes.extend(encode_add_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::And => {
            bytes.extend(encode_and_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Or => {
            bytes.extend(encode_orr_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Subtract => {
            bytes.extend(encode_sub_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Multiply => {
            bytes.extend(encode_mul_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::ShiftLeft => {
            bytes.extend(encode_lslv_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        // Arithmetic (sign-filling) right shift for a signed `>>`, sized to the
        // operands so a narrow operand's sign bit fills correctly.
        StateGuardOperator::ShiftRight => {
            bytes.extend(if narrow {
                encode_asrv_w_register(
                    destination_register,
                    destination_register,
                    right_register,
                )
            } else {
                encode_asrv_x_register(
                    destination_register,
                    destination_register,
                    right_register,
                )
            });
        }
        // Logical (zero-filling) right shift for an unsigned `>>`.
        StateGuardOperator::ShiftRightLogical => {
            bytes.extend(encode_lsrv_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Divide | StateGuardOperator::DivideUnsigned => {
            let signed = matches!(operator, StateGuardOperator::Divide);
            bytes.extend(encode_division(
                signed,
                narrow,
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Modulo | StateGuardOperator::ModuloUnsigned => {
            let signed = matches!(operator, StateGuardOperator::Modulo);
            bytes.extend(encode_division(
                signed,
                narrow,
                19,
                destination_register,
                right_register,
            ));
            bytes.extend(if narrow {
                encode_msub_w_register(
                    destination_register,
                    19,
                    right_register,
                    destination_register,
                )
            } else {
                encode_msub_x_register(
                    destination_register,
                    19,
                    right_register,
                    destination_register,
                )
            });
        }
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => {
            // Compare at the operand width so an i32 sign/high bit is read
            // correctly, then conditionally take the right operand.
            bytes.extend(if narrow {
                encode_compare_w_register(destination_register, right_register)
            } else {
                encode_compare_x_register(destination_register, right_register)
            });
            // Keep `dst` (skip the move) when it is already the winner; the unsigned
            // variants use the unsigned condition (HS/LS) instead of signed (GE/LE).
            bytes.extend(match operator {
                StateGuardOperator::Max => encode_conditional_branch_greater_or_equal(8)?,
                StateGuardOperator::Min => encode_conditional_branch_less_or_equal(8)?,
                StateGuardOperator::MaxUnsigned => {
                    encode_conditional_branch_higher_or_same(8)?
                }
                StateGuardOperator::MinUnsigned => encode_conditional_branch_lower_or_same(8)?,
                _ => unreachable!(),
            });
            bytes.extend(encode_move_x_register(destination_register, right_register));
        }
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => {
            // Compare at the operand width (`byte_size` here is the operand
            // width, not the bool result), then materialize 0/1 by branching
            // over the `1` move on the negated condition. Ordering uses signed
            // (GT/GE/LT/LE) or unsigned (HI/HS/LO/LS) conditions per the
            // operand type.
            bytes.extend(if narrow {
                encode_compare_w_register(destination_register, right_register)
            } else {
                encode_compare_x_register(destination_register, right_register)
            });
            bytes.extend(encode_movz_w(destination_register, 0));
            bytes.extend(match operator {
                StateGuardOperator::Equal => encode_conditional_branch_not_equal(8)?,
                StateGuardOperator::NotEqual => encode_conditional_branch_equal(8)?,
                StateGuardOperator::Greater => encode_conditional_branch_less_or_equal(8)?,
                StateGuardOperator::GreaterOrEqual => encode_conditional_branch_less(8)?,
                StateGuardOperator::Less => encode_conditional_branch_greater_or_equal(8)?,
                StateGuardOperator::LessOrEqual => encode_conditional_branch_greater(8)?,
                StateGuardOperator::GreaterUnsigned => {
                    encode_conditional_branch_lower_or_same(8)?
                }
                StateGuardOperator::GreaterOrEqualUnsigned => {
                    encode_conditional_branch_lower(8)?
                }
                StateGuardOperator::LessUnsigned => {
                    encode_conditional_branch_higher_or_same(8)?
                }
                StateGuardOperator::LessOrEqualUnsigned => {
                    encode_conditional_branch_higher(8)?
                }
                _ => unreachable!(),
            });
            bytes.extend(encode_movz_w(destination_register, 1));
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot lower runtime binary operator `{operator:?}` yet"
            )));
        }
    }

    Ok(())
}

/// `SDIV`/`UDIV` sized to the operands: the `W` form for operands of 4 bytes or
/// narrower (whose loads zero-extend), the `X` form for 8-byte operands.
fn encode_division(
    signed: bool,
    narrow: bool,
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    match (signed, narrow) {
        (true, true) => encode_sdiv_w_register(destination_register, left_register, right_register),
        (true, false) => {
            encode_sdiv_x_register(destination_register, left_register, right_register)
        }
        (false, true) => {
            encode_udiv_w_register(destination_register, left_register, right_register)
        }
        (false, false) => {
            encode_udiv_x_register(destination_register, left_register, right_register)
        }
    }
}

/// Whether the operator produces a bool from comparing its operands (so its
/// compare width comes from the operands, not the bool-sized target).
fn is_comparison_operator(operator: StateGuardOperator) -> bool {
    matches!(
        operator,
        StateGuardOperator::Equal
            | StateGuardOperator::NotEqual
            | StateGuardOperator::Greater
            | StateGuardOperator::GreaterOrEqual
            | StateGuardOperator::Less
            | StateGuardOperator::LessOrEqual
            | StateGuardOperator::GreaterUnsigned
            | StateGuardOperator::GreaterOrEqualUnsigned
            | StateGuardOperator::LessUnsigned
            | StateGuardOperator::LessOrEqualUnsigned
    )
}

/// Value width of a runtime operand, looking through nested binary operands.
/// `None` for immediates (which carry no width).
fn runtime_value_operand_value_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> Option<usize> {
    if let Some((_, _, byte_size)) = operands.storage(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, byte_size)) = operands.pointee(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_base_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_fixed_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((left, _, right)) = operands.binary(operand) {
        return runtime_value_operand_value_byte_size(operands, left)
            .or_else(|| runtime_value_operand_value_byte_size(operands, right));
    }
    if let Some((_, _, target_byte_size, _, _, _)) = operands.convert(operand) {
        return Some(target_byte_size);
    }
    if operands.text_equals(operand).is_some() || operands.text_equals_literal(operand).is_some() {
        // Text content equality evaluates to a bool.
        return Some(1);
    }
    None
}

/// Width to compare two operands at: the first operand with a known width, else
/// the i32 default. (`a OP b` requires `a` and `b` to share a type, so either
/// operand's width is the comparison width.)
fn runtime_binary_compare_byte_size(
    operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_value_byte_size(operands, left)
        .or_else(|| runtime_value_operand_value_byte_size(operands, right))
        .unwrap_or(4)
}

/// Width to pass to `append_runtime_binary_operation`. Comparisons produce a
/// `bool`, so the target width is not the compared-operands' width — derive it
/// from the operands instead. All other operations share the target's width.
fn runtime_binary_operation_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operator: StateGuardOperator,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    target_byte_size: usize,
) -> usize {
    if is_comparison_operator(operator) {
        runtime_binary_compare_byte_size(operands, left, right)
    } else {
        target_byte_size
    }
}

/// Run an IEEE-754 binary operation on the raw float bits already materialized in
/// the GPRs `left_register` (left operand) and `right_register` (right operand),
/// leaving the result bits back in `left_register`.
///
/// The operand width selects single (4) vs double (8) precision, mirroring the
/// x86 backend's `addss`/`addsd` selection. The integers are moved into the FP
/// bank via `FMOV` (a raw bit copy, no numeric conversion), the scalar FP op runs
/// in `S0`/`D0` and `S1`/`D1`, and the result is moved back with `FMOV`.
fn append_runtime_float_binary_operation(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    left_register: u8,
    operator: StateGuardOperator,
    right_register: u8,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_float_move_from_gpr(byte_size, 0, left_register)?);
    bytes.extend(encode_float_move_from_gpr(byte_size, 1, right_register)?);
    match operator {
        StateGuardOperator::Add => bytes.extend(encode_float_add(byte_size, 0, 0, 1)?),
        StateGuardOperator::Subtract => bytes.extend(encode_float_subtract(byte_size, 0, 0, 1)?),
        StateGuardOperator::Multiply => bytes.extend(encode_float_multiply(byte_size, 0, 0, 1)?),
        StateGuardOperator::Divide => bytes.extend(encode_float_divide(byte_size, 0, 0, 1)?),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot lower runtime float binary operator `{operator:?}` yet"
            )));
        }
    }
    bytes.extend(encode_float_move_to_gpr(byte_size, left_register, 0)?);
    Ok(())
}

fn append_runtime_storage_result_write(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    match byte_size {
        1 | 2 | 4 | 8 => append_store_data_to_x_offset(bytes, 17, 16, byte_offset, byte_size, 19)?,
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot write {byte_size}-byte runtime storage results yet"
            )));
        }
    }

    Ok(())
}

fn encode_conditional_branch_for_operator_bytes(
    operator: StateGuardOperator,
    failure_branch_distance: isize,
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

fn append_scale_x_register_by_constant(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    source_register: u8,
    scale: usize,
) -> Result<(), Diagnostic> {
    if scale == 0 {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder cannot scale indexed runtime storage by zero",
        ));
    }

    append_unsigned_immediate(bytes, destination_register, 0);
    let working_register = 19u8;
    bytes.extend(encode_move_x_register(working_register, source_register));

    let highest_bit = usize::BITS - scale.leading_zeros();
    for bit_index in 0..highest_bit {
        if (scale >> bit_index) & 1 == 1 {
            bytes.extend(encode_add_x_register(
                destination_register,
                destination_register,
                working_register,
            ));
        }

        if bit_index + 1 < highest_bit {
            bytes.extend(encode_add_x_register(
                working_register,
                working_register,
                working_register,
            ));
        }
    }

    Ok(())
}

fn append_add_constant_to_x_register(
    bytes: &mut Vec<u8>,
    register: u8,
    value: usize,
) -> Result<(), Diagnostic> {
    let scratch_register = if register == 19 { 26 } else { 19 };
    append_add_x_constant(bytes, register, register, value, scratch_register)
}

fn append_store_x_to_x_offset(
    bytes: &mut Vec<u8>,
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    if byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095 {
        bytes.extend(encode_store_x_to_x(
            source_register,
            base_register,
            byte_offset,
        )?);
    } else {
        append_add_constant_to_x_register(bytes, base_register, byte_offset)?;
        bytes.extend(encode_store_x_to_x(source_register, base_register, 0)?);
    }

    Ok(())
}

pub(in crate::aarch64) fn append_load_data_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, byte_size) {
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                base_register,
                byte_offset,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(
                destination_register,
                base_register,
                byte_offset,
            )?),
            _ => unreachable!("runtime data loads are 1, 2, 4, or 8 bytes"),
        }
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        append_add_constant_to_x_register(bytes, scratch_register, byte_offset)?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                scratch_register,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(
                destination_register,
                scratch_register,
                0,
            )?),
            _ => unreachable!("runtime data loads are 1, 2, 4, or 8 bytes"),
        }
    }

    Ok(())
}

pub(in crate::aarch64) fn append_store_data_to_x_offset(
    bytes: &mut Vec<u8>,
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, byte_size) {
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_store_w_to_x(
                source_register,
                base_register,
                byte_offset,
                byte_size,
            )?),
            8 => bytes.extend(encode_store_x_to_x(
                source_register,
                base_register,
                byte_offset,
            )?),
            _ => unreachable!("runtime data stores are 1, 2, 4, or 8 bytes"),
        }
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        append_add_constant_to_x_register(bytes, scratch_register, byte_offset)?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_store_w_to_x(
                source_register,
                scratch_register,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_store_x_to_x(source_register, scratch_register, 0)?),
            _ => unreachable!("runtime data stores are 1, 2, 4, or 8 bytes"),
        }
    }

    Ok(())
}

fn append_fixed_width_load_x_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        scratch_register,
        base_register,
        scratch_register,
    ));
    bytes.extend(
        encode_load_x_from_x(destination_register, scratch_register, 0)
            .expect("zero-offset x-register load should always encode"),
    );
}

/// Loads a 32-bit array INDEX (zero-extended into the full X register) from
/// `[base_register + byte_offset]`. Array indices are always non-negative and
/// fit in 32 bits; loading the full 64-bit slot would splice adjacent bytes
/// into the high half of an `i32` index and produce a wild element address.
/// `LDR Wt` auto-zeroes the upper 32 bits of Xt, which is exactly what we want.
///
/// Emits the SAME 24-byte sequence as `append_fixed_width_load_x_from_x_offset`
/// (padded 4-instruction immediate = 16 bytes, ADD = 4, load = 4) — only the
/// final load differs (`LDR Wt` vs `LDR Xt`, both 4 bytes) — so width functions
/// are unchanged.
fn append_fixed_width_load_index_w_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        scratch_register,
        base_register,
        scratch_register,
    ));
    bytes.extend(
        encode_load_w_from_x(destination_register, scratch_register, 0, 4)
            .expect("zero-offset w-register load should always encode"),
    );
}

fn data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        2 => byte_offset.is_multiple_of(2) && byte_offset / 2 <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::widths;

    /// The zero-extending index load must keep the exact byte width of the
    /// 64-bit variant (only the final opcode changes), so width functions are
    /// undisturbed.
    #[test]
    fn index_w_load_matches_x_load_width() {
        let mut w_bytes = Vec::new();
        append_fixed_width_load_index_w_from_x_offset(&mut w_bytes, 17, 20, 0x40, 21);
        let mut x_bytes = Vec::new();
        append_fixed_width_load_x_from_x_offset(&mut x_bytes, 17, 20, 0x40, 21);
        assert_eq!(w_bytes.len(), x_bytes.len());
        assert_eq!(w_bytes.len(), 24);
    }

    /// The final instruction must be `LDR Wt` (opcode family 0xB9400000), which
    /// zero-extends the upper 32 bits, NOT `LDR Xt` (0xF9400000).
    #[test]
    fn index_w_load_emits_w_register_load() {
        let mut bytes = Vec::new();
        append_fixed_width_load_index_w_from_x_offset(&mut bytes, 17, 20, 0x40, 21);
        let last = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
        // size field (bits 30-31) of LDR Wt is 0b10; LDR Xt is 0b11.
        assert_eq!(last & 0xFFC0_0000, 0xB940_0000, "expected LDR Wt (32-bit)");
    }

    /// The frame-index target-address setup width must still match what the
    /// encoder emits after switching the index load to 32-bit.
    #[test]
    fn frame_index_setup_width_matches_emission() {
        for &(element_size, field_offset) in
            &[(1usize, 0usize), (4, 0), (8, 8), (24, 16), (40, 0)]
        {
            let mut bytes = Vec::new();
            append_runtime_frame_index_target_address(&mut bytes, 0x10, 0x40, element_size, field_offset)
                .unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_frame_index_setup_width(element_size, field_offset),
                "element_size={element_size}, field_offset={field_offset}"
            );
        }
    }

    /// New frame-indexed -> pointee copy encoder length must equal its width.
    #[test]
    fn frame_indexed_to_pointee_copy_width_matches_emission() {
        let cases = [
            // (element_size, source_field, pointer_offset, target_field, byte_count)
            (24usize, 0usize, 0usize, 0usize, 8usize),
            (40, 8, 16, 8, 16),
            (16, 0, 24, 0, 4),
            (32, 16, 8, 16, 24),
        ];
        for &(element_size, source_field, pointer_offset, target_field, byte_count) in &cases {
            let bytes = encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
                0x10,
                0x40,
                element_size,
                source_field,
                pointer_offset,
                target_field,
                byte_count,
            )
            .unwrap();
            let expected = widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
                element_size,
                source_field,
                target_field,
                byte_count,
            );
            assert_eq!(
                bytes.len(),
                expected,
                "element_size={element_size}, source_field={source_field}, pointer_offset={pointer_offset}, target_field={target_field}, byte_count={byte_count}"
            );
        }
    }

    /// The value compare materializes the expected value into a register and
    /// compares register-to-register; its emitted length must equal the width
    /// for every operand size, regardless of the expected value's sign or
    /// magnitude.
    #[test]
    fn storage_value_compare_width_matches_emission() {
        for &byte_size in &[1usize, 2, 4, 8] {
            for &expected in &[0i64, 7, -3, -1000, 4_294_967_297, i64::MIN] {
                let bytes = encode_runtime_storage_value_compare_bytes(
                    0x20,
                    byte_size,
                    expected,
                    8,
                    StateGuardOperator::Equal,
                )
                .unwrap();
                assert_eq!(
                    bytes.len(),
                    widths::runtime_storage_value_compare_width(0x20, byte_size),
                    "byte_size={byte_size}, expected={expected}"
                );
            }
        }
    }

    /// Negative integer writes store the two's-complement bit pattern truncated
    /// to the target width, and the emitted length must equal the width.
    #[test]
    fn negative_integer_write_width_matches_emission() {
        for &(byte_size, value) in &[
            (1usize, -42i64),
            (2, -1000),
            (4, -70000),
            (8, -42),
            (4, 0x1_0000), // > 16 bits: must not silently truncate
        ] {
            let bytes = encode_runtime_machine_integer_write(0x10, byte_size, value).unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_machine_integer_write_width(0x10, byte_size),
                "byte_size={byte_size}, value={value}"
            );
        }
    }

    /// A 4-byte write of a value above 16 bits must materialize BOTH halfwords
    /// (MOVZ + MOVK), not silently truncate to the low 16 bits.
    #[test]
    fn integer_write_materializes_full_32_bits() {
        let bytes = encode_runtime_machine_integer_write(0x10, 4, 0x0004_0003).unwrap();
        // The two instructions before the trailing store materialize w17.
        let word_at = |from_end: usize| {
            let start = bytes.len() - from_end;
            u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
        };
        // MOVZ w17, #3: 0x52800000 | (3 << 5) | 17.
        assert_eq!(word_at(12), 0x5280_0000 | (3 << 5) | 17, "MOVZ w17, #3");
        // MOVK w17, #4, LSL #16: 0x72800000 | (1 << 21) | (4 << 5) | 17.
        assert_eq!(
            word_at(8),
            0x7280_0000 | (1 << 21) | (4 << 5) | 17,
            "MOVK w17, #4, LSL #16"
        );
    }

    /// The frame-base-indexed setup loads the index as 4 bytes; the integer
    /// write width must agree with the encoder.
    #[test]
    fn frame_base_indexed_integer_write_width_matches_emission() {
        for &(base, index_off, element_size, field, value_size) in &[
            (0x20usize, 0x48usize, 4usize, 0usize, 4usize),
            (0x20, 0x48, 8, 8, 8),
            (0x20, 0x40, 16, 0, 1),
        ] {
            let bytes = encode_runtime_frame_base_indexed_integer_write(
                base, index_off, element_size, field, value_size, 7,
            )
            .unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_frame_base_indexed_integer_write_width(
                    base, index_off, element_size, field, value_size,
                ),
                "value_size={value_size}"
            );
        }
    }

    /// The float storage compare adds two FMOVs + an FCMP; its emitted length
    /// must equal the (float-aware) width for both single and double precision.
    #[test]
    fn float_storage_compare_width_matches_emission() {
        for &byte_size in &[4usize, 8] {
            let bytes = encode_runtime_storage_compare_bytes(
                0x10,
                0x20,
                byte_size,
                8,
                StateGuardOperator::Less,
                true,
            )
            .unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_storage_compare_width(0x10, 0x20, byte_size, true),
                "byte_size={byte_size}"
            );
            // The float path must be exactly 8 bytes (two FMOVs) longer than the
            // integer path at the same offsets/width.
            assert_eq!(
                widths::runtime_storage_compare_width(0x10, 0x20, byte_size, true),
                widths::runtime_storage_compare_width(0x10, 0x20, byte_size, false) + 8,
            );
        }
    }

    /// The float storage compare must emit an FCMP (single `0x1e22_2020` family /
    /// double `0x1e62_2020` family) — i.e. ftype follows the operand width — and
    /// not an integer `CMP`.
    #[test]
    fn float_storage_compare_emits_fcmp_of_correct_precision() {
        let single = encode_runtime_storage_compare_bytes(
            0x10,
            0x20,
            4,
            8,
            StateGuardOperator::Less,
            true,
        )
        .unwrap();
        let double = encode_runtime_storage_compare_bytes(
            0x10,
            0x20,
            8,
            8,
            StateGuardOperator::Less,
            true,
        )
        .unwrap();
        // The FCMP is the instruction immediately before the trailing branch.
        let fcmp_word = |bytes: &[u8]| {
            let start = bytes.len() - 8;
            u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
        };
        // FCMP base (Rm/Rn cleared): single 0x1e202000, double 0x1e602000.
        assert_eq!(fcmp_word(&single) & 0xFFE0_FC1F, 0x1e20_2000, "single FCMP");
        assert_eq!(fcmp_word(&double) & 0xFFE0_FC1F, 0x1e60_2000, "double FCMP");
    }

    /// Build a value-operand arena holding a single storage source operand and
    /// return both the arena and the source handle. The convert encoder loads the
    /// source via the generic value-operand path; a storage source gives a
    /// deterministic, offset-free load width.
    fn storage_source(
        byte_size: usize,
    ) -> (
        omega_core::arena::Arena<omega_target_operations::RuntimeValueOperand>,
        omega_target_operations::RuntimeValueOperandHandle,
    ) {
        let mut arena = omega_core::arena::Arena::new();
        let handle = arena.insert(omega_target_operations::RuntimeValueOperand::Storage {
            region: omega_target_operations::RuntimeStorageRegion::Machine,
            byte_offset: 0x20,
            byte_size,
        });
        (arena, handle)
    }

    /// The converting-store encoder length must equal its width function for every
    /// (target_offset, source/target width, float/int) combination — layout and
    /// relocation placement both rely on the width being exact.
    #[test]
    fn convert_encoder_length_matches_width() {
        // (target_offset, src_size, tgt_size, src_float, tgt_float, src_signed)
        let cases = [
            // int -> float
            (0x10usize, 4usize, 8usize, false, true, true),
            (0x20, 8, 8, false, true, true),
            (0x18, 4, 4, false, true, true),
            // float -> int
            (0x10, 8, 4, true, false, true),
            (0x20, 8, 8, true, false, true),
            (0x28, 4, 4, true, false, true),
            (0x30, 4, 1, true, false, true),
            // float -> float
            (0x10, 8, 4, true, true, true),  // f64 -> f32 (FCVT narrow)
            (0x20, 4, 8, true, true, true),  // f32 -> f64 (FCVT widen)
            (0x18, 8, 8, true, true, true),  // f64 -> f64 (no-op convert)
            // int -> int
            (0x10, 4, 8, false, false, true),  // signed widen (SXTW)
            (0x20, 4, 8, false, false, false), // unsigned widen (no SXTW)
            (0x28, 8, 4, false, false, true),  // narrow (store truncates)
            (0x30, 4, 4, false, false, true),  // same width
            // a larger, non-trivially-encodable target offset
            (0x4000, 8, 8, false, true, true),
        ];
        for &(target_offset, src_size, tgt_size, src_float, tgt_float, src_signed) in &cases {
            let (arena, source) = storage_source(src_size);
            let bytes = encode_runtime_storage_convert(
                &arena,
                target_offset,
                tgt_size,
                source,
                src_size,
                src_float,
                tgt_float,
                src_signed,
            )
            .unwrap();
            let width = widths::runtime_storage_convert_width(
                &arena,
                target_offset,
                source,
                src_size,
                tgt_size,
                src_float,
                tgt_float,
                src_signed,
            );
            assert_eq!(
                bytes.len(),
                width,
                "len != width for target_offset={target_offset:#x}, src_size={src_size}, tgt_size={tgt_size}, src_float={src_float}, tgt_float={tgt_float}, src_signed={src_signed}"
            );
        }
    }

    /// int -> float must emit SCVTF then FMOV(result -> GPR); float -> int must
    /// emit FMOV(bits -> FP) then FCVTZS. Check the opcode families of the two
    /// trailing convert instructions (they sit right before the result store).
    #[test]
    fn convert_emits_expected_conversion_opcodes() {
        // int(w) -> double: SCVTF d0,w17 (0x1e62_0000 family) + FMOV x17,d0
        // (0x9e66_0000 family).
        let (arena, source) = storage_source(4);
        let bytes =
            encode_runtime_storage_convert(&arena, 0x10, 8, source, 4, false, true, true).unwrap();
        // The store is a single 4-byte STR at offset 0x10 (encodable), so the two
        // convert words are at len-12..len-4.
        let word_at = |b: &[u8], from_end: usize| {
            let start = b.len() - from_end;
            u32::from_le_bytes(b[start..start + 4].try_into().unwrap())
        };
        let scvtf = word_at(&bytes, 12);
        let fmov_back = word_at(&bytes, 8);
        // SCVTF d0, w17: base 0x1e620000, Rn=17 -> (17<<5).
        assert_eq!(scvtf, 0x1e62_0000 | (17 << 5), "SCVTF d0, w17");
        // FMOV x17, d0: base 0x9e660000, Rd=17.
        assert_eq!(fmov_back, 0x9e66_0000 | 17, "FMOV x17, d0");

        // double -> int(w): FMOV d0,x17 (0x9e67_0000) + FCVTZS w17,d0
        // (0x1e38_0000 family).
        let (arena, source) = storage_source(8);
        let bytes =
            encode_runtime_storage_convert(&arena, 0x10, 4, source, 8, true, false, true).unwrap();
        let fmov_in = word_at(&bytes, 12);
        let fcvtzs = word_at(&bytes, 8);
        // FMOV d0, x17: base 0x9e670000, Rn=17 -> (17<<5).
        assert_eq!(fmov_in, 0x9e67_0000 | (17 << 5), "FMOV d0, x17");
        // FCVTZS w17, d0: base 0x1e780000 (double src, 32-bit dst), Rd=17.
        assert_eq!(fcvtzs, 0x1e78_0000 | 17, "FCVTZS w17, d0");
    }

    /// A signed 32->64 int widening must emit SXTW x17,w17; an unsigned widening
    /// (or any non-widening) must emit nothing for the convert step.
    #[test]
    fn convert_int_widening_uses_sxtw_only_when_signed() {
        let (arena, source) = storage_source(4);
        // signed widen: convert step = SXTW (one 4-byte word). Width must include it.
        let signed_width = widths::runtime_storage_convert_width(
            &arena, 0x10, source, 4, 8, false, false, true,
        );
        let unsigned_width = widths::runtime_storage_convert_width(
            &arena, 0x10, source, 4, 8, false, false, false,
        );
        assert_eq!(
            signed_width,
            unsigned_width + 4,
            "signed widen must be exactly one SXTW longer than unsigned"
        );
        let signed_bytes =
            encode_runtime_storage_convert(&arena, 0x10, 8, source, 4, false, false, true).unwrap();
        // SXTW x17, w17: 0x93407c00 | (17<<5) | 17 — it sits right before the store.
        let store_width = if signed_bytes.len() >= 8 { 4 } else { 0 };
        let _ = store_width;
        let sxtw_start = signed_bytes.len() - 8; // SXTW (4) + STR (4)
        let sxtw = u32::from_le_bytes(
            signed_bytes[sxtw_start..sxtw_start + 4].try_into().unwrap(),
        );
        assert_eq!(sxtw, 0x9340_7c00 | (17 << 5) | 17, "SXTW x17, w17");
    }
}
