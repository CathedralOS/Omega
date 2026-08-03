use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, append_unsigned_immediate_padded,
    append_unsigned_immediate_w_padded, encode_add_page_offset_placeholder, encode_add_x_immediate,
    encode_add_x_register, encode_adds_x_register, encode_adrp_placeholder, encode_and_w_low_ones,
    encode_and_w_top_bit, encode_and_x_low_ones, encode_and_x_register, encode_and_x_top_bit,
    encode_asrv_w_register, encode_asrv_x_register, encode_atomic_load, encode_atomic_store,
    encode_brk, encode_cas, encode_cbz_x, encode_compare_w_immediate, encode_compare_w_register,
    encode_compare_x_immediate, encode_compare_x_register,
    encode_compare_x_register_sign_broadcast, encode_conditional_branch_equal,
    encode_conditional_branch_greater, encode_conditional_branch_greater_or_equal,
    encode_conditional_branch_higher, encode_conditional_branch_higher_or_same,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_lower, encode_conditional_branch_lower_or_same,
    encode_conditional_branch_no_overflow, encode_conditional_branch_not_equal,
    encode_conditional_branch_plus, encode_csel_x, encode_csinv_x, encode_eor_x_register,
    encode_float_add, encode_float_compare, encode_float_conditional_select,
    encode_float_convert_double_to_single, encode_float_convert_single_to_double,
    encode_float_divide, encode_float_fused_multiply_add, encode_float_move_from_gpr,
    encode_float_move_to_gpr, encode_float_multiply, encode_float_sqrt, encode_float_subtract,
    encode_float_to_signed_int, encode_float_to_unsigned_int, encode_ldadd, encode_ldclr,
    encode_ldeor, encode_ldset, encode_load_byte_w_from_x, encode_load_byte_w_post_increment,
    encode_load_w_from_x, encode_load_x_from_x, encode_lsl_x_immediate, encode_lslv_w_register,
    encode_lslv_x_register, encode_lsr_x_immediate, encode_lsrv_w_register, encode_lsrv_x_register,
    encode_move_w_register, encode_move_x_register, encode_movz, encode_movz_w,
    encode_msub_w_register, encode_msub_x_register, encode_mul_x_register, encode_mvn_register,
    encode_orr_x_register, encode_read_fpcr, encode_sdiv_w_register, encode_sdiv_x_register,
    encode_sign_extend_byte_to_w, encode_sign_extend_byte_to_x, encode_sign_extend_halfword_to_w,
    encode_sign_extend_halfword_to_x, encode_sign_extend_word_to_x, encode_signed_int_to_float,
    encode_smulh_x, encode_store_byte_w_post_increment, encode_store_byte_w_to_x,
    encode_store_w_to_x, encode_store_w17_to_x16, encode_store_x_to_x, encode_store_x17_to_x16,
    encode_sub_w_register, encode_sub_x_register, encode_subs_x_immediate, encode_subs_x_register,
    encode_swp, encode_udiv_w_register, encode_udiv_x_register, encode_umulh_x,
    encode_unconditional_branch, encode_unsigned_int_to_float, encode_write_fpcr,
    encode_zero_extend_byte_to_w, encode_zero_extend_halfword_to_w,
};
use super::widths::{
    add_constant_width, bit_fragment_container_bytes,
    runtime_frame_base_indexed_address_to_runtime_frame_write_width,
    runtime_frame_base_indexed_binary_write_width, runtime_frame_base_indexed_integer_write_width,
    runtime_frame_fixed_indexed_address_to_runtime_frame_write_width,
    runtime_frame_indexed_address_to_runtime_frame_write_width,
    runtime_frame_indexed_binary_write_width, runtime_frame_indexed_integer_write_width,
    runtime_frame_indexed_string_write_width, runtime_frame_string_write_width,
    runtime_machine_bounded_buffer_literal_append_width,
    runtime_machine_bounded_buffer_source_append_width, runtime_machine_bounded_buffer_write_width,
    runtime_machine_indexed_integer_write_width, runtime_machine_indexed_string_write_width,
    runtime_machine_integer_write_width, runtime_machine_string_write_width,
    runtime_pointee_address_to_runtime_frame_write_width, runtime_pointee_binary_write_width,
    runtime_pointee_bounded_buffer_write_width, runtime_pointee_integer_write_width,
    runtime_pointee_string_write_width, runtime_storage_address_to_runtime_frame_write_width,
    runtime_storage_binary_write_width, runtime_storage_bit_field_write_width,
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

pub fn encode_atomic_load_to_storage(
    source_offset: usize,
    byte_size: usize,
    result_offset: usize,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_load_to_storage_width(
        source_offset,
        byte_size,
        result_offset,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    bytes.extend(encode_atomic_load(17, 16, byte_size, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(17, 16, 0)?),
        _ => unreachable!("atomic-load width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_load_to_storage_width(source_offset, byte_size, result_offset)
    );
    Ok(bytes)
}

pub fn runtime_atomic_load_to_storage_width(
    source_offset: usize,
    _byte_size: usize,
    result_offset: usize,
) -> usize {
    8 + add_constant_width(source_offset) + 4 + 8 + add_constant_width(result_offset) + 4
}

pub fn runtime_atomic_load_result_address_offset(source_offset: usize) -> usize {
    8 + add_constant_width(source_offset) + 4
}

pub fn encode_atomic_store_from_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_store_from_operand_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    append_add_constant_to_x_register(&mut bytes, 16, target_offset)?;
    bytes.extend(encode_atomic_store(17, 16, byte_size, ordering)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_store_from_operand_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_store_from_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    8 + runtime_value_operand_width(runtime_value_operands, value)
        + add_constant_width(target_offset)
        + 4
}

/// `target = source as T`: hold the target base in x16 (untouched by source
/// evaluation, which uses x17/x26/x19), load the source bits into x17, convert
/// them in place between integer/float representations, then store the result at
/// `target_offset`. Mirrors the x86_64 convert path (`cvttsd2si`/`cvtsi2sd`/
/// `cvtsd2ss`/`cvtss2sd` + sized int moves).
#[allow(clippy::too_many_arguments)]
/// AArch64 atomic `fetch_add` via the ordering-selected LSE `LDADD*` form.
/// The single RMW returns its observed prior in x26, which is then stored into
/// the language result place.
/// (An earlier fence-era comment here claimed this was unimplemented; the
/// LDADDAL path is live and pinned by canaries/pass/atomics on arm64 hosts.)
pub fn encode_atomic_fetch_add(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        // The field address is `base + target_offset`; a single ADD immediate
        // only reaches 4095. Atomic fields sit at small offsets in practice;
        // a clear error beats a silent miscompile.
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_add target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        delta,
    ));
    // x16 = the atomic field's storage-region base, relocated at the instruction
    // start (same adrp/add convention as the binary write, so the shared
    // relocation record patches it). The `delta` operand is loaded NEXT, at the
    // binary-write left-operand offset (8), so its relocations land correctly;
    // the address ADD comes AFTER so it never shifts the operand's position.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        delta,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    // LDADD* w17/x17, w26/x26, [x16] -- prior returned in x26.
    bytes.extend(encode_ldadd(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("LDADD width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_add_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_add_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    // adrp + add-page-offset (8) + delta operand load + the address ADD (0 when
    // the offset is 0, else 4) + the single LDADDAL (4).
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, delta)
        + address_add
        + 4
        + 8
        + add_constant_width(result_offset)
        + 4
}

pub fn runtime_atomic_fetch_add_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, delta) + address_add + 4
}

pub fn encode_atomic_fetch_sub(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_sub target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_sub_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        delta,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        delta,
    )?;
    // LDADD of the width-truncated two's-complement negation implements
    // wrapping fetch_sub while preserving the instruction-observed prior.
    bytes.extend(match byte_size {
        1 | 2 | 4 => encode_sub_w_register(17, 31, 17),
        8 => encode_sub_x_register(17, 31, 17),
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic fetch_sub cannot encode a {other}-byte width"
            )));
        }
    });
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldadd(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_sub width validated before LDADD"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_sub_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_sub_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        0,
        result_offset,
        delta,
    ) + 4
}

pub fn runtime_atomic_fetch_sub_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_result_address_offset(runtime_value_operands, target_offset, delta) + 4
}

pub fn encode_atomic_fetch_xor(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_xor target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_xor_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldeor(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_xor width validated before LDEOR"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_xor_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_xor_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    )
}

pub fn runtime_atomic_fetch_xor_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_result_address_offset(runtime_value_operands, target_offset, value)
}

pub fn encode_atomic_fetch_or(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_or target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_or_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldset(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_or width validated before LDSET"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_or_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_or_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    )
}

pub fn runtime_atomic_fetch_or_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_result_address_offset(runtime_value_operands, target_offset, value)
}

pub fn encode_atomic_fetch_and(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic fetch_and target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_and_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        value,
    )?;
    bytes.extend(encode_mvn_register(byte_size, 17, 17)?);
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_ldclr(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("fetch_and width validated before LDCLR"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_and_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_and_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_or_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
    ) + 4
}

pub fn runtime_atomic_fetch_and_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_or_result_address_offset(runtime_value_operands, target_offset, value) + 4
}

pub fn encode_atomic_swap(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic swap target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_swap_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        new_value,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        new_value,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    bytes.extend(encode_swp(byte_size, 17, 26, 16, ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("SWP width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_swap_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            new_value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_swap_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value)
        + address_add
        + 4
        + 8
        + add_constant_width(result_offset)
        + 4
}

pub fn runtime_atomic_swap_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value) + address_add + 4
}

pub fn encode_atomic_compare_exchange(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
    success_ordering: psi_language_core::MemoryOrdering,
) -> Result<Vec<u8>, Diagnostic> {
    if target_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 atomic compare_exchange target offset `{target_offset}` exceeds the \
             single-instruction ADD immediate range (4095)"
        )));
    }
    let mut bytes = Vec::with_capacity(runtime_atomic_compare_exchange_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        expected,
        new_value,
    ));
    // x16 = the atomic field's region base (relocated at the instruction start).
    // new_value loads FIRST at offset 8 (the binary-write left-operand offset, so
    // its relocations land correctly), then expected; the address ADD comes after
    // so it never shifts the operand positions. CAS* clobbers x26 (expected ->
    // prior) and stores x17 (new_value) only on a match.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        new_value,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        expected,
    )?;
    append_add_x_constant(&mut bytes, 16, 16, target_offset, 19)?;
    // CASAL Ws=x26 (expected), Wt=x17 (new_value), [x16].
    bytes.extend(encode_cas(byte_size, 26, 17, 16, success_ordering)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, result_offset)?;
    match byte_size {
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(26, 16, 0, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(26, 16, 0)?),
        _ => unreachable!("CAS width validation accepts only 1, 2, 4, or 8 bytes"),
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_compare_exchange_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            expected,
            new_value
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_compare_exchange_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    _byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    // adrp + add-page-offset (8) + new_value load + expected load + the address
    // ADD (0 when offset is 0, else 4) + the single CASAL (4).
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value)
        + runtime_value_operand_width(runtime_value_operands, expected)
        + address_add
        + 4
        + 8
        + add_constant_width(result_offset)
        + 4
}

pub fn runtime_atomic_compare_exchange_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    let address_add = if target_offset == 0 { 0 } else { 4 };
    8 + runtime_value_operand_width(runtime_value_operands, new_value)
        + runtime_value_operand_width(runtime_value_operands, expected)
        + address_add
        + 4
}

pub fn encode_runtime_storage_convert(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
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
        target_signed,
        trapping,
        saturating,
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
        target_signed,
        trapping,
        saturating,
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
            target_signed,
            trapping,
            saturating,
        ),
        "convert encoder length must match its width"
    );
    Ok(bytes)
}

/// Convert one runtime value and store it into a machine-owned indexed place.
/// Address setup leaves the element address in x16; recursive operand
/// evaluation and conversion use x17 plus the ordinary left-operand scratch
/// bank, which deliberately preserves x16.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_convert_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )?;
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
        target_signed,
        trapping,
        saturating,
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, target_byte_size)?;
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
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<(), Diagnostic> {
    match (source_is_float, target_is_float) {
        (false, true) => {
            // Normalize narrow signed sources in the W register, select SCVTF
            // versus UCVTF from the source identity, then move the float bits
            // back into the GPR.
            if source_signed {
                match source_byte_size {
                    1 => bytes.extend(encode_sign_extend_byte_to_w(register, register)),
                    2 => bytes.extend(encode_sign_extend_halfword_to_w(register, register)),
                    _ => {}
                }
            }
            bytes.extend(if source_signed {
                encode_signed_int_to_float(source_byte_size, target_byte_size, 0, register)?
            } else {
                encode_unsigned_int_to_float(source_byte_size, target_byte_size, 0, register)?
            });
            bytes.extend(encode_float_move_to_gpr(target_byte_size, register, 0)?);
        }
        (true, false) => {
            // float -> int: FMOV the source bits into d0/s0, then FCVTZS Xn/Wn,
            // d0/s0 (round toward zero). The result write truncates to the target
            // width for i8/i16.
            let int_gpr_byte_size = if target_byte_size > 4 { 8 } else { 4 };
            bytes.extend(encode_float_move_from_gpr(source_byte_size, 0, register)?);
            // F4c: a TRAPPING float->int cast guards the VALUE before the
            // conversion -- NaN or out-of-range traps (FCVTZS would silently
            // saturate, which is the Saturating policy, not Trapping's).
            if trapping {
                append_float_to_int_trap_guard(
                    bytes,
                    source_byte_size,
                    target_byte_size,
                    target_signed,
                    register,
                )?;
            }
            bytes.extend(if target_signed {
                encode_float_to_signed_int(source_byte_size, int_gpr_byte_size, register, 0)?
            } else {
                encode_float_to_unsigned_int(source_byte_size, int_gpr_byte_size, register, 0)?
            });
            if saturating && target_byte_size < 4 {
                append_float_to_narrow_int_saturating(
                    bytes,
                    register,
                    target_byte_size,
                    target_signed,
                );
            }
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
            // EVERY narrow (1/2-byte) source extends when widening -- signed
            // sign-extends, unsigned ZERO-extends (mirroring x86's mandatory
            // movzx/movsx: the register may hold a wider bit pattern from an
            // immediate, a folded local, or a chained convert -- `(-1 i16) as
            // u16 as u32` must read 65535, not the full -1 pattern). A 4-byte
            // signed source sign-extends to 64; a 4-byte unsigned source is
            // already correct (w-ops zero-extend).
            if target_byte_size > source_byte_size {
                match (source_byte_size, source_signed) {
                    (1, true) if target_byte_size > 4 => {
                        bytes.extend(encode_sign_extend_byte_to_x(register, register))
                    }
                    (1, true) => bytes.extend(encode_sign_extend_byte_to_w(register, register)),
                    (1, false) => bytes.extend(encode_zero_extend_byte_to_w(register, register)),
                    (2, true) if target_byte_size > 4 => {
                        bytes.extend(encode_sign_extend_halfword_to_x(register, register))
                    }
                    (2, true) => bytes.extend(encode_sign_extend_halfword_to_w(register, register)),
                    (2, false) => {
                        bytes.extend(encode_zero_extend_halfword_to_w(register, register))
                    }
                    (4, true) => bytes.extend(encode_sign_extend_word_to_x(register, register)),
                    _ => {}
                }
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

fn runtime_value_operand_uses_control_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> bool {
    if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        matches!(
            operator,
            StateGuardOperator::AddTowardZero
                | StateGuardOperator::AddTowardPositive
                | StateGuardOperator::AddTowardNegative
        ) || runtime_value_operand_uses_control_state(runtime_value_operands, left)
            || runtime_value_operand_uses_control_state(runtime_value_operands, right)
    } else if let Some((source, ..)) = runtime_value_operands.convert(operand) {
        runtime_value_operand_uses_control_state(runtime_value_operands, source)
    } else {
        false
    }
}

pub fn runtime_value_compare_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> MachineStateSet {
    let mut state = MachineStateSet::new([MachineState::Flags]);
    if runtime_value_operand_uses_control_state(runtime_value_operands, left)
        || runtime_value_operand_uses_control_state(runtime_value_operands, right)
    {
        state = state.union(MachineStateSet::new([MachineState::ControlState]));
    }
    state
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

fn bit_width_mask(width: u16) -> Result<u64, Diagnostic> {
    match width {
        1..=63 => Ok((1_u64 << width) - 1),
        64 => Ok(u64::MAX),
        _ => Err(Diagnostic::error("AArch64 bit-field width must be 1..=64")),
    }
}

fn validate_runtime_bit_field_fragment(
    fragment: &omega_target_operations::RuntimeBitFieldFragment,
) -> Result<usize, Diagnostic> {
    let container_bytes = bit_fragment_container_bytes(fragment)?;
    let destination_end = u32::from(fragment.destination_lsb) + u32::from(fragment.width);
    if destination_end > u32::from(fragment.container_width_bits) {
        return Err(Diagnostic::error(
            "AArch64 bit-field fragment exceeds its destination container",
        ));
    }
    let source_end = u32::from(fragment.source_lsb) + u32::from(fragment.width);
    if source_end > 64 {
        return Err(Diagnostic::error(
            "AArch64 bit-field fragment exceeds its logical scalar",
        ));
    }
    bit_width_mask(fragment.width)?;
    Ok(container_bytes)
}

pub fn encode_runtime_storage_bit_field_write(
    base_byte_offset: usize,
    fragments: &[omega_target_operations::RuntimeBitFieldFragment],
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if fragments.is_empty() {
        return Err(Diagnostic::error(
            "AArch64 bit-field write requires at least one fragment",
        ));
    }
    let mut bytes = Vec::with_capacity(runtime_storage_bit_field_write_width(
        base_byte_offset,
        fragments,
    )?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    for fragment in fragments {
        let container_bytes = validate_runtime_bit_field_fragment(fragment)?;
        let offset = base_byte_offset
            .checked_add(fragment.container_byte_offset)
            .ok_or_else(|| Diagnostic::error("AArch64 bit-field offset overflows"))?;
        let fragment_mask = bit_width_mask(fragment.width)?;
        let destination_mask = fragment_mask
            .checked_shl(u32::from(fragment.destination_lsb))
            .ok_or_else(|| {
                Diagnostic::error("AArch64 bit-field destination mask overflows 64 bits")
            })?;
        let source_bits = (value as u64)
            .checked_shr(u32::from(fragment.source_lsb))
            .unwrap_or(0)
            & fragment_mask;
        let inserted = source_bits
            .checked_shl(u32::from(fragment.destination_lsb))
            .ok_or_else(|| Diagnostic::error("AArch64 bit-field value overflows 64 bits"))?;

        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, container_bytes, 19)?;
        append_unsigned_immediate_padded(&mut bytes, 20, !destination_mask);
        bytes.extend(encode_and_x_register(17, 17, 20));
        append_unsigned_immediate_padded(&mut bytes, 20, inserted);
        bytes.extend(encode_orr_x_register(17, 17, 20));
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, container_bytes, 19)?;
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_bit_field_write_width(base_byte_offset, fragments)?
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

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    use psi_numerics::arithmetic::ArithmeticDomain;
    // Decision 17 (aarch64): Saturating/Trapping add/sub/mul are implemented via a
    // wide (64-bit) op whose result is EXACT for <=32-bit operands, range-compared
    // against the target type's [min,max], then clamped (Saturating: CSEL-style
    // branch+move) or trapped (Trapping: BRK on out-of-range). Exact/Wrapping use
    // the default width-correct op (the aarch64 op + truncating store already
    // wraps), so they are unchanged. Float domains are always Exact/Wrapping here.
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    // SIGNED Saturating div/mod need the TYPE_MIN / -1 fixup (see
    // append_saturating_signed_divide_modulo). Unsigned div/mod never overflow, and
    // Trapping div/mod ride aarch64 `sdiv` (which does not fault), so both fall
    // through to the normal path below.
    let saturating_signed_divide_modulo = domain == ArithmeticDomain::Saturating
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        );
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        target_offset,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
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
        // Comparisons run at the OPERAND width (a bool target is 1 byte, but
        // the FMOV/FCMP need the f32/f64 width); arithmetic keeps the target
        // width, which equals the operand width for float targets.
        append_runtime_float_binary_operation(
            &mut bytes,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
            17,
            operator,
            26,
            domain,
            // x15/x14 are free on the write path (the float arm uses only
            // x17/x26 + v0/v1); the F5 guard clobbers them.
            [15, 14],
        )?;
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add
                | StateGuardOperator::Subtract
                | StateGuardOperator::Multiply
                | StateGuardOperator::ShiftLeft
        )
    {
        // Wide-width op + range-compare clamp/trap. Result left in x17; x16 (target
        // base) and x20 (saved base for indexed targets) are NOT touched.
        append_saturating_trapping_arithmetic(
            &mut bytes,
            domain,
            operator,
            byte_size,
            target_signed,
            17,
            26,
            &[15, 14, 13, 12],
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )?;
    } else if saturating_signed_divide_modulo {
        // TYPE_MIN / -1 fixup for signed Saturating div/mod (result left in x17).
        append_saturating_signed_divide_modulo(
            &mut bytes,
            byte_size,
            matches!(operator, StateGuardOperator::Modulo),
            17,
            26,
            9,
        )?;
    } else {
        append_runtime_binary_operation_with_domain(
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
            domain,
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
            domain,
            target_signed,
        )
    );
    Ok(bytes)
}

/// Saturating/Trapping integer add/sub/mul (decision 17, aarch64). The operands
/// are already in x17 (left) and x26 (right). A 64-bit op produces the EXACT
/// result for operands of 4 bytes or narrower (it cannot overflow 64 bits), so the
/// full result is range-compared against the target type's [min,max] and either
/// clamped (Saturating) or trapped (Trapping). The final value is left in x17.
///
/// Register use: x17 = result, x26 = scratch (the spent right operand) holding the
/// active clamp bound, x19 = scratch holding the alternate bound. x16 (target base)
/// and x20 (saved base for indexed targets) are deliberately untouched so the
/// surrounding store still addresses the target. Width MUST equal
/// `saturating_trapping_arithmetic_width` in `widths.rs`.
fn append_saturating_trapping_arithmetic(
    bytes: &mut Vec<u8>,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
    dest: u8,
    rhs: u8,
    scratch: &[u8],
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> Result<(), Diagnostic> {
    use psi_numerics::arithmetic::ArithmeticDomain;
    // Register-parametric so the OPERAND-position lowering (fused arithmetic
    // under a guard compare) can reuse these proven sequences at whatever
    // dest/rhs the operand evaluator assigned. The binary WRITE path passes
    // dest=17, rhs=26, scratch=[15,14,13,12] -- byte-identical to the
    // pre-parameterization hardcoded registers. `rhs` doubles as the narrow
    // paths' bound scratch (its operand value is spent by then).
    let [high_scratch, immediate_scratch, sign_scratch, bound_scratch] = match scratch {
        [a, b, c, d, ..] => [*a, *b, *c, *d],
        _ => {
            return Err(Diagnostic::error(
                "AArch64 saturating/trapping arithmetic needs four scratch registers",
            ));
        }
    };
    if byte_size == 8 {
        // A 64-bit op can itself overflow 64 bits, so the wide-result range
        // compare below cannot see it. Add/sub instead use the FLAGS the
        // x86_64 backend uses: ADDS/SUBS set C (unsigned carry/borrow) and V
        // (signed overflow), and CSEL/CSINV realize the clamp branchlessly.
        // 64-bit multiply overflow: the MULH high half is the witness --
        // signed overflow iff SMULH != low >> 63 (the sign broadcast);
        // unsigned iff UMULH != 0. The high half computes BEFORE the low
        // half overwrites x17.
        if operator == StateGuardOperator::ShiftLeft {
            // F8c: a TRAPPING out-of-range COUNT traps before the value
            // math -- regardless of x (`0 << 70` traps). The recovery
            // witness below keeps its own count-caveat branches for the
            // Saturating clamp.
            if domain == ArithmeticDomain::Trapping {
                append_shift_count_trap_guard(bytes, rhs, byte_size)?;
            }
            // 64-bit `<<` loses the shifted-out bits, so the witness is
            // RECOVERY: y = x << n, then y >> n (arithmetic for signed,
            // logical for unsigned) equals x exactly when nothing was lost.
            // Two count caveats need explicit compares (LSLV masks the count
            // mod 64, so recovery alone cannot see them): a count >= 64
            // overflows every nonzero x, and x == 0 never overflows at any
            // count. Layout: recovery-mismatch branches to the fixup;
            // count < 64 branches to keep; x == 0 branches to keep; fall
            // into the fixup (clamp by x's sign / all-ones / brk).
            let fixup_bytes: isize = match (domain, target_signed) {
                // cmp x,#0 + csinv MIN/MAX.
                (ArithmeticDomain::Saturating, true) => 8,
                // movz/movk*3 padded u64::MAX.
                (ArithmeticDomain::Saturating, false) => 16,
                // brk.
                _ => 4,
            };
            bytes.extend(encode_move_x_register(sign_scratch, dest)); // save x
            if domain == ArithmeticDomain::Saturating && target_signed {
                append_unsigned_immediate(bytes, bound_scratch, i64::MIN as u64); // 2 instr
            }
            bytes.extend(encode_lslv_x_register(dest, dest, rhs));
            bytes.extend(if target_signed {
                encode_asrv_x_register(high_scratch, dest, rhs)
            } else {
                encode_lsrv_x_register(high_scratch, dest, rhs)
            });
            bytes.extend(encode_compare_x_register(high_scratch, sign_scratch));
            bytes.extend(encode_conditional_branch_not_equal(20)?); // -> fixup
            bytes.extend(encode_compare_x_immediate(rhs, 64)?);
            bytes.extend(encode_conditional_branch_lower(12 + fixup_bytes)?); // -> keep
            bytes.extend(encode_subs_x_immediate(31, sign_scratch, 0)?); // cmp x, #0
            bytes.extend(encode_conditional_branch_equal(4 + fixup_bytes)?); // -> keep
            match (domain, target_signed) {
                (ArithmeticDomain::Saturating, true) => {
                    bytes.extend(encode_subs_x_immediate(31, sign_scratch, 0)?);
                    // MI (x negative) -> MIN, else NOT(MIN) = MAX.
                    bytes.extend(encode_csinv_x(dest, bound_scratch, bound_scratch, 0b0100));
                }
                (ArithmeticDomain::Saturating, false) => {
                    append_unsigned_immediate_padded(bytes, dest, u64::MAX);
                }
                _ => bytes.extend(encode_brk(0)),
            }
            return Ok(());
        }
        if matches!(operator, StateGuardOperator::Multiply) {
            match (domain, target_signed) {
                (ArithmeticDomain::Saturating, true) => {
                    // smulh x15 ; eor x13 = true-result sign ; x12 = MIN ;
                    // mul x17 ; cmp x15, x17 asr #63 ; b.eq +12 (keep) ;
                    // cmp x13, #0 ; csinv x17, x12, x12, MI (MIN, else MAX).
                    bytes.extend(encode_smulh_x(high_scratch, dest, rhs));
                    bytes.extend(encode_eor_x_register(sign_scratch, dest, rhs));
                    append_unsigned_immediate(bytes, bound_scratch, i64::MIN as u64); // 2 instr
                    bytes.extend(encode_mul_x_register(dest, dest, rhs));
                    bytes.extend(encode_compare_x_register_sign_broadcast(high_scratch, dest));
                    bytes.extend(encode_conditional_branch_equal(12)?);
                    bytes.extend(encode_subs_x_immediate(31, sign_scratch, 0)?); // cmp sign_scratch, #0
                    bytes.extend(encode_csinv_x(dest, bound_scratch, bound_scratch, 0b0100)); // MI -> MIN, else MAX
                }
                (ArithmeticDomain::Saturating, false) => {
                    // umulh x15 ; mul x17 ; cmp x15, #0 ;
                    // csinv x17, x17, xzr, EQ (keep, else all-ones).
                    bytes.extend(encode_umulh_x(high_scratch, dest, rhs));
                    bytes.extend(encode_mul_x_register(dest, dest, rhs));
                    bytes.extend(encode_subs_x_immediate(31, high_scratch, 0)?);
                    bytes.extend(encode_csinv_x(dest, dest, 31, 0b0000)); // EQ
                }
                (ArithmeticDomain::Trapping, true) => {
                    bytes.extend(encode_smulh_x(high_scratch, dest, rhs));
                    bytes.extend(encode_mul_x_register(dest, dest, rhs));
                    bytes.extend(encode_compare_x_register_sign_broadcast(high_scratch, dest));
                    bytes.extend(encode_conditional_branch_equal(8)?);
                    bytes.extend(encode_brk(0));
                }
                (ArithmeticDomain::Trapping, false) => {
                    bytes.extend(encode_umulh_x(high_scratch, dest, rhs));
                    bytes.extend(encode_mul_x_register(dest, dest, rhs));
                    bytes.extend(encode_subs_x_immediate(31, high_scratch, 0)?);
                    bytes.extend(encode_conditional_branch_equal(8)?);
                    bytes.extend(encode_brk(0));
                }
                _ => unreachable!("only Saturating/Trapping reach this helper"),
            }
            return Ok(());
        }
        let subtract = matches!(operator, StateGuardOperator::Subtract);
        match (domain, target_signed) {
            (ArithmeticDomain::Saturating, true) => {
                // x14 = i64::MIN, then on overflow the RESULT's sign is the
                // INVERSE of the true sign: N set (negative result) means the
                // true result was positive -> saturate to MAX = NOT(MIN).
                //   movz/movk x14 = MIN ; adds/subs ; b.vc +8 (keep)
                //   csinv x17, x14, x14, PL   (PL: MIN; else NOT(MIN) = MAX)
                append_unsigned_immediate(bytes, immediate_scratch, i64::MIN as u64); // 2 instr
                bytes.extend(if subtract {
                    encode_subs_x_register(dest, dest, rhs)
                } else {
                    encode_adds_x_register(dest, dest, rhs)
                });
                bytes.extend(encode_conditional_branch_no_overflow(8)?);
                bytes.extend(encode_csinv_x(
                    dest,
                    immediate_scratch,
                    immediate_scratch,
                    0b0101,
                )); // PL
            }
            (ArithmeticDomain::Saturating, false) => {
                if subtract {
                    // Borrow clears C: keep the result on CS, else clamp to 0.
                    bytes.extend(encode_subs_x_register(dest, dest, rhs));
                    bytes.extend(encode_csel_x(dest, dest, 31, 0b0010)); // CS -> keep, else XZR
                } else {
                    // Carry sets C: keep on CC, else all-ones (u64::MAX).
                    bytes.extend(encode_adds_x_register(dest, dest, rhs));
                    bytes.extend(encode_csinv_x(dest, dest, 31, 0b0011)); // CC -> keep, else NOT(XZR)
                }
            }
            (ArithmeticDomain::Trapping, true) => {
                bytes.extend(if subtract {
                    encode_subs_x_register(dest, dest, rhs)
                } else {
                    encode_adds_x_register(dest, dest, rhs)
                });
                bytes.extend(encode_conditional_branch_no_overflow(8)?);
                bytes.extend(encode_brk(0));
            }
            (ArithmeticDomain::Trapping, false) => {
                if subtract {
                    bytes.extend(encode_subs_x_register(dest, dest, rhs));
                    bytes.extend(encode_conditional_branch_higher_or_same(8)?); // CS = no borrow
                } else {
                    bytes.extend(encode_adds_x_register(dest, dest, rhs));
                    bytes.extend(encode_conditional_branch_lower(8)?); // CC = no carry
                }
                bytes.extend(encode_brk(0));
            }
            _ => unreachable!("only Saturating/Trapping reach this helper"),
        }
        return Ok(());
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping arithmetic cannot handle {byte_size}-byte targets yet on aarch64"
        )));
    }

    if operator == StateGuardOperator::ShiftLeft {
        // F8c: a TRAPPING out-of-range COUNT traps before the value math --
        // regardless of x (`0 << 40` traps; the count is invalid, not the
        // result). Saturating cannot reach one post-F8a; its count cap
        // below stays for robustness.
        if domain == ArithmeticDomain::Trapping {
            append_shift_count_trap_guard(bytes, rhs, byte_size)?;
        }
        // Narrow `<<`: cap the COUNT at the type width w -- any count >= w
        // overflows every nonzero x, and the cap keeps the 64-bit LSLV EXACT
        // (|x| <= 2^31-ish shifted by <= 32 fits 64 bits) -- then range-check
        // the exact value. The count reads UNSIGNED (a negative signed count
        // is huge unsigned and caps to w, matching the interpreter's
        // `count as u64 >= width`); only the VALUE register sign-extends.
        // Unsigned targets take a SINGLE unsigned upper-bound check: x >= 0
        // shifted left cannot go below zero, and the wide value can exceed
        // i64::MAX (2^31 << 32), which the shared add/sub/mul tail's SIGNED
        // lower compare would misread as negative and clamp to 0.
        if target_signed && !left_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => encode_sign_extend_byte_to_x(dest, dest),
                2 => encode_sign_extend_halfword_to_x(dest, dest),
                _ => encode_sign_extend_word_to_x(dest, dest),
            });
        }
        let width_bits = (8 * byte_size) as u64;
        append_unsigned_immediate_padded(bytes, high_scratch, width_bits);
        bytes.extend(encode_compare_x_immediate(rhs, width_bits as u32)?);
        bytes.extend(encode_csel_x(rhs, rhs, high_scratch, 0b0011)); // LO -> keep, else w
        bytes.extend(encode_lslv_x_register(dest, dest, rhs));
        let unsigned_max: u64 = (1u64 << (8 * byte_size)) - 1;
        let signed_min = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
        let signed_max = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
        if target_signed {
            // Both bounds; the exact wide value stays within i64 (cap = w).
            append_unsigned_immediate_padded(bytes, rhs, signed_min);
            bytes.extend(encode_compare_x_register(dest, rhs));
            bytes.extend(encode_conditional_branch_greater_or_equal(8)?);
            bytes.extend(if domain == ArithmeticDomain::Saturating {
                encode_move_x_register(dest, rhs)
            } else {
                encode_brk(0)
            });
            append_unsigned_immediate_padded(bytes, rhs, signed_max);
            bytes.extend(encode_compare_x_register(dest, rhs));
            bytes.extend(encode_conditional_branch_less_or_equal(8)?);
            bytes.extend(if domain == ArithmeticDomain::Saturating {
                encode_move_x_register(dest, rhs)
            } else {
                encode_brk(0)
            });
        } else {
            append_unsigned_immediate_padded(bytes, rhs, unsigned_max);
            bytes.extend(encode_compare_x_register(dest, rhs));
            bytes.extend(encode_conditional_branch_lower_or_same(8)?);
            bytes.extend(if domain == ArithmeticDomain::Saturating {
                encode_move_x_register(dest, rhs)
            } else {
                encode_brk(0)
            });
        }
        return Ok(());
    }

    // STORAGE operands were loaded zero-extended at the target width: for
    // signed targets, sign-extend them to 64 bits so the wide op sees the
    // true signed values (a negative i8 -50 is 0xCE = 206 zero-extended).
    // IMMEDIATE operands are already their true wide value (the loader
    // emits the full i64) -- re-extending one from the target width
    // CORRUPTS it: 2147483648 re-read as i32 is -2^31, which flipped the
    // MIN idiom `0 - 2147483648` into 0 - (-2^31) and saturated to MAX
    // (the pinned sat_narrow_wide_literal_operand_divergence).
    if target_signed {
        let extend_one: fn(u8) -> [u8; 4] = match byte_size {
            1 => |register| encode_sign_extend_byte_to_x(register, register),
            2 => |register| encode_sign_extend_halfword_to_x(register, register),
            _ => |register| encode_sign_extend_word_to_x(register, register),
        };
        if !left_is_wide_immediate {
            bytes.extend(extend_one(dest));
        }
        if !right_is_wide_immediate {
            bytes.extend(extend_one(rhs));
        }
    }

    // Wide (64-bit) op: x17 = x17 OP x26. Exact for <=32-bit operands.
    match operator {
        StateGuardOperator::Add => bytes.extend(encode_add_x_register(dest, dest, rhs)),
        StateGuardOperator::Subtract => bytes.extend(encode_sub_x_register(dest, dest, rhs)),
        StateGuardOperator::Multiply => bytes.extend(encode_mul_x_register(dest, dest, rhs)),
        _ => unreachable!("only +/-/* reach the saturating/trapping arithmetic helper"),
    }

    let unsigned_max: u64 = (1u64 << (8 * byte_size)) - 1;
    let signed_min = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
    let signed_max = ((1i128 << (8 * byte_size - 1)) - 1) as u64;

    match (domain, target_signed) {
        (ArithmeticDomain::Saturating, false) => {
            // Unsigned wide results overflow in ONE direction per operator:
            // subtract only DOWNWARD (the wrapped wide underflow reads
            // signed-negative, so the lower compare is SIGNED `b.ge` -- an
            // unsigned compare against 0 is vacuously true and never
            // clamps), add/mul only UPWARD. The upper compare must be
            // UNSIGNED (`b.ls`): a u32 product can exceed 2^63, whose
            // SIGNED reading is negative -- the old both-checks tail ran
            // the signed lower compare first and clamped 4e9 * 4e9 to 0
            // instead of MAX.
            if operator == StateGuardOperator::Subtract {
                append_unsigned_immediate_padded(bytes, rhs, 0);
                bytes.extend(encode_compare_x_register(dest, rhs));
                bytes.extend(encode_conditional_branch_greater_or_equal(8)?);
                bytes.extend(encode_move_x_register(dest, rhs));
            } else {
                append_unsigned_immediate_padded(bytes, rhs, unsigned_max);
                bytes.extend(encode_compare_x_register(dest, rhs));
                bytes.extend(encode_conditional_branch_lower_or_same(8)?);
                bytes.extend(encode_move_x_register(dest, rhs));
            }
        }
        (ArithmeticDomain::Saturating, true) => {
            // Signed: clamp to [MIN, MAX] using signed comparisons on the exact
            // 64-bit result.
            //   movz/movk x26, #MIN ; cmp x17,x26 ; b.ge +8 ; mov x17,x26
            //   movz/movk x26, #MAX ; cmp x17,x26 ; b.le +8 ; mov x17,x26
            append_unsigned_immediate_padded(bytes, rhs, signed_min);
            bytes.extend(encode_compare_x_register(dest, rhs));
            bytes.extend(encode_conditional_branch_greater_or_equal(8)?);
            bytes.extend(encode_move_x_register(dest, rhs));
            append_unsigned_immediate_padded(bytes, rhs, signed_max);
            bytes.extend(encode_compare_x_register(dest, rhs));
            bytes.extend(encode_conditional_branch_less_or_equal(8)?);
            bytes.extend(encode_move_x_register(dest, rhs));
        }
        (ArithmeticDomain::Trapping, false) => {
            // Same one-direction-per-operator shape as the Saturating arm:
            // subtract traps on the signed-negative underflow reading;
            // add/mul trap on the UNSIGNED upper compare (a 2^63+ product's
            // signed reading is negative and would have trapped as a
            // phantom "underflow" -- right outcome, wrong witness).
            if operator == StateGuardOperator::Subtract {
                append_unsigned_immediate_padded(bytes, rhs, 0);
                bytes.extend(encode_compare_x_register(dest, rhs));
                bytes.extend(encode_conditional_branch_greater_or_equal(8)?);
                bytes.extend(encode_brk(0));
            } else {
                append_unsigned_immediate_padded(bytes, rhs, unsigned_max);
                bytes.extend(encode_compare_x_register(dest, rhs));
                bytes.extend(encode_conditional_branch_lower_or_same(8)?);
                bytes.extend(encode_brk(0));
            }
        }
        (ArithmeticDomain::Trapping, true) => {
            // Signed: trap unless MIN <= result <= MAX.
            //   movz/movk x26, #MIN ; cmp x17,x26 ; b.ge +8 ; brk
            //   movz/movk x26, #MAX ; cmp x17,x26 ; b.le +8 ; brk
            append_unsigned_immediate_padded(bytes, rhs, signed_min);
            bytes.extend(encode_compare_x_register(dest, rhs));
            bytes.extend(encode_conditional_branch_greater_or_equal(8)?);
            bytes.extend(encode_brk(0));
            append_unsigned_immediate_padded(bytes, rhs, signed_max);
            bytes.extend(encode_compare_x_register(dest, rhs));
            bytes.extend(encode_conditional_branch_less_or_equal(8)?);
            bytes.extend(encode_brk(0));
        }
        _ => unreachable!("only Saturating/Trapping reach this helper"),
    }
    Ok(())
}

/// SATURATING SIGNED divide/modulo (dividend x17, divisor x26, result x17; scratch x9).
/// Integer division overflows ONLY at TYPE_MIN / -1 — and unlike x86 `idiv`, aarch64
/// `sdiv` does not trap there (it returns TYPE_MIN, the WRAPPED value) nor on divide-by-
/// zero (returns 0). So Saturating only needs to fix the single `divisor == -1` corner:
/// `a % -1 == 0`, and `a / -1 == -a` clamped so TYPE_MIN saturates to TYPE_MAX instead of
/// wrapping to TYPE_MIN. Every other divisor (including 0) goes through the normal
/// `sdiv`/`sdiv`+`msub`. Operands are sign-extended to 64 bits first so the exact result
/// of a <=32-bit op lives in the low word and truncates correctly on store. Unsigned
/// saturating div/mod never overflow and never reach here (the normal path handles them).
fn append_saturating_signed_divide_modulo(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
    dest: u8,
    rhs: u8,
    scratch: u8,
) -> Result<(), Diagnostic> {
    // Register-parametric like append_saturating_trapping_arithmetic: the
    // binary WRITE path passes dest=17, rhs=26, scratch=9 (byte-identical to
    // the pre-parameterization hardcoded registers); the OPERAND-position
    // lowering passes the operand evaluator's assigned registers.
    if !matches!(byte_size, 1 | 2 | 4) {
        // 64-bit saturating div would need TYPE_MIN detection that `neg`/`mul -1`
        // cannot signal at the full width (it wraps); not needed by any live sample.
        return Err(Diagnostic::error(
            "saturating divide/modulo on 64-bit integers is not implemented yet on aarch64"
                .to_owned(),
        ));
    }
    let signed_max = ((1i128 << (8 * byte_size - 1)) - 1) as u64;

    // The `divisor == -1` fixup block.
    let mut special: Vec<u8> = Vec::new();
    if want_remainder {
        // a % -1 == 0.
        special.extend(encode_movz(dest, 0));
    } else {
        // a / -1 == -a: multiply by the -1 still sitting in x9, then clamp a
        // TYPE_MIN result (-a == TYPE_MAX+1) down to TYPE_MAX.
        special.extend(encode_mul_x_register(dest, dest, scratch));
        append_unsigned_immediate_padded(&mut special, scratch, signed_max);
        special.extend(encode_compare_x_register(dest, scratch));
        special.extend(encode_conditional_branch_less_or_equal(8)?); // <= MAX -> keep
        special.extend(encode_move_x_register(dest, scratch)); // else clamp to MAX
    }

    // The normal path (every divisor except -1).
    let mut normal: Vec<u8> = Vec::new();
    if want_remainder {
        normal.extend(encode_sdiv_x_register(scratch, dest, rhs)); // q = a / b
        normal.extend(encode_msub_x_register(dest, scratch, rhs, dest)); // a - q*b
    } else {
        normal.extend(encode_sdiv_x_register(dest, dest, rhs));
    }

    // Sign-extend both operands to 64 bits.
    match byte_size {
        1 => {
            bytes.extend(encode_sign_extend_byte_to_x(dest, dest));
            bytes.extend(encode_sign_extend_byte_to_x(rhs, rhs));
        }
        2 => {
            bytes.extend(encode_sign_extend_halfword_to_x(dest, dest));
            bytes.extend(encode_sign_extend_halfword_to_x(rhs, rhs));
        }
        _ => {
            bytes.extend(encode_sign_extend_word_to_x(dest, dest));
            bytes.extend(encode_sign_extend_word_to_x(rhs, rhs));
        }
    }
    // x9 = -1; branch to `normal` unless the divisor is exactly -1.
    append_unsigned_immediate_padded(bytes, scratch, u64::MAX);
    bytes.extend(encode_compare_x_register(rhs, scratch));
    bytes.extend(encode_conditional_branch_not_equal(
        (8 + special.len()) as isize,
    )?);
    bytes.extend(special);
    bytes.extend(encode_unconditional_branch((4 + normal.len()) as isize)?);
    bytes.extend(normal);
    Ok(())
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
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_machine_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_string_write_width(byte_length) + 40);
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_store_data_to_x_offset(&mut bytes, 17, 16, byte_offset, 8, 15)?;
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    append_store_data_to_x_offset(&mut bytes, 17, 16, byte_offset + 8, 8, 15)?;
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

/// Write a string literal into an owned `[u8; N]` byte carrier held directly in
/// machine storage (`self.buffer = "150"`). The carrier is `{ len: u64, bytes:
/// [u8; N] }`: store the length word at `[base + off]`, then each literal byte
/// inline at `[base + off + 8 + i]`. Content is immediate, so the base --
/// materialized by the leading `adrp`+`add` placeholder pair, patched to the
/// machine storage base by the relocation pass (the single
/// `insert_data_address_at_instruction_start` reloc, arch-shared with the string
/// writes) -- is the only relocation, mirroring the x86_64 carrier write.
///
/// Every emitted element is a fixed 4-byte AArch64 instruction (immediates live
/// in the instruction word, not as inline data bytes), so the sequence is
/// inherently instruction-aligned. Previously this op errored at encode while its
/// layout width borrowed the x86_64 (odd) width, so a forward branch skipping the
/// block that contained it computed a non-4-aligned distance and failed with a
/// misleading "b.ne target is not instruction aligned".
pub fn encode_runtime_machine_bounded_buffer_write(
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_write_width(
        byte_offset,
        literal,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = machine storage base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize the carrier address once. STRB's unsigned immediate tops out
    // at 4095 bytes, while large machines routinely place text carriers later
    // in storage; rebasing also keeps every per-byte store small.
    append_add_x_constant(&mut bytes, 16, 16, byte_offset, 15)?;
    append_unsigned_immediate(&mut bytes, 17, literal.len() as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 0)?); // [carrier] = len word
    for (index, byte) in literal.as_bytes().iter().enumerate() {
        append_unsigned_immediate(&mut bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_to_x(17, 16, 8 + index)?);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_write_width(byte_offset, literal)
    );
    Ok(bytes)
}

/// Write a string literal into an owned `[u8; N]` carrier reached THROUGH a
/// stored pointer (`rooms[0].label = "Gate"`): load the pointer from
/// `frame[pointer_byte_offset]` into x16, then store `{len, bytes}` inline at
/// `*ptr + field`. Content is immediate, so the frame base (the leading
/// `adrp`+`add`, relocated at instruction start) is the only relocation --
/// mirroring the x86_64 pointee carrier write.
pub fn encode_runtime_pointee_bounded_buffer_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_bounded_buffer_write_width(
        pointer_byte_offset,
        field_byte_offset,
        literal,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = frame base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee carrier",
    )?; // x16 = stored pointer
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    append_unsigned_immediate(&mut bytes, 17, literal.len() as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 0)?); // [*ptr + field] = len word
    for (index, byte) in literal.as_bytes().iter().enumerate() {
        append_unsigned_immediate(&mut bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_to_x(17, 16, 8 + index)?);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_bounded_buffer_write_width(pointer_byte_offset, field_byte_offset, literal)
    );
    Ok(bytes)
}

/// Append a string LITERAL onto an owned `[u8; N]` carrier at its running
/// length (a later concat segment, e.g. the trailing `" =="`). x16 = machine
/// storage base (the only relocation, at instruction start); x15 = running
/// length; x14 = byte cursor (`base + target + 8 + len`, advanced by
/// post-increment stores); the literal bytes are immediates. The new length
/// (`len + literal.len`) is stored last.
pub fn encode_runtime_machine_bounded_buffer_literal_append(
    target_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_literal_append_width(
        target_byte_offset,
        literal,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = machine storage base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(15, 16, target_byte_offset)?); // x15 = running len
    append_add_x_constant(&mut bytes, 14, 16, target_byte_offset + 8, 13)?; // x14 = bytes base
    bytes.extend(encode_add_x_register(14, 14, 15)); // x14 = write cursor (bytes + len)
    for byte in literal.as_bytes() {
        append_unsigned_immediate(&mut bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_post_increment(17, 14, 1)?);
    }
    bytes.extend(encode_add_x_immediate(15, 15, literal.len())?); // len += literal.len
    bytes.extend(encode_store_x_to_x(15, 16, target_byte_offset)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_literal_append_width(target_byte_offset, literal)
    );
    Ok(bytes)
}

/// Append a source carrier's content onto a target carrier (concat builder
/// source segment, after the first literal initialized the target). x16 = the
/// machine storage base (target; relocated at instruction start); a frame-local
/// source (`let`-local struct's carrier) adds a frame-base `adrp`+`add` pair for
/// x14 right after (relocated at the arch-aware +8 -- see the relocation
/// record). x15 = target running len, x13 = source len (consumed as the copy
/// counter), x12/x11 = source/target byte cursors, w17 = byte scratch. The new
/// length is stored BEFORE the copy loop, which decrements x13 to zero -- the
/// same must-precede rule as the x86_64 `rep movsb` encoder.
pub fn encode_runtime_machine_bounded_buffer_source_append(
    target_byte_offset: usize,
    source_byte_offset: usize,
    source_in_frame: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_source_append_width(
        target_byte_offset,
        source_byte_offset,
        source_in_frame,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = machine storage base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    let source_base = if source_in_frame {
        bytes.extend(encode_adrp_placeholder(14)); // x14 = frame base (reloc @ +8)
        bytes.extend(encode_add_page_offset_placeholder(14));
        14
    } else {
        16
    };
    bytes.extend(encode_load_x_from_x(15, 16, target_byte_offset)?); // x15 = target len
    bytes.extend(encode_load_x_from_x(13, source_base, source_byte_offset)?); // x13 = source len
    append_add_x_constant(&mut bytes, 12, source_base, source_byte_offset + 8, 10)?; // x12 = src bytes
    append_add_x_constant(&mut bytes, 11, 16, target_byte_offset + 8, 10)?; // x11 = dst bytes base
    bytes.extend(encode_add_x_register(11, 11, 15)); // x11 = dst cursor (bytes + len)
    // new len = target_len + source_len -- MUST precede the loop, which
    // consumes x13 as it copies; computing it after would always add 0.
    bytes.extend(encode_add_x_register(15, 15, 13));
    bytes.extend(encode_store_x_to_x(15, 16, target_byte_offset)?);
    // Bounded byte copy:
    //   loop: cbz  x13, done   (+20: skip ldrb/strb/subs/b)
    //         ldrb w17, [x12], #1
    //         strb w17, [x11], #1
    //         subs x13, x13, #1
    //         b    loop        (-16)
    //   done:
    bytes.extend(encode_cbz_x(13, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(17, 12, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(17, 11, 1)?);
    bytes.extend(encode_subs_x_immediate(13, 13, 1)?);
    bytes.extend(encode_unconditional_branch(-16)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_source_append_width(
            target_byte_offset,
            source_byte_offset,
            source_in_frame
        )
    );
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
    index_byte_size: usize,
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
        16,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
    index_byte_size: usize,
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
        index_byte_size,
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
    index_region: omega_target_operations::RuntimeStorageRegion,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_address_to_runtime_frame_write_width(
        index_region,
        element_byte_size,
        field_byte_offset,
        target_offset,
    ));
    append_runtime_frame_index_target_address_with_index_region(
        &mut bytes,
        16,
        index_region,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
        16,
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
    index_byte_size: usize,
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
        16,
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_indexed_integer_write_with_index_region(
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        value,
    )
}

pub fn encode_runtime_frame_indexed_integer_write_with_index_region(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
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
    append_runtime_frame_index_target_address_with_index_region(
        &mut bytes,
        16,
        index_region,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_base_indexed_integer_write_width(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        16,
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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

/// The machine-indexed ADDRESS write: `frame[target] = &machine[base + idx*size
/// + field]` -- the SS5b wide-referee recast (`&self.buf[k] as &Wide`) binds the
/// frame slot to the ELEMENT ADDRESS (reads deref it; a wider-than-pointer
/// referee cannot content-spill). The address computation is the SAME prefix as
/// the machine-indexed copies (`append_runtime_machine_index_target_address`
/// into x16, machine page pair relocated at instruction start + the frame
/// index's own page pair for a RuntimeFrame index), so the relocation walker
/// reuses the copy family's offset fns; then the target frame page pair (x17)
/// and an 8-byte store of x16 (scratch x9 materializes a large target offset).
pub fn encode_runtime_machine_indexed_address_to_runtime_frame_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_machine_indexed_address_to_runtime_frame_write_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
        ),
    );
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_store_data_to_x_offset(&mut bytes, 16, 17, target_offset, 8, 9)?;
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_machine_indexed_address_to_runtime_frame_write_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
        )
    );
    Ok(bytes)
}

pub fn encode_runtime_machine_indexed_integer_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_integer_write_width(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
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
    index_byte_size: usize,
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
        16,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
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
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        16,
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

/// RMW into a machine-resident indexed element (`self.tallies[k] += 1`): the
/// machine-index address helper walks x16 to the element (its optional
/// frame-index pair sits at the constant the string-write offset helper
/// exposes), the operands evaluate into x17/x26 (preserving x16), and the
/// result stores at [x16, 0] -- the machine-region mirror of the working
/// frame-base flavor above.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(super::widths::runtime_machine_indexed_binary_write_width(
        runtime_value_operands,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
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
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_runtime_frame_indexed(
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
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
        16,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
    index_byte_size: usize,
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
        16,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
    index_byte_size: usize,
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
        16,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
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
    index_byte_size: usize,
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
        16,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        17,
        26,
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
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
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

/// Write-side mirror of
/// [`encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`].
/// x86_64-only for now; aarch64 emits nothing real.
/// Write `machine[index] = <machine-storage source>` — the store-side mirror of
/// `encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`.
/// Computes the ELEMENT address (base + index*scale + field) into x16 exactly as
/// the read does, computes the SOURCE address (source region + `source_offset`)
/// into x20, then LOADs from the source (x20) and STOREs into the element (x16) --
/// the load/store bases are swapped relative to the read. The source page pair's
/// SYMBOL is chosen by the relocation record (machine for a field source, the
/// runtime frame for a slot-backed local source); the emitted bytes are
/// identical either way.
pub fn encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
    source_offset: usize,
    base_byte_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width(
            source_offset,
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_count,
        ),
    );
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, source_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
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

/// Exact scratch footprint of the direct-source to runtime-pointee encoder
/// above. Base/result registers are unconditional; x19 participates only when
/// a base adjustment exceeds one ADD immediate, while x19/x26 participate when
/// a chunk offset needs an address scratch.
pub fn runtime_storage_copy_to_runtime_pointee_clobbers(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(20)];
    if byte_count > 0 {
        registers.push(MachineRegister::Aarch64X(17));
    }
    if source_offset > 4095 || pointer_byte_offset > 4095 || field_byte_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        if !data_offset_encodable(offset, chunk_size) {
            registers.extend([MachineRegister::Aarch64X(19), MachineRegister::Aarch64X(26)]);
        }
        Ok(())
    })
    .expect("runtime copy chunk partition is total");
    RegisterSet::new(registers)
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
    address_register: u8,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    append_runtime_frame_index_target_address_with_index_region(
        bytes,
        address_register,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        index_scratch,
        scale_scratch,
    )
}

/// Machine-storage flavor: a MACHINE-resident index (a subslice start held in a
/// machine field) materializes its own page pair into x21 at the CONSTANT
/// offset 32 (after the frame pair + the fixed-width descriptor load), which
/// the relocation record patches to the machine symbol.
fn append_runtime_frame_index_target_address_with_index_region(
    bytes: &mut Vec<u8>,
    address_register: u8,
    index_region: omega_target_operations::RuntimeStorageRegion,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    append_runtime_frame_index_target_address_with_index_width(
        bytes,
        address_register,
        index_region,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        index_scratch,
        scale_scratch,
    )
}

fn append_runtime_frame_index_target_address_with_index_width(
    bytes: &mut Vec<u8>,
    address_register: u8,
    index_region: omega_target_operations::RuntimeStorageRegion,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_fixed_width_load_x_from_x_offset(bytes, address_register, 20, descriptor_offset, 19);
    let index_base = if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        // The fixed-width index load below uses x21 as its offset scratch.
        // Prefer x15 for the machine base, except when the caller already
        // holds the pointee address there (runtime value operands do); x19 is
        // free after the descriptor load and is excluded from caller picks.
        let machine_base = if address_register == 15 { 19 } else { 15 };
        bytes.extend(encode_adrp_placeholder(machine_base)); // machine base [reloc @ 32]
        bytes.extend(encode_add_page_offset_placeholder(machine_base));
        machine_base
    } else {
        20
    };
    append_fixed_width_load_unsigned_index_from_x_offset(
        bytes,
        index_scratch,
        index_base,
        index_offset,
        index_byte_size,
        21,
    );
    append_scale_x_register_by_constant(bytes, scale_scratch, index_scratch, element_byte_size)?;
    bytes.extend(encode_add_x_register(
        address_register,
        address_register,
        scale_scratch,
    ));
    append_add_constant_to_x_register(bytes, address_register, field_byte_offset)?;
    Ok(())
}

fn append_runtime_frame_fixed_index_target_address(
    bytes: &mut Vec<u8>,
    address_register: u8,
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
    append_load_data_from_x_offset(bytes, address_register, 20, descriptor_offset, 8, 19)?;
    append_add_constant_to_x_register(bytes, address_register, byte_offset)?;
    Ok(())
}

/// Fixed-shape element-address recipe for the dual/double-indexed encoders:
/// unlike `append_runtime_machine_index_target_address` (whose adds/loads are
/// value-dependent in width), every element here is a fixed 4-byte instruction,
/// so the positions of the relocated `adrp` pairs are REGION-DEPENDENT
/// CONSTANTS -- which is what the relocation-offset helpers (which only see the
/// regions, never the offsets) require. Shape:
///
///   mov  x20, <base>                      (4)  index base default
///   [frame index: adrp/add x20]           (8)  frame base (RELOCATED)
///   ldr{b,h,w,x} x17, [x20, #index_offset] (4) exact selected-width index
///   movz x26, #element_byte_size          (4)
///   mul  x26, x17, x26                    (4)
///   add  <base>, <base>, x26              (4)
///   add  <base>, <base>, #(array + field) (4)  unconditional (#0 is valid)
///
/// The index offset must fit the LDR scaled immediate and the combined
/// array+field offset must fit the ADD immediate; both fail LOUDLY otherwise.
fn append_fixed_shape_index_element_address(
    bytes: &mut Vec<u8>,
    base_register: u8,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    combined_byte_offset: usize,
) -> Result<(), Diagnostic> {
    if element_byte_size > 0xffff {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot scale a runtime index by element size \
             `{element_byte_size}` yet"
        )));
    }
    bytes.extend(encode_move_x_register(20, base_register));
    if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        bytes.extend(encode_adrp_placeholder(20));
        bytes.extend(encode_add_page_offset_placeholder(20));
    }
    append_direct_unsigned_index_load(bytes, 17, 20, index_offset, index_byte_size)?;
    bytes.extend(encode_movz(26, element_byte_size as u16));
    bytes.extend(encode_mul_x_register(26, 17, 26));
    bytes.extend(encode_add_x_register(base_register, base_register, 26));
    bytes.extend(encode_add_x_immediate(
        base_register,
        base_register,
        combined_byte_offset,
    )?);
    Ok(())
}

/// Fixed-shape double-index address math (36 bytes, nine 4-byte instructions):
/// after the caller has materialized the machine base into x16 (and, when any
/// index is frame-resident, the frame base into x15), walk x16 to the element
/// `base + outer*outer_stride + inner*inner_stride + combined_offset`. Every
/// element is fixed width so the relocated adrp positions around it are
/// constants. Clobbers x14/x17/x26.
#[allow(clippy::too_many_arguments)]
fn append_double_index_address_math(
    bytes: &mut Vec<u8>,
    outer_base_register: u8,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_base_register: u8,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    combined_byte_offset: usize,
) -> Result<(), Diagnostic> {
    for stride in [outer_stride, inner_stride] {
        if stride > 0xffff {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot scale a double index by stride `{stride}` yet"
            )));
        }
    }
    append_direct_unsigned_index_load(
        bytes,
        17,
        outer_base_register,
        outer_index_offset,
        outer_index_byte_size,
    )?;
    append_direct_unsigned_index_load(
        bytes,
        26,
        inner_base_register,
        inner_index_offset,
        inner_index_byte_size,
    )?;
    bytes.extend(encode_movz(14, outer_stride as u16));
    bytes.extend(encode_mul_x_register(17, 17, 14));
    bytes.extend(encode_movz(14, inner_stride as u16));
    bytes.extend(encode_mul_x_register(26, 26, 14));
    bytes.extend(encode_add_x_register(16, 16, 17));
    bytes.extend(encode_add_x_register(16, 16, 26));
    bytes.extend(encode_add_x_immediate(16, 16, combined_byte_offset)?);
    Ok(())
}

fn append_direct_unsigned_index_load(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
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
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot load {byte_size}-byte runtime indexes yet"
            )));
        }
    }
    Ok(())
}

/// Materialize the double-indexed bases: the machine base into x16 (relocated
/// at instruction start) and -- when any index is frame-resident -- the SHARED
/// frame base into x15 (relocated at the constant
/// `runtime_machine_double_indexed_frame_base_offset` = 8). Returns the
/// (outer_base, inner_base) index-load registers.
fn append_double_index_bases(
    bytes: &mut Vec<u8>,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> (u8, u8) {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if outer_index_region == frame || inner_index_region == frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    (
        if outer_index_region == frame { 15 } else { 16 },
        if inner_index_region == frame { 15 } else { 16 },
    )
}

/// Read `grid[i][j]` (both indices runtime) into a storage slot: the
/// double-index address math walks x16 to the element, the element loads into
/// x17, then a second relocated base pair addresses the target region for the
/// store. Historically silently dropped on aarch64 (the zero-width hole).
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot read {byte_count}-byte double-indexed values yet"
        )));
    }
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
            outer_index_region,
            inner_index_region,
        ),
    );
    let (outer_base, inner_base) =
        append_double_index_bases(&mut bytes, outer_index_region, inner_index_region);
    append_double_index_address_math(
        &mut bytes,
        outer_base,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_base,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    match byte_count {
        8 => bytes.extend(encode_load_x_from_x(17, 16, 0)?),
        _ => bytes.extend(encode_load_w_from_x(17, 16, 0, byte_count)?),
    }
    // Target base (relocated at `..target_base_offset`, a constant per frame-ness).
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_count {
        8 => bytes.extend(encode_store_x_to_x(17, 16, target_offset)?),
        _ => bytes.extend(encode_store_w_to_x(17, 16, target_offset, byte_count)?),
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
            outer_index_region,
            inner_index_region,
        )
    );
    Ok(bytes)
}

/// Write `grid[i][j] = <storage slot>` -- the source value loads into x24
/// FIRST (right after the base pairs, while x16 is still the unbiased machine
/// base; the shared frame pair also serves a frame-resident SOURCE), then the
/// address math walks x16 to the element and x24 stores there.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_count}-byte double-indexed values yet"
        )));
    }
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
            source_region,
            outer_index_region,
            inner_index_region,
        ),
    );
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if source_region == frame || outer_index_region == frame || inner_index_region == frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    let source_base = if source_region == frame { 15 } else { 16 };
    match byte_count {
        8 => bytes.extend(encode_load_x_from_x(24, source_base, source_offset)?),
        _ => bytes.extend(encode_load_w_from_x(
            24,
            source_base,
            source_offset,
            byte_count,
        )?),
    }
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == frame { 15 } else { 16 },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == frame { 15 } else { 16 },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    match byte_count {
        8 => bytes.extend(encode_store_x_to_x(24, 16, 0)?),
        _ => bytes.extend(encode_store_w_to_x(24, 16, 0, byte_count)?),
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
            source_region,
            outer_index_region,
            inner_index_region,
        )
    );
    Ok(bytes)
}

/// Read `g[i][j]` from a FRAME-resident 2D array (a `let`/param local): one
/// frame pair serves the array and both indices, then the shared address math,
/// the element load, and the relocated target pair + store. Every offset is a
/// pure constant (60-byte total).
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot read {byte_count}-byte double-indexed values yet"
        )));
    }
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(),
    );
    bytes.extend(encode_adrp_placeholder(16)); // frame base [reloc @ 0]
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_double_index_address_math(
        &mut bytes,
        16,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        16,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    match byte_count {
        8 => bytes.extend(encode_load_x_from_x(17, 16, 0)?),
        _ => bytes.extend(encode_load_w_from_x(17, 16, 0, byte_count)?),
    }
    bytes.extend(encode_adrp_placeholder(16)); // target base [reloc @ 48]
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_count {
        8 => bytes.extend(encode_store_x_to_x(17, 16, target_offset)?),
        _ => bytes.extend(encode_store_w_to_x(17, 16, target_offset, byte_count)?),
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width()
    );
    Ok(bytes)
}

/// Write twin: `grid[i][j] = <literal>` -- the same address math, then the
/// value immediate materialized into x17 (AFTER every relocation, so its
/// variable width perturbs no reloc offset) and stored at the element.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_integer_write(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_size}-byte double-indexed values yet"
        )));
    }
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_machine_double_indexed_integer_write_width(
            outer_index_region,
            inner_index_region,
            value,
        ),
    );
    let (outer_base, inner_base) =
        append_double_index_bases(&mut bytes, outer_index_region, inner_index_region);
    append_double_index_address_math(
        &mut bytes,
        outer_base,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_base,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    append_unsigned_immediate(&mut bytes, 17, value as u64);
    match byte_size {
        8 => bytes.extend(encode_store_x_to_x(17, 16, 0)?),
        _ => bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?),
    }
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_machine_double_indexed_integer_write_width(
            outer_index_region,
            inner_index_region,
            value,
        )
    );
    Ok(bytes)
}

/// Copy a FRAME-resident inline array element at a runtime index into another
/// frame slot (`let v = arr[i]` where `arr` and `i` are locals/params): ONE
/// frame pair serves the element address, the index, and the target slot --
/// the unbiased base is stashed in x24 before the element math biases x16, and
/// the chunk stores land at `[x24 + target_offset + chunk]`. Single relocation
/// (the record's arch-aware target-frame offset is None for aarch64).
pub fn encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame_width(
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(16)); // frame base [reloc @ 0]
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(24, 16)); // unbiased base for the target
    // The index lives in the SAME region as the base, so the fixed-shape
    // recipe's same-region flavor (no extra page pair) applies.
    append_fixed_shape_index_element_address(
        &mut bytes,
        16,
        omega_target_operations::RuntimeStorageRegion::Machine,
        index_offset,
        index_byte_size,
        element_byte_size,
        base_byte_offset + field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(0, target_offset, byte_count, |offset, chunk_size| {
        let source_offset = offset;
        let target_chunk_offset = target_offset + offset;
        match chunk_size {
            8 => {
                bytes.extend(encode_load_x_from_x(17, 16, source_offset)?);
                bytes.extend(encode_store_x_to_x(17, 24, target_chunk_offset)?);
            }
            _ => {
                bytes.extend(encode_load_w_from_x(17, 16, source_offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(
                    17,
                    24,
                    target_chunk_offset,
                    chunk_size,
                )?);
            }
        }
        Ok(())
    })?;
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame_width(
            target_offset,
            byte_count,
        )
    );
    Ok(bytes)
}

/// RMW into a double-indexed element (`grid[i][j] += 1`): the double-index
/// bases + math walk x16 to the element, the operands evaluate into x17/x26
/// (preserving x16), and the result stores at [x16, 0].
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_machine_double_indexed_binary_write_width(
            runtime_value_operands,
            outer_index_region,
            inner_index_region,
            byte_size,
            left,
            operator,
            right,
        ),
    );
    let (outer_base, inner_base) =
        append_double_index_bases(&mut bytes, outer_index_region, inner_index_region);
    append_double_index_address_math(
        &mut bytes,
        outer_base,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_base,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
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
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

/// Copy `machine[j] -> machine[i]` where BOTH indices are runtime values
/// (`arr[i] = arr[j]`): compute the source element address (fixed shape,
/// stashed in x24), compute the target element address (a second relocated
/// machine base), then chunk-copy through x17. Historically this op was
/// silently DROPPED on aarch64 (the zero-width emission hole); the layout
/// guard now makes any regression loud.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
    source_base_byte_offset: usize,
    source_index_offset: usize,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    source_index_byte_size: usize,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_index_offset: usize,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_byte_size: usize,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_machine_indexed_to_machine_indexed_width(
            source_index_region,
            target_index_region,
            byte_count,
        ),
    );
    // Source element address -> x16 -> stash x24.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_fixed_shape_index_element_address(
        &mut bytes,
        16,
        source_index_region,
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    bytes.extend(encode_move_x_register(24, 16));
    // Target element address -> x16 (the second relocated machine base sits at
    // `..second_base_offset`, a region-dependent constant).
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_fixed_shape_index_element_address(
        &mut bytes,
        16,
        target_index_region,
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_base_byte_offset + target_field_byte_offset,
    )?;
    // Chunk-copy source (x24) -> target (x16) through x17.
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            8 => {
                bytes.extend(encode_load_x_from_x(17, 24, offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => {
                bytes.extend(encode_load_w_from_x(17, 24, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
        }
        Ok(())
    })?;
    debug_assert_eq!(
        bytes.len(),
        super::widths::runtime_storage_copy_machine_indexed_to_machine_indexed_width(
            source_index_region,
            target_index_region,
            byte_count,
        )
    );
    Ok(bytes)
}

fn append_runtime_machine_index_target_address(
    bytes: &mut Vec<u8>,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    append_add_constant_to_x_register(bytes, 16, base_byte_offset)?;
    // Load the index at its declared width and zero-extend narrow forms so
    // adjacent slot bytes cannot be spliced into the address. `append_load_data_from_x_offset`
    // materializes a large `index_offset` (a loop counter declared AFTER a big array,
    // offset > 16380) into scratch x19 — it moves the base (x20) into x19 first, so
    // x20 is preserved. Its width is `machine_index_load_width(index_region,
    // index_offset)`, which the width + relocation-address-offset functions consume in
    // lockstep so the source/target adrp positions stay exact for large offsets.
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            bytes.extend(encode_adrp_placeholder(20));
            bytes.extend(encode_add_page_offset_placeholder(20));
            append_load_data_from_x_offset(bytes, 17, 20, index_offset, index_byte_size, 19)?;
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            append_load_data_from_x_offset(bytes, 17, 20, index_offset, index_byte_size, 19)?;
        }
    }
    append_scale_x_register_by_constant(bytes, 26, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 26));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

/// `index_scratch`/`scale_scratch` are parameters because this runs in TWO
/// register climates: write-TARGET address setup (pre-operands; 17/26 are
/// free -- the historical hardcodes) and OPERAND-position evaluation, where
/// hardcoded 17 CLOBBERED the left operand's result while addressing the
/// right one (`self.double(arr[i])` doubled the INDEX: d = i + arr[i] -- the
/// local-array value-operand ZII/garbage divergence; x86_64 is immune because
/// it stashes the left result on the stack).
fn append_runtime_frame_base_index_target_address(
    bytes: &mut Vec<u8>,
    address_register: u8,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    append_runtime_frame_base_index_target_address_with_index_width(
        bytes,
        address_register,
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        index_scratch,
        scale_scratch,
    )
}

fn append_runtime_frame_base_index_target_address_with_index_width(
    bytes: &mut Vec<u8>,
    address_register: u8,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    index_scratch: u8,
    scale_scratch: u8,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(address_register, 20));
    append_add_constant_to_x_register(bytes, address_register, base_byte_offset)?;
    append_load_data_from_x_offset(bytes, index_scratch, 20, index_offset, index_byte_size, 19)?;
    append_scale_x_register_by_constant(bytes, scale_scratch, index_scratch, element_byte_size)?;
    bytes.extend(encode_add_x_register(
        address_register,
        address_register,
        scale_scratch,
    ));
    append_add_constant_to_x_register(bytes, address_register, field_byte_offset)?;
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
    } else if let Some((_, base_byte_offset, _, fragments)) =
        runtime_value_operands.bit_field(operand)
    {
        if fragments.is_empty() {
            return Err(Diagnostic::error(
                "AArch64 bit-field operand requires at least one fragment",
            ));
        }
        if matches!(destination_register, 19 | 20 | 21) {
            return Err(Diagnostic::error(
                "AArch64 bit-field operand destination conflicts with reserved assembly registers",
            ));
        }
        bytes.extend(encode_adrp_placeholder(19));
        bytes.extend(encode_add_page_offset_placeholder(19));
        bytes.extend(encode_movz(destination_register, 0));
        for fragment in &fragments {
            let container_bytes = validate_runtime_bit_field_fragment(fragment)?;
            let offset = base_byte_offset
                .checked_add(fragment.container_byte_offset)
                .ok_or_else(|| Diagnostic::error("AArch64 bit-field offset overflows"))?;
            append_load_data_from_x_offset(bytes, 20, 19, offset, container_bytes, 21)?;
            if fragment.destination_lsb != 0 {
                bytes.extend(encode_lsr_x_immediate(
                    20,
                    20,
                    fragment.destination_lsb as u8,
                ));
            }
            append_unsigned_immediate_padded(bytes, 21, bit_width_mask(fragment.width)?);
            bytes.extend(encode_and_x_register(20, 20, 21));
            if fragment.source_lsb != 0 {
                bytes.extend(encode_lsl_x_immediate(20, 20, fragment.source_lsb as u8));
            }
            bytes.extend(encode_orr_x_register(
                destination_register,
                destination_register,
                20,
            ));
        }
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
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_indexed(operand)
    {
        // Index/scale scratch from the OPERAND scratch list: the historical
        // hardcoded 17/26 clobbered the left operand's already-computed
        // result (x17) while addressing the RIGHT operand of a fused binary
        // (`self.double(arr[i])` doubled the index instead of the element --
        // the local-array value-operand divergence; x86_64 is immune, it
        // stashes the left result on the stack). Exclude the helper's
        // internal registers (15 address, 19/20/21 bases/offset scratch) and
        // this operand's own destination.
        let mut scratch_picks = scratch_registers
            .iter()
            .copied()
            .filter(|register| !matches!(register, 15 | 19 | 20 | 21))
            .filter(|register| *register != destination_register);
        let (Some(index_scratch), Some(scale_scratch)) =
            (scratch_picks.next(), scratch_picks.next())
        else {
            return Err(Diagnostic::error(
                "AArch64 MVP encoder ran out of scratch registers for an indexed operand",
            ));
        };
        append_runtime_frame_index_target_address_with_index_width(
            bytes,
            // x15, NOT x16: the caller may hold its own address in x16 across
            // operand evaluation (a binary write's target base, an indexed
            // RMW's element address). Loading this operand's element through
            // x16 clobbered that and sent the caller's store to a wild
            // address (the transition-arg slice-sum SIGSEGV).
            15,
            index_region,
            descriptor_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            index_scratch,
            scale_scratch,
        )?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                15,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 15, 0)?),
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
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        // Index/scale scratch from the OPERAND scratch list: the historical
        // hardcoded 17/26 clobbered the left operand's already-computed
        // result (x17) while addressing the RIGHT operand of a fused binary
        // (`self.double(arr[i])` doubled the index instead of the element --
        // the local-array value-operand divergence; x86_64 is immune, it
        // stashes the left result on the stack). Exclude the helper's
        // internal registers (15 address, 19/20/21 bases/offset scratch) and
        // this operand's own destination.
        let mut scratch_picks = scratch_registers
            .iter()
            .copied()
            .filter(|register| !matches!(register, 15 | 19 | 20 | 21))
            .filter(|register| *register != destination_register);
        let (Some(index_scratch), Some(scale_scratch)) =
            (scratch_picks.next(), scratch_picks.next())
        else {
            return Err(Diagnostic::error(
                "AArch64 MVP encoder ran out of scratch registers for an indexed operand",
            ));
        };
        append_runtime_frame_base_index_target_address_with_index_width(
            bytes,
            // x15, NOT x16: the caller may hold its own address in x16 across
            // operand evaluation (a binary write's target base, an indexed
            // RMW's element address). Loading this operand's element through
            // x16 clobbered that and sent the caller's store to a wild
            // address (the transition-arg slice-sum SIGSEGV).
            15,
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            index_scratch,
            scale_scratch,
        )?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                15,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 15, 0)?),
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
            // x15, NOT x16: the caller may hold its own address in x16 across
            // operand evaluation (a binary write's target base, an indexed
            // RMW's element address). Loading this operand's element through
            // x16 clobbered that and sent the caller's store to a wild
            // address (the transition-arg slice-sum SIGSEGV).
            15,
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
        )?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                15,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 15, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load runtime fixed indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.machine_indexed(operand)
    {
        // MACHINE-owned array element in operand position: machine base pair
        // at the operand start (relocated), then -- for a frame-resident
        // index -- the frame pair at the PINNED offset 8 (see
        // machine_indexed_operand_frame_index_base_offset). Index/scale
        // scratch come from the operand scratch list exactly like the frame
        // arms (the hardcoded-x17 clobber lesson).
        let mut scratch_picks = scratch_registers
            .iter()
            .copied()
            .filter(|register| !matches!(register, 15 | 19 | 20 | 21))
            .filter(|register| *register != destination_register);
        let (Some(index_scratch), Some(scale_scratch)) =
            (scratch_picks.next(), scratch_picks.next())
        else {
            return Err(Diagnostic::error(
                "AArch64 MVP encoder ran out of scratch registers for a machine-indexed operand",
            ));
        };
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
        let index_base =
            if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                bytes.extend(encode_adrp_placeholder(20));
                bytes.extend(encode_add_page_offset_placeholder(20));
                20
            } else {
                15
            };
        append_load_data_from_x_offset(
            bytes,
            index_scratch,
            index_base,
            index_offset,
            index_byte_size,
            19,
        )?;
        append_scale_x_register_by_constant(
            bytes,
            scale_scratch,
            index_scratch,
            element_byte_size,
        )?;
        bytes.extend(encode_add_x_register(15, 15, scale_scratch));
        append_add_constant_to_x_register(bytes, 15, base_byte_offset + field_byte_offset)?;
        match byte_size {
            1 | 2 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                15,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 15, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load machine-indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((
        _,
        left_offset,
        left_is_bounded_buffer,
        _,
        right_offset,
        right_is_bounded_buffer,
    )) = runtime_value_operands.text_equals(operand)
    {
        append_runtime_text_equals_operand(
            bytes,
            destination_register,
            scratch_registers,
            left_offset,
            left_is_bounded_buffer,
            right_offset,
            right_is_bounded_buffer,
        )?;
        Ok(())
    } else if let Some((place, literal, place_is_bounded_buffer)) =
        runtime_value_operands.text_equals_literal(operand)
    {
        append_runtime_text_equals_literal_operand(
            runtime_value_operands,
            bytes,
            destination_register,
            scratch_registers,
            place,
            &literal,
            place_is_bounded_buffer,
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
            // them. The width is THREADED from build time
            // (binary_byte_width, set once from the operands' scalar type) --
            // a const-folded f32 field pair becomes IMMEDIATE operands with no
            // storage width, and the old storage-size fallback then ran the
            // add at double precision over f32 bit patterns (the
            // f32_field_binary_to_local canary). Storage sizes remain the
            // fallback for operands built before the width was threaded.
            // MUST stay the fixed runtime_float_binary_operation_width().
            let byte_size = runtime_value_operands
                .binary_byte_width(operand)
                .or_else(|| runtime_value_operand_value_byte_size(runtime_value_operands, left))
                .or_else(|| runtime_value_operand_value_byte_size(runtime_value_operands, right))
                .unwrap_or(8);
            append_runtime_float_binary_operation(
                bytes,
                byte_size,
                destination_register,
                operator,
                rhs_register,
                runtime_value_operands
                    .binary_arithmetic_domain(operand)
                    .map(|(domain, _)| domain)
                    .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact),
                // x15/x14 are outside the operand register set on this path;
                // the F5 guard clobbers them.
                [15, 14],
            )?;
        } else if let Some((domain, operands_signed)) = runtime_value_operands
            .binary_arithmetic_domain(operand)
            .filter(|(domain, _)| {
                matches!(
                    domain,
                    psi_numerics::arithmetic::ArithmeticDomain::Saturating
                        | psi_numerics::arithmetic::ArithmeticDomain::Trapping
                )
            })
            .filter(|_| {
                matches!(
                    operator,
                    StateGuardOperator::Add
                        | StateGuardOperator::Subtract
                        | StateGuardOperator::Multiply
                        | StateGuardOperator::ShiftLeft
                )
            })
        {
            // Decision 17 in OPERAND position: reuse the binary WRITE path's
            // register-parametric clamp/trap sequences at this operand's
            // dest/rhs. The operand's byte_width is its REAL scalar width here
            // (set at construction for non-Exact domains); the remaining
            // scratch supplies the sequences' immediate/high/sign/bound
            // registers.
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            append_saturating_trapping_arithmetic(
                bytes,
                domain,
                operator,
                byte_width,
                operands_signed,
                destination_register,
                rhs_register,
                remaining_scratch,
                runtime_value_operands.immediate_integer(left).is_some(),
                runtime_value_operands.immediate_integer(right).is_some(),
            )?;
        } else if runtime_value_operands
            .binary_arithmetic_domain(operand)
            .is_some_and(|(domain, operands_signed)| {
                domain == psi_numerics::arithmetic::ArithmeticDomain::Saturating
                    && operands_signed
                    && matches!(
                        operator,
                        StateGuardOperator::Divide | StateGuardOperator::Modulo
                    )
            })
        {
            // Signed Saturating div/mod in OPERAND position: the TYPE_MIN/-1
            // fixup (a / -1 clamps TYPE_MIN to TYPE_MAX; a % -1 == 0), same
            // register-parametric reuse as the arithmetic arm above. Wrapping
            // div/mod need NO arm here: aarch64 `sdiv` wraps naturally (the
            // x86_64 backend guards its trapping `idiv` instead). Trapping
            // div/mod fall through -- pre-existing aarch64 behavior (`sdiv`
            // does not fault), matching the write path.
            let Some((&div_scratch, _)) = remaining_scratch.split_first() else {
                return Err(Diagnostic::error(
                    "AArch64 MVP encoder ran out of scratch registers for runtime arithmetic",
                ));
            };
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            append_saturating_signed_divide_modulo(
                bytes,
                byte_width,
                matches!(operator, StateGuardOperator::Modulo),
                destination_register,
                rhs_register,
                div_scratch,
            )?;
        } else {
            // Comparisons use the operand width; other nested binaries do not
            // carry their result width, so assume 64-bit (matches the x86_64
            // backend).
            append_runtime_binary_operation_with_domain(
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
                runtime_value_operands
                    .binary_arithmetic_domain(operand)
                    .map(|(domain, _)| domain)
                    .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact),
            )?;
            // A nested WRAPPING binary must hand its PARENT the width-wrapped
            // VALUE: the plain 64-bit op leaves the untruncated result
            // (0u32 - 2 = 0xFFFF_FFFF_FFFF_FFFE in the register), and a
            // sign/width-sensitive parent (>>, /, %, comparisons) then reads
            // it wrong -- native diverged from the interpreter, which wraps
            // AT THE NODE (decision 17). The store-truncation-is-the-wrap
            // shortcut only holds at the WRITE, never in operand position.
            // Extension picks the node's own signedness; Exact is proven
            // non-overflowing and Saturating/Trapping clamp/trap above.
            // Width tracked in widths.rs -- MUST stay in lockstep.
            if let Some((psi_numerics::arithmetic::ArithmeticDomain::Wrapping, operands_signed)) =
                runtime_value_operands.binary_arithmetic_domain(operand)
                && let Some(byte_width) = runtime_value_operands.binary_byte_width(operand)
                && byte_width < 8
            {
                append_wrapping_operand_truncation(
                    bytes,
                    destination_register,
                    byte_width,
                    operands_signed,
                );
            }
        }
        Ok(())
    } else if let Some((
        source,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
    )) = runtime_value_operands.convert(operand)
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
            runtime_value_operands.convert_target_signed(operand),
            runtime_value_operands.convert_trapping(operand),
            runtime_value_operands.convert_saturating(operand),
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
    left_is_bounded_buffer: bool,
    right_offset: usize,
    right_is_bounded_buffer: bool,
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
    if left_is_bounded_buffer {
        append_fixed_width_address_from_x_offset(
            bytes,
            left_ptr,
            19,
            left_offset + 8,
            byte_scratch,
        );
        append_fixed_width_load_x_from_x_offset(bytes, left_len, 19, left_offset, byte_scratch);
    } else {
        append_fixed_width_load_x_from_x_offset(bytes, left_ptr, 19, left_offset, byte_scratch);
        append_fixed_width_load_x_from_x_offset(bytes, left_len, 19, left_offset + 8, byte_scratch);
    }

    // Right descriptor: page relocated at the pinned right-base offset.
    debug_assert_eq!(
        bytes.len() - operand_start,
        super::widths::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
        "right descriptor page must sit at the pinned relocation offset"
    );
    bytes.extend(encode_adrp_placeholder(19));
    bytes.extend(encode_add_page_offset_placeholder(19));
    if right_is_bounded_buffer {
        append_fixed_width_address_from_x_offset(
            bytes,
            right_ptr,
            19,
            right_offset + 8,
            byte_scratch,
        );
        append_fixed_width_load_x_from_x_offset(bytes, right_len, 19, right_offset, byte_scratch);
    } else {
        append_fixed_width_load_x_from_x_offset(bytes, right_ptr, 19, right_offset, byte_scratch);
        append_fixed_width_load_x_from_x_offset(
            bytes,
            right_len,
            19,
            right_offset + 8,
            byte_scratch,
        );
    }

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
    bytes.extend(encode_load_byte_w_post_increment(
        byte_scratch,
        left_ptr,
        1,
    )?);
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
/// `destination = (place == literal)` as bool 0/1, where `place` names either
/// the String side's `{ptr @ +0, len @ +8}` text descriptor or -- when
/// `place_is_bounded_buffer` -- an owned `[u8; N]` carrier whose layout is
/// `{len @ +0, bytes inline @ +8}` (same address setups: a relocated storage
/// base, a pointee field behind a frame pointer slot, or a frame-indexed /
/// frame-base-indexed / frame-fixed-indexed element field). The literal's
/// expected bytes are compared as inline immediates -- no rodata descriptor
/// exists for the literal side. The carrier and descriptor reads are
/// width-identical (an `add` computing the inline bytes address vs a pointer
/// load, one `ldr` each for the length), so the shared width is
/// `runtime_text_equals_literal_operand_width` (place-setup plus a fixed
/// head plus 12 bytes per literal byte), independent of the flag -- mirroring
/// the x86_64 encoder's same-width carrier branch.
///
/// Register use: the place address setup lands the descriptor address in the
/// FOURTH pool scratch -- never x16 (a binary WRITE holds its target base
/// there; the old x16 setup sent the store to a wild address) and never a
/// FIXED register that a pool may also hand out: a fixed x15 collided with
/// the RIGHT pool's first pick, so `ptr_register` was also x15 and its load
/// destroyed the address before the len read (texteq as the RIGHT operand of
/// `&&` read garbage; LEFT position survived only because x15 was the len
/// register there -- a read-then-write last use). Drawing ptr/len/byte/addr
/// from the pool makes collision impossible by construction. Indexed setups
/// still clobber x19/x21 scratch; both pools exclude x17/x26, so the
/// sibling operand's home is never touched.
fn append_runtime_text_equals_literal_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination_register: u8,
    scratch_registers: &[u8],
    place: RuntimeValueOperandHandle,
    literal: &str,
    place_is_bounded_buffer: bool,
) -> Result<(), Diagnostic> {
    let [
        ptr_register,
        len_register,
        byte_scratch,
        address_register,
        ..,
    ] = *scratch_registers
    else {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder ran out of scratch registers for runtime text literal equality",
        ));
    };
    let operand_start = bytes.len();

    // Descriptor address -> x16. The relocated page materialization sits at
    // the operand start (the relocation planner targets it there).
    if let Some((_, byte_offset, _)) = runtime_value_operands.storage(place) {
        bytes.extend(encode_adrp_placeholder(address_register));
        bytes.extend(encode_add_page_offset_placeholder(address_register));
        append_add_constant_to_x_register(bytes, address_register, byte_offset)?;
    } else if let Some((pointer_byte_offset, field_byte_offset, _)) =
        runtime_value_operands.pointee(place)
    {
        // x16 = frame base (relocated page pair), then the stored pointer.
        // The descriptor sits in the POINTEE at the field offset -- never
        // read the pointer slot's own bytes as a descriptor.
        bytes.extend(encode_adrp_placeholder(address_register));
        bytes.extend(encode_add_page_offset_placeholder(address_register));
        append_runtime_storage_load(
            bytes,
            address_register,
            address_register,
            pointer_byte_offset,
            8,
            "runtime text pointee",
        )?;
        if field_byte_offset > 0 {
            append_add_constant_to_x_register(bytes, address_register, field_byte_offset)?;
        }
    } else if let Some((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_indexed(place)
    {
        append_runtime_frame_index_target_address_with_index_width(
            bytes,
            address_register,
            index_region,
            descriptor_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            17,
            26,
        )?;
    } else if let Some((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_base_indexed(place)
    {
        append_runtime_frame_base_index_target_address_with_index_width(
            bytes,
            address_register,
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            17,
            26,
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
            address_register,
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

    if place_is_bounded_buffer {
        // Owned carrier `{len@0, bytes@8}`: the bytes ADDRESS is computed
        // (x15 + 8, not a stored pointer) and the length is read at offset 0.
        // Width-identical to the descriptor path (one `add` + one `ldr` vs two
        // `ldr`s), so branch offsets and the operand width are unchanged.
        bytes.extend(encode_add_x_immediate(ptr_register, address_register, 8)?);
        bytes.extend(encode_load_x_from_x(len_register, address_register, 0)?);
    } else {
        bytes.extend(encode_load_x_from_x(ptr_register, address_register, 0)?);
        bytes.extend(encode_load_x_from_x(len_register, address_register, 8)?);
    }

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
/// Narrow SIGNED divide/modulo operands may arrive ZERO-extended (the
/// guard-subject load path), so a 32-bit `sdiv` would divide i8 -20 as 236.
/// Sign-extend both to the operation width first -- idempotent when they are
/// already sign-extended (the storage-write path); unsigned division is
/// correct zero-extended and skips this. Mirrors the x86_64
/// `append_integer_divide_modulo_core` fix.
fn append_narrow_signed_division_operand_extension(
    bytes: &mut Vec<u8>,
    signed: bool,
    byte_size: usize,
    left_register: u8,
    right_register: u8,
) {
    if !signed {
        return;
    }
    for register in [left_register, right_register] {
        match byte_size {
            1 => bytes.extend(encode_sign_extend_byte_to_w(register, register)),
            2 => bytes.extend(encode_sign_extend_halfword_to_w(register, register)),
            _ => {}
        }
    }
}

/// Domain-aware twin of `append_runtime_binary_operation`: after the plain
/// op, a WRAPPING `<<` gets the modular count clamp -- a count >= the
/// operand width yields 0 (x * 2^n = 0 mod 2^w), where LSLV alone masks the
/// count mod 64 (1u64 << 70 gave 64; the interpreter is modular at every
/// width since the shift-domain ruling, 2026-07-13). `cmp count, #width;
/// csel dest, xzr, dest, hs` -- 8 bytes, tracked by
/// `runtime_binary_operation_width_with_domain`. Wrapping `>>` and the
/// indexed/pointee binary-write kinds (which carry no domain) are the
/// slice-B remainder in TASKS.md.
fn append_runtime_binary_operation_with_domain(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    operator: StateGuardOperator,
    rhs_register: u8,
    byte_size: usize,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) -> Result<(), Diagnostic> {
    let wrapping = domain == psi_numerics::arithmetic::ArithmeticDomain::Wrapping;
    let non_exact = domain != psi_numerics::arithmetic::ArithmeticDomain::Exact;
    // F8b (ch5 shift-count ruling, settled 2026-07-18): WRAPPING masks the
    // COUNT to the operand width (`k & (width - 1)`). The register-form
    // shifts mask natively at the FORM width (W mod 32, X mod 64) -- exactly
    // the ruling at widths 4/8 -- so only sub-word operands need the explicit
    // AND. Clobbers the rhs register (dead after the operation). This
    // supersedes the 2026-07-13 modular-VALUE fixes (zero clamp / count
    // saturation) for Wrapping; Saturating cannot reach an out-of-range
    // count anymore (the F8a validation obligation), and Trapping keeps the
    // old floor fixes until F8c lands the count trap.
    if wrapping
        && matches!(
            operator,
            StateGuardOperator::ShiftLeft
                | StateGuardOperator::ShiftRight
                | StateGuardOperator::ShiftRightLogical
        )
    {
        if matches!(byte_size, 1 | 2) {
            let ones = if byte_size == 1 { 3 } else { 4 };
            bytes.extend(encode_and_w_low_ones(rhs_register, rhs_register, ones));
        }
        if operator == StateGuardOperator::ShiftLeft {
            // The plain arm's X-form LSLV masks mod 64; the ruling wants the
            // OPERAND width's mask, so narrow widths take the W form (the
            // sub-word AND above already tightened 1/2-byte counts).
            bytes.extend(if byte_size <= 4 {
                encode_lslv_w_register(destination_register, destination_register, rhs_register)
            } else {
                encode_lslv_x_register(destination_register, destination_register, rhs_register)
            });
            return Ok(());
        }
        // `>>`/`>>>` ride the plain arm: it already picks W/X by width (with
        // the sign/zero extension), whose native masking + the sub-word AND
        // is the masked-count semantics.
        return append_runtime_binary_operation(
            bytes,
            destination_register,
            operator,
            rhs_register,
            byte_size,
        );
    }
    let trapping = domain == psi_numerics::arithmetic::ArithmeticDomain::Trapping;
    if trapping
        && matches!(
            operator,
            StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical
        )
    {
        // F8c (ch5 shift-count ruling): a TRAPPING shift with an
        // out-of-range count TRAPS -- regardless of the shifted value (the
        // count is invalid, not the result). Guard BEFORE the op: an
        // in-range count skips the brk and the plain W/X-form shift computes
        // it exactly.
        append_shift_count_trap_guard(bytes, rhs_register, byte_size)?;
        return append_runtime_binary_operation(
            bytes,
            destination_register,
            operator,
            rhs_register,
            byte_size,
        );
    }
    if non_exact && operator == StateGuardOperator::ShiftRight {
        // SATURATING arithmetic `>>` keeps floor(x / 2^n) semantics for an
        // (unreachable post-F8a) at/above-width count: it must SIGN-FILL,
        // and a post-fix cannot recover the sign once the masked shift
        // consumed the value -- so saturate the COUNT first. CSINV turns
        // at/above-width counts into ~0, which ASRV masks to the form
        // width - 1 (31/63): exactly the sign-fill shift. Clobbers the rhs
        // register (dead after the operation, as on x86_64).
        let width_bits = u32::try_from(byte_size * 8).unwrap_or(64);
        bytes.extend(encode_compare_x_immediate(rhs_register, width_bits)?);
        // LO (unsigned <): in-range counts keep rhs; otherwise NOT(XZR).
        bytes.extend(encode_csinv_x(rhs_register, rhs_register, 31, 0b0011));
    }
    append_runtime_binary_operation(
        bytes,
        destination_register,
        operator,
        rhs_register,
        byte_size,
    )?;
    if non_exact && operator == StateGuardOperator::ShiftRightLogical {
        // Saturating logical `>>`: zero at/above-width (floor semantics;
        // unreachable post-F8a, kept for robustness).
        let width_bits = u32::try_from(byte_size * 8).unwrap_or(64);
        bytes.extend(encode_compare_x_immediate(rhs_register, width_bits)?);
        // HS (unsigned >=): count at or above the width selects XZR.
        bytes.extend(encode_csel_x(
            destination_register,
            31,
            destination_register,
            0b0010,
        ));
    }
    Ok(())
}

/// Width of [`float_policy_guard_bytes`]: the emitter run with fixed
/// registers -- register numbers never change instruction lengths on
/// aarch64, so the length IS the width (one source of truth).
pub(in crate::aarch64) fn float_policy_guard_width(
    operator: StateGuardOperator,
    byte_size: usize,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) -> usize {
    float_policy_guard_bytes(
        domain,
        operator,
        byte_size,
        17,
        26,
        matches!(
            operator,
            StateGuardOperator::MultiplyThenAdd | StateGuardOperator::FusedMultiplyAdd
        )
        .then_some(9),
        15,
        14,
    )
    .map(|bytes| bytes.len())
    .unwrap_or(0)
}

/// F5 float ARITHMETIC policy guard, emitted right after the FP op leaves
/// its result in v0 (the raw OPERAND bits stay live in `left`/`right` -- the
/// FMOVs copied them). ALL-INTEGER: sign-clearing a float's bit pattern and
/// comparing against the format's Inf pattern classifies it in ONE integer
/// compare -- LO = finite, EQ = infinite, HI = NaN.
///
/// - `Saturating` (overflow only, per the float brief): an INFINITE landed
///   result from FINITE operands clamps to +-MAX_FINITE carrying the
///   result's sign; a divide whose divisor is +-0.0 keeps its non-finite
///   (division by zero does not clamp), and NaN results pass through
///   (invalid ops stay `Finite` obligations).
/// - `Trapping`: every non-finite result `brk`s, including a NaN or infinity
///   propagated from a non-finite operand.
///
/// Every other operator/domain returns no bytes. Clobbers `left`, `right`, an
/// optional MTA `middle` (dead: the result rides v0), and both scratches. The WIDTH twin calls
/// this with fixed registers and takes `.len()` -- one source of truth (the
/// place-copy rung-2a discipline), no hand-counted lockstep constant.
fn float_policy_guard_bytes(
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    left: u8,
    right: u8,
    middle: Option<u8>,
    s0: u8,
    s1: u8,
) -> Result<Vec<u8>, Diagnostic> {
    use psi_numerics::arithmetic::ArithmeticDomain;
    if !matches!(
        domain,
        ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
    ) || !matches!(
        operator,
        StateGuardOperator::Add
            | StateGuardOperator::AddTowardZero
            | StateGuardOperator::AddTowardPositive
            | StateGuardOperator::AddTowardNegative
            | StateGuardOperator::Subtract
            | StateGuardOperator::Multiply
            | StateGuardOperator::MultiplyThenAdd
            | StateGuardOperator::FusedMultiplyAdd
            | StateGuardOperator::Divide
            | StateGuardOperator::Min
            | StateGuardOperator::Max
            | StateGuardOperator::Sqrt
    ) {
        return Ok(Vec::new());
    }
    let (inf_bits, max_bits): (u64, u64) = if byte_size <= 4 {
        (0x7F80_0000, 0x7F7F_FFFF)
    } else {
        (0x7FF0_0000_0000_0000, 0x7FEF_FFFF_FFFF_FFFF)
    };
    let abs = |register: u8| -> [u8; 4] {
        if byte_size <= 4 {
            encode_and_w_low_ones(register, register, 31)
        } else {
            encode_and_x_low_ones(register, register, 63)
        }
    };
    let sign = |register: u8| -> [u8; 4] {
        if byte_size <= 4 {
            encode_and_w_top_bit(register, register)
        } else {
            encode_and_x_top_bit(register, register)
        }
    };
    let mut bytes = Vec::new();
    // Classify the result: s0 = |result bits|, s1 = Inf pattern.
    bytes.extend(encode_float_move_to_gpr(byte_size, s0, 0)?);
    bytes.extend(abs(s0));
    append_unsigned_immediate_padded(&mut bytes, s1, inf_bits);
    bytes.extend(encode_compare_x_register(s0, s1));
    match domain {
        ArithmeticDomain::Saturating => {
            // The CLAMP tail (fixed content, assembled first so every skip
            // branch knows its distance): MAX_FINITE | sign(result) -> v0.
            let mut clamp = Vec::new();
            clamp.extend(encode_float_move_to_gpr(byte_size, s0, 0)?);
            clamp.extend(sign(s0));
            append_unsigned_immediate_padded(&mut clamp, s1, max_bits);
            clamp.extend(encode_orr_x_register(s1, s1, s0));
            clamp.extend(encode_float_move_from_gpr(byte_size, 0, s1)?);
            // The CHECK chain between the result classify and the clamp;
            // every branch skips to the end (past the clamp).
            let mut checks: Vec<(fn(isize) -> Result<[u8; 4], Diagnostic>, Vec<[u8; 4]>)> =
                Vec::new();
            // result not infinite -> keep (NaN passes through under
            // Saturating: invalid ops stay Finite obligations).
            checks.push((encode_conditional_branch_not_equal, Vec::new()));
            if operator == StateGuardOperator::Divide {
                // divisor +-0.0 -> keep the IEEE non-finite (no clamp).
                checks.push((
                    encode_conditional_branch_equal,
                    vec![abs(right), encode_compare_x_immediate(right, 0)?],
                ));
                // After the zero check the divisor's |bits| are already in
                // `right`: compare against Inf for the finiteness face.
                checks.push((
                    encode_conditional_branch_higher_or_same,
                    vec![encode_compare_x_register(right, s1)],
                ));
            } else {
                checks.push((
                    encode_conditional_branch_higher_or_same,
                    vec![abs(right), encode_compare_x_register(right, s1)],
                ));
            }
            if let Some(middle) = middle {
                checks.push((
                    encode_conditional_branch_higher_or_same,
                    vec![abs(middle), encode_compare_x_register(middle, s1)],
                ));
            }
            checks.push((
                encode_conditional_branch_higher_or_same,
                vec![abs(left), encode_compare_x_register(left, s1)],
            ));
            // Assemble: compute each branch's distance to the end.
            let mut segments: Vec<Vec<u8>> = Vec::new();
            for (index, (_, setup)) in checks.iter().enumerate() {
                let mut segment = Vec::new();
                for instruction in setup {
                    segment.extend(instruction);
                }
                debug_assert!(index == 0 || !setup.is_empty());
                segment.extend([0, 0, 0, 0]); // branch placeholder
                segments.push(segment);
            }
            // Distances: from each placeholder to the end of the clamp.
            let mut tail_after: Vec<usize> = Vec::new();
            let mut running = clamp.len();
            for segment in segments.iter().rev() {
                tail_after.push(running);
                running += segment.len();
            }
            tail_after.reverse();
            for ((branch, _), (segment, after)) in
                checks.iter().zip(segments.iter_mut().zip(tail_after))
            {
                let position = segment.len() - 4;
                // The branch offset counts from the branch instruction
                // itself: 4 (the branch) + the bytes after this segment.
                let encoded = branch((4 + after) as isize)?;
                segment[position..].copy_from_slice(&encoded);
                bytes.extend(segment.iter());
            }
            bytes.extend(clamp);
        }
        ArithmeticDomain::Trapping => {
            // Result-only policy: a finite magnitude skips the trap; infinity
            // and NaN both reach BRK regardless of their operands.
            bytes.extend(encode_conditional_branch_lower(8)?);
            bytes.extend(encode_brk(0));
        }
        _ => unreachable!("gated above"),
    }
    Ok(bytes)
}

/// F4 float->int trap guard: the value in FP register 0 must be NaN-free and
/// within the signed or unsigned target range or the cast traps (ch5 cast
/// ruling; Trapping = trap on NaN/out-of-range, where FCVTZS/FCVTZU saturate).
/// Sequence: `fcmp v0,v0 ; b.vc +8 ; brk` (NaN is unordered) then two bound
/// checks, each `padded-immediate bound bits -> scratch ; fmov v1,scratch ;
/// fcmp v0,v1 ; b.cond +8 ; brk`. The bounds are EXCLUSIVE float constants
/// exact in the source format; i64's and f32's lower bounds are INCLUSIVE
/// (-2^63 / -2^31 are exact and fit; the -1 neighbours are not
/// representable). Clobbers `scratch_gpr` (the conversion overwrites it
/// next) and FP register 1. Fixed width FLOAT_TO_INT_TRAP_GUARD_WIDTH.
fn append_float_to_int_trap_guard(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
    scratch_gpr: u8,
) -> Result<(), Diagnostic> {
    // (upper exclusive, lower bound, lower is inclusive)
    let target_bits = (target_byte_size * 8) as i32;
    let upper = 2.0_f64.powi(target_bits - i32::from(target_signed));
    let (upper_bits, lower_bits, lower_inclusive) = if !target_signed {
        if source_byte_size > 4 {
            (upper.to_bits(), (-1.0_f64).to_bits(), false)
        } else {
            (
                u64::from((upper as f32).to_bits()),
                u64::from((-1.0_f32).to_bits()),
                false,
            )
        }
    } else if source_byte_size > 4 {
        let minimum = -upper;
        let lower_candidate = minimum - 1.0;
        let lower_inclusive = lower_candidate == minimum;
        (
            upper.to_bits(),
            (if lower_inclusive {
                minimum
            } else {
                lower_candidate
            })
            .to_bits(),
            lower_inclusive,
        )
    } else {
        let minimum = (-upper) as f32;
        let lower_candidate = minimum - 1.0;
        let lower_inclusive = lower_candidate == minimum;
        (
            u64::from((upper as f32).to_bits()),
            u64::from(
                (if lower_inclusive {
                    minimum
                } else {
                    lower_candidate
                })
                .to_bits(),
            ),
            lower_inclusive,
        )
    };
    // NaN: fcmp with itself is unordered -> V set.
    bytes.extend(encode_float_compare(source_byte_size, 0, 0)?);
    bytes.extend(encode_conditional_branch_no_overflow(8)?);
    bytes.extend(encode_brk(0));
    // Upper: keep when f < upper (ordered less), else trap.
    append_unsigned_immediate_padded(bytes, scratch_gpr, upper_bits);
    bytes.extend(encode_float_move_from_gpr(
        source_byte_size,
        1,
        scratch_gpr,
    )?);
    bytes.extend(encode_float_compare(source_byte_size, 0, 1)?);
    bytes.extend(encode_conditional_branch_less(8)?);
    bytes.extend(encode_brk(0));
    // Lower: keep when f > lower (or >= for the inclusive bounds), else trap.
    append_unsigned_immediate_padded(bytes, scratch_gpr, lower_bits);
    bytes.extend(encode_float_move_from_gpr(
        source_byte_size,
        1,
        scratch_gpr,
    )?);
    bytes.extend(encode_float_compare(source_byte_size, 0, 1)?);
    bytes.extend(if lower_inclusive {
        encode_conditional_branch_greater_or_equal(8)?
    } else {
        encode_conditional_branch_greater(8)?
    });
    bytes.extend(encode_brk(0));
    Ok(())
}

pub(in crate::aarch64) fn float_to_narrow_int_saturating_width(
    target_byte_size: usize,
    target_signed: bool,
) -> usize {
    if target_byte_size >= 4 {
        0
    } else if target_signed {
        52
    } else {
        24
    }
}

fn append_float_to_narrow_int_saturating(
    bytes: &mut Vec<u8>,
    register: u8,
    target_byte_size: usize,
    target_signed: bool,
) {
    let bits = target_byte_size * 8;
    let scratch = 15;
    if target_signed {
        bytes.extend(encode_sign_extend_word_to_x(register, register));
        let sign_bit = 1_u64 << (bits - 1);
        append_unsigned_immediate_padded(bytes, scratch, sign_bit - 1);
        bytes.extend(encode_compare_x_register(register, scratch));
        bytes.extend(encode_csel_x(register, register, scratch, 0b1101)); // LE
        append_unsigned_immediate_padded(bytes, scratch, 0_u64.wrapping_sub(sign_bit));
        bytes.extend(encode_compare_x_register(register, scratch));
        bytes.extend(encode_csel_x(register, register, scratch, 0b1010)); // GE
    } else {
        let maximum = (1_u64 << bits) - 1;
        append_unsigned_immediate_padded(bytes, scratch, maximum);
        bytes.extend(encode_compare_x_register(register, scratch));
        bytes.extend(encode_csel_x(register, register, scratch, 0b1001)); // LS
    }
    debug_assert_eq!(
        bytes.len() % 4,
        0,
        "aarch64 conversion fixup stays instruction-aligned"
    );
}

/// Bytes of [`append_float_to_int_trap_guard`]: NaN check (fcmp + b.vc + brk
/// = 12) + two bound checks (padded immediate 16 + fmov 4 + fcmp 4 + b.cond
/// 4 + brk 4 = 32 each) = 76. MUST stay in lockstep.
pub(in crate::aarch64) const FLOAT_TO_INT_TRAP_GUARD_WIDTH: usize = 76;

/// F8c count guard: `cmp count, #width ; b.lo +8 ; brk #0` -- a TRAPPING
/// shift's out-of-range count traps before the shift runs. 12 bytes; the
/// width fns add SHIFT_COUNT_TRAP_GUARD_WIDTH in lockstep.
fn append_shift_count_trap_guard(
    bytes: &mut Vec<u8>,
    count_register: u8,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let width_bits = u32::try_from(byte_size * 8).unwrap_or(64);
    bytes.extend(encode_compare_x_immediate(count_register, width_bits)?);
    // LO (unsigned <): an in-range count hops over the brk.
    bytes.extend(encode_conditional_branch_lower(8)?);
    bytes.extend(encode_brk(0));
    Ok(())
}

/// Bytes of [`append_shift_count_trap_guard`]: cmp (4) + b.lo (4) + brk (4).
pub(in crate::aarch64) const SHIFT_COUNT_TRAP_GUARD_WIDTH: usize = 12;

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
        // Logical `&&`/`||` over 0/1 booleans AND the bitwise `&`/`|`/`^`
        // operators all lower to the register-form AND/ORR/EOR (a single
        // instruction; the store truncates to the target width for narrow
        // operands, and bitwise ops are width-independent on x-registers).
        StateGuardOperator::And | StateGuardOperator::BitwiseAnd => {
            bytes.extend(encode_and_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Or | StateGuardOperator::BitwiseOr => {
            bytes.extend(encode_orr_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::BitwiseXor => {
            bytes.extend(encode_eor_x_register(
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
            // A narrow signed value may arrive ZERO-extended (guard-subject
            // loads); ASR fills from bit 31/63, so extend the VALUE register to
            // the operation width first (idempotent when already extended).
            if byte_size == 1 {
                bytes.extend(encode_sign_extend_byte_to_w(
                    destination_register,
                    destination_register,
                ));
            } else if byte_size == 2 {
                bytes.extend(encode_sign_extend_halfword_to_w(
                    destination_register,
                    destination_register,
                ));
            }
            bytes.extend(if narrow {
                encode_asrv_w_register(destination_register, destination_register, right_register)
            } else {
                encode_asrv_x_register(destination_register, destination_register, right_register)
            });
        }
        // Logical (zero-filling) right shift for an unsigned `>>`. The zero
        // fill must start at the OPERAND width: a narrow value may sit in a
        // register with garbage/wrapped HIGH bits (a 64-bit nested Wrapping op
        // hands its parent the untruncated result), and the X form would shift
        // those down into the live word. Sub-word values are zero-extended
        // first (the logical twin of the ShiftRight arm's sign-extension);
        // width 4 rides the W form directly (it reads only the low 32 bits).
        StateGuardOperator::ShiftRightLogical => {
            if byte_size == 1 {
                bytes.extend(encode_zero_extend_byte_to_w(
                    destination_register,
                    destination_register,
                ));
            } else if byte_size == 2 {
                bytes.extend(encode_zero_extend_halfword_to_w(
                    destination_register,
                    destination_register,
                ));
            }
            bytes.extend(if narrow {
                encode_lsrv_w_register(destination_register, destination_register, right_register)
            } else {
                encode_lsrv_x_register(destination_register, destination_register, right_register)
            });
        }
        StateGuardOperator::Divide | StateGuardOperator::DivideUnsigned => {
            let signed = matches!(operator, StateGuardOperator::Divide);
            append_narrow_signed_division_operand_extension(
                bytes,
                signed,
                byte_size,
                destination_register,
                right_register,
            );
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
            append_narrow_signed_division_operand_extension(
                bytes,
                signed,
                byte_size,
                destination_register,
                right_register,
            );
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
                StateGuardOperator::MaxUnsigned => encode_conditional_branch_higher_or_same(8)?,
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
                StateGuardOperator::GreaterUnsigned => encode_conditional_branch_lower_or_same(8)?,
                StateGuardOperator::GreaterOrEqualUnsigned => encode_conditional_branch_lower(8)?,
                StateGuardOperator::LessUnsigned => encode_conditional_branch_higher_or_same(8)?,
                StateGuardOperator::LessOrEqualUnsigned => encode_conditional_branch_higher(8)?,
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
            | StateGuardOperator::IsNan
            | StateGuardOperator::IsFinite
            | StateGuardOperator::IsInfinite
            | StateGuardOperator::IsNormal
            | StateGuardOperator::IsSubnormal
            | StateGuardOperator::FloatClassify
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
pub(in crate::aarch64) fn runtime_value_operand_value_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> Option<usize> {
    if let Some((_, _, byte_size)) = operands.storage(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, value_byte_size, _)) = operands.bit_field(operand) {
        return Some(value_byte_size);
    }
    if let Some((_, _, byte_size)) = operands.pointee(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, _, _, byte_size)) = operands.frame_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, _, byte_size)) = operands.frame_base_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_fixed_indexed(operand) {
        return Some(byte_size);
    }
    if let Some(width) = operands.binary_byte_width(operand) {
        return Some(width);
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
pub(in crate::aarch64) fn runtime_binary_operation_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operator: StateGuardOperator,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    target_byte_size: usize,
) -> usize {
    if matches!(
        operator,
        StateGuardOperator::IsNan
            | StateGuardOperator::IsFinite
            | StateGuardOperator::IsInfinite
            | StateGuardOperator::IsNormal
            | StateGuardOperator::IsSubnormal
            | StateGuardOperator::FloatClassify
    ) && let Some(width @ (4 | 8)) = operands.immediate_integer(right)
    {
        return width as usize;
    }
    if is_comparison_operator(operator) {
        runtime_binary_compare_byte_size(operands, left, right)
    } else if matches!(
        operator,
        StateGuardOperator::Divide
            | StateGuardOperator::Modulo
            | StateGuardOperator::DivideUnsigned
            | StateGuardOperator::ModuloUnsigned
            | StateGuardOperator::ShiftLeft
            | StateGuardOperator::ShiftRight
            | StateGuardOperator::ShiftRightLogical
    ) {
        // Non-modular ops must run at the OPERAND width, not a hardcoded 64-bit:
        // a 64-bit sdiv/asr on a narrow i32 (loaded without a sign-extended top
        // half) reads the sign/high bit wrong. Sizing to the shifted/divided VALUE
        // (left, else the other operand) picks the narrow (W-register) form so an
        // i32 sign bit is honored -- mirrors the x86_64 backend. Both W/X encodings
        // are the same fixed length here, so relocation offsets are unaffected. Two
        // immediates carry no width, so fall back to the declared target width.
        runtime_value_operand_value_byte_size(operands, left)
            .or_else(|| runtime_value_operand_value_byte_size(operands, right))
            .unwrap_or(target_byte_size)
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
/// Truncate a nested WRAPPING binary's 64-bit register result to the node's
/// declared width, extending per the node's signedness, so the parent
/// operation consumes the wrapped VALUE (interp wraps at the node). One
/// 4-byte instruction for widths 1/2/4; 8-byte nodes are already exact.
fn append_wrapping_operand_truncation(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_width: usize,
    operands_signed: bool,
) {
    match (byte_width, operands_signed) {
        (1, false) => bytes.extend(encode_zero_extend_byte_to_w(register, register)),
        (2, false) => bytes.extend(encode_zero_extend_halfword_to_w(register, register)),
        (4, false) => bytes.extend(encode_move_w_register(register, register)),
        (1, true) => bytes.extend(encode_sign_extend_byte_to_x(register, register)),
        (2, true) => bytes.extend(encode_sign_extend_halfword_to_x(register, register)),
        (4, true) => bytes.extend(encode_sign_extend_word_to_x(register, register)),
        _ => {}
    }
}

fn is_integer_float_classification_predicate(operator: StateGuardOperator) -> bool {
    matches!(
        operator,
        StateGuardOperator::IsFinite
            | StateGuardOperator::IsInfinite
            | StateGuardOperator::IsNormal
            | StateGuardOperator::IsSubnormal
    )
}

fn append_float_classification_threshold(
    bytes: &mut Vec<u8>,
    end_branches: &mut Vec<(usize, u8)>,
    value_register: u8,
    threshold_register: u8,
    threshold_bits: u64,
    condition: u8,
) {
    append_unsigned_immediate_padded(bytes, threshold_register, threshold_bits);
    bytes.extend(encode_compare_x_register(
        value_register,
        threshold_register,
    ));
    let branch = bytes.len();
    bytes.extend([0; 4]);
    end_branches.push((branch, condition));
}

/// Classify the raw IEEE bits without touching FP control state. Entry and
/// result share `destination_register`; both scratches are dead on exit.
fn float_classification_predicate_bytes(
    operator: StateGuardOperator,
    byte_size: usize,
    destination_register: u8,
    scratches: [u8; 2],
) -> Result<Vec<u8>, Diagnostic> {
    let (infinity, minimum_normal) = if byte_size > 4 {
        (0x7ff0_0000_0000_0000_u64, 0x0010_0000_0000_0000_u64)
    } else {
        (0x7f80_0000_u64, 0x0080_0000_u64)
    };
    let value = scratches[0];
    let threshold = scratches[1];
    let mut bytes = Vec::new();
    if byte_size > 4 {
        bytes.extend(encode_move_x_register(value, destination_register));
        bytes.extend(encode_and_x_low_ones(value, value, 63));
    } else {
        bytes.extend(encode_move_w_register(value, destination_register));
        bytes.extend(encode_and_w_low_ones(value, value, 31));
    }
    bytes.extend(encode_movz_w(destination_register, 0));

    // Record branch sites and patch them once the common end is known.
    // Conditions use AArch64's unsigned integer flags over |bits|.
    let mut end_branches: Vec<(usize, u8)> = Vec::new();
    match operator {
        StateGuardOperator::IsFinite => append_float_classification_threshold(
            &mut bytes,
            &mut end_branches,
            value,
            threshold,
            infinity,
            0,
        ),
        StateGuardOperator::IsInfinite => append_float_classification_threshold(
            &mut bytes,
            &mut end_branches,
            value,
            threshold,
            infinity,
            1,
        ),
        StateGuardOperator::IsNormal => {
            append_float_classification_threshold(
                &mut bytes,
                &mut end_branches,
                value,
                threshold,
                minimum_normal,
                2,
            );
            append_float_classification_threshold(
                &mut bytes,
                &mut end_branches,
                value,
                threshold,
                infinity,
                0,
            );
        }
        StateGuardOperator::IsSubnormal => {
            bytes.extend(encode_compare_x_immediate(value, 0)?);
            let branch = bytes.len();
            bytes.extend([0; 4]);
            end_branches.push((branch, 3));
            append_float_classification_threshold(
                &mut bytes,
                &mut end_branches,
                value,
                threshold,
                minimum_normal,
                0,
            );
        }
        _ => unreachable!("classification helper is predicate-only"),
    }
    bytes.extend(encode_movz_w(destination_register, 1));
    let end = bytes.len();
    for (branch, condition) in end_branches {
        let distance = end as isize - branch as isize;
        let instruction = match condition {
            0 => encode_conditional_branch_higher_or_same(distance)?,
            1 => encode_conditional_branch_not_equal(distance)?,
            2 => encode_conditional_branch_lower(distance)?,
            3 => encode_conditional_branch_equal(distance)?,
            _ => unreachable!(),
        };
        bytes[branch..branch + 4].copy_from_slice(&instruction);
    }
    Ok(bytes)
}

pub(in crate::aarch64) fn float_classification_predicate_width(
    operator: StateGuardOperator,
    byte_size: usize,
) -> usize {
    float_classification_predicate_bytes(operator, byte_size, 17, [15, 14])
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

fn append_packed_float_class(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    tag: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_add_x_immediate(
        destination_register,
        destination_register,
        tag,
    )?);
    Ok(())
}

/// Return the stable `FloatClass` enum carrier: i32 tag at byte 0 and the
/// overlaid `negative: bool` payload at byte 4. The source declaration fixes
/// tags as NaN=0, Infinity=1, Normal=2, Subnormal=3, Zero=4.
fn float_classify_bytes(
    byte_size: usize,
    destination_register: u8,
    scratches: [u8; 2],
) -> Result<Vec<u8>, Diagnostic> {
    let (infinity, minimum_normal, sign_shift) = if byte_size > 4 {
        (0x7ff0_0000_0000_0000_u64, 0x0010_0000_0000_0000_u64, 63)
    } else {
        (0x7f80_0000_u64, 0x0080_0000_u64, 31)
    };
    let value = scratches[0];
    let threshold = scratches[1];
    let mut bytes = Vec::new();
    if byte_size > 4 {
        bytes.extend(encode_move_x_register(value, destination_register));
        bytes.extend(encode_and_x_low_ones(value, value, 63));
    } else {
        bytes.extend(encode_move_w_register(value, destination_register));
        bytes.extend(encode_and_w_low_ones(value, value, 31));
    }
    bytes.extend(encode_lsr_x_immediate(
        destination_register,
        destination_register,
        sign_shift,
    ));
    bytes.extend(encode_lsl_x_immediate(
        destination_register,
        destination_register,
        32,
    ));

    append_unsigned_immediate_padded(&mut bytes, threshold, infinity);
    bytes.extend(encode_compare_x_register(value, threshold));
    let nan_branch = bytes.len();
    bytes.extend([0; 4]);
    let infinity_branch = bytes.len();
    bytes.extend([0; 4]);
    bytes.extend(encode_compare_x_immediate(value, 0)?);
    let zero_branch = bytes.len();
    bytes.extend([0; 4]);
    append_unsigned_immediate_padded(&mut bytes, threshold, minimum_normal);
    bytes.extend(encode_compare_x_register(value, threshold));
    let subnormal_branch = bytes.len();
    bytes.extend([0; 4]);

    append_packed_float_class(&mut bytes, destination_register, 2)?;
    let normal_end = bytes.len();
    bytes.extend([0; 4]);
    let subnormal = bytes.len();
    append_packed_float_class(&mut bytes, destination_register, 3)?;
    let subnormal_end = bytes.len();
    bytes.extend([0; 4]);
    let zero = bytes.len();
    append_packed_float_class(&mut bytes, destination_register, 4)?;
    let zero_end = bytes.len();
    bytes.extend([0; 4]);
    let infinity_label = bytes.len();
    append_packed_float_class(&mut bytes, destination_register, 1)?;
    let infinity_end = bytes.len();
    bytes.extend([0; 4]);
    let nan = bytes.len();
    bytes.extend(encode_movz_w(destination_register, 0));
    let end = bytes.len();

    for (branch, instruction) in [
        (
            nan_branch,
            encode_conditional_branch_higher((nan - nan_branch) as isize)?,
        ),
        (
            infinity_branch,
            encode_conditional_branch_equal((infinity_label - infinity_branch) as isize)?,
        ),
        (
            zero_branch,
            encode_conditional_branch_equal((zero - zero_branch) as isize)?,
        ),
        (
            subnormal_branch,
            encode_conditional_branch_lower((subnormal - subnormal_branch) as isize)?,
        ),
    ] {
        bytes[branch..branch + 4].copy_from_slice(&instruction);
    }
    for branch in [normal_end, subnormal_end, zero_end, infinity_end] {
        let instruction = encode_unconditional_branch(end as isize - branch as isize)?;
        bytes[branch..branch + 4].copy_from_slice(&instruction);
    }
    Ok(bytes)
}

pub(in crate::aarch64) fn float_classify_width(byte_size: usize) -> usize {
    float_classify_bytes(byte_size, 17, [15, 14])
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

fn append_runtime_float_binary_operation(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    left_register: u8,
    operator: StateGuardOperator,
    right_register: u8,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    guard_scratches: [u8; 2],
) -> Result<(), Diagnostic> {
    if operator == StateGuardOperator::FloatPair {
        // The pair is internal to MTA lowering. Keep the second operand in the
        // pinned x9 scratch and return the third through the pair destination.
        bytes.extend(encode_move_x_register(9, left_register));
        bytes.extend(encode_move_x_register(left_register, right_register));
        return Ok(());
    }
    if operator == StateGuardOperator::MultiplyThenAdd {
        // x9 was populated by the structural FloatPair. Two explicit FP ops
        // preserve round(round(a*b)+c); this is intentionally not FMADD.
        bytes.extend(encode_float_move_from_gpr(byte_size, 0, left_register)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 1, 9)?);
        bytes.extend(encode_float_multiply(byte_size, 0, 0, 1)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 1, right_register)?);
        bytes.extend(encode_float_add(byte_size, 0, 0, 1)?);
        bytes.extend(float_policy_guard_bytes(
            domain,
            operator,
            byte_size,
            left_register,
            right_register,
            Some(9),
            guard_scratches[0],
            guard_scratches[1],
        )?);
        bytes.extend(encode_float_move_to_gpr(byte_size, left_register, 0)?);
        return Ok(());
    }
    if operator == StateGuardOperator::FusedMultiplyAdd {
        // x9 was populated by the structural FloatPair. FMADD performs the
        // multiply and add with one final rounding; do not split this into
        // FMUL/FADD or reuse the distinct multiply-then-add arm.
        bytes.extend(encode_float_move_from_gpr(byte_size, 0, left_register)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 1, 9)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 2, right_register)?);
        bytes.extend(encode_float_fused_multiply_add(byte_size, 0, 0, 1, 2)?);
        bytes.extend(float_policy_guard_bytes(
            domain,
            operator,
            byte_size,
            left_register,
            right_register,
            Some(9),
            guard_scratches[0],
            guard_scratches[1],
        )?);
        bytes.extend(encode_float_move_to_gpr(byte_size, left_register, 0)?);
        return Ok(());
    }
    if is_integer_float_classification_predicate(operator) {
        bytes.extend(float_classification_predicate_bytes(
            operator,
            byte_size,
            left_register,
            guard_scratches,
        )?);
        return Ok(());
    }
    if operator == StateGuardOperator::FloatClassify {
        bytes.extend(float_classify_bytes(
            byte_size,
            left_register,
            guard_scratches,
        )?);
        return Ok(());
    }
    bytes.extend(encode_float_move_from_gpr(byte_size, 0, left_register)?);
    bytes.extend(encode_float_move_from_gpr(byte_size, 1, right_register)?);
    // F5: the arithmetic ops append the policy guard AFTER the op (the raw
    // operand bits stay live in the GPRs -- the FMOVs copy, never move).
    let guard = |bytes: &mut Vec<u8>| -> Result<(), Diagnostic> {
        bytes.extend(float_policy_guard_bytes(
            domain,
            operator,
            byte_size,
            left_register,
            right_register,
            None,
            guard_scratches[0],
            guard_scratches[1],
        )?);
        Ok(())
    };
    let directed_rounding = match operator {
        // FPCR.RMode bits 22..23: +inf=01, -inf=10, zero=11.
        StateGuardOperator::AddTowardPositive => Some(0x0040_0000),
        StateGuardOperator::AddTowardNegative => Some(0x0080_0000),
        StateGuardOperator::AddTowardZero => Some(0x00c0_0000),
        _ => None,
    };
    if let Some(fpcr) = directed_rounding {
        // x13 retains the exact prior FPCR while x12 installs the requested
        // direction. x16 remains the live destination-address register.
        // Policy adaptation runs only after the prior state is back.
        bytes.extend(encode_read_fpcr(13));
        append_unsigned_immediate(bytes, 12, fpcr);
        bytes.extend(encode_write_fpcr(12));
    }
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::AddTowardZero
        | StateGuardOperator::AddTowardPositive
        | StateGuardOperator::AddTowardNegative => {
            bytes.extend(encode_float_add(byte_size, 0, 0, 1)?);
            if directed_rounding.is_some() {
                bytes.extend(encode_write_fpcr(13));
            }
            guard(bytes)?;
        }
        StateGuardOperator::Subtract => {
            bytes.extend(encode_float_subtract(byte_size, 0, 0, 1)?);
            guard(bytes)?;
        }
        StateGuardOperator::Multiply => {
            bytes.extend(encode_float_multiply(byte_size, 0, 0, 1)?);
            guard(bytes)?;
        }
        StateGuardOperator::Divide => {
            bytes.extend(encode_float_divide(byte_size, 0, 0, 1)?);
            guard(bytes)?;
        }
        // FMAX/FMIN(NM) do NOT match the pinned SSE semantics (`a > b ? a : b`;
        // NaN or equal returns b -- see the interpreter's eval_min_max), so
        // min/max lower as FCMP + FCSEL with GT/MI, both false on unordered.
        // Two instructions: runtime_float_binary_operation_width(operator)
        // tracks this in lockstep.
        StateGuardOperator::Max => {
            bytes.extend(encode_float_compare(byte_size, 0, 1)?);
            bytes.extend(encode_float_conditional_select(byte_size, 0, 0, 1, 0b1100)?);
            guard(bytes)?;
        }
        StateGuardOperator::Min => {
            bytes.extend(encode_float_compare(byte_size, 0, 1)?);
            bytes.extend(encode_float_conditional_select(byte_size, 0, 0, 1, 0b0100)?);
            guard(bytes)?;
        }
        // Unary, carried with both operands = x (the x86_64 table's shape):
        // sqrt(operand1) into slot 0.
        StateGuardOperator::Sqrt => {
            bytes.extend(encode_float_sqrt(byte_size, 0, 1)?);
            guard(bytes)?;
        }
        StateGuardOperator::IsNan => {
            bytes.extend(encode_float_compare(byte_size, 0, 0)?);
            bytes.extend(encode_movz_w(left_register, 0));
            bytes.extend(encode_conditional_branch_no_overflow(8)?);
            bytes.extend(encode_movz_w(left_register, 1));
            return Ok(());
        }
        // COMPARISON into a 0/1 GPR result (`let ok: bool = self.a > self.b`
        // with float operands): FCMP at the OPERAND width, then the integer
        // write path's materialization pattern (MOVZ 0 / negated skip /
        // MOVZ 1) using the guard path's float-aware conditions -- ordered
        // comparisons are FALSE on unordered inputs, matching x86 `ucomis*`
        // and the interpreter. The result is already integer bits in the
        // GPR, so the trailing FMOV-back is skipped (early return).
        // Unsigned spellings normalize to the signed conditions first: float
        // NZCV conditions carry no signedness. Width tracked by
        // runtime_float_binary_operation_width -- MUST stay in lockstep.
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
            let ordered_operator = match operator {
                StateGuardOperator::GreaterUnsigned => StateGuardOperator::Greater,
                StateGuardOperator::GreaterOrEqualUnsigned => StateGuardOperator::GreaterOrEqual,
                StateGuardOperator::LessUnsigned => StateGuardOperator::Less,
                StateGuardOperator::LessOrEqualUnsigned => StateGuardOperator::LessOrEqual,
                other => other,
            };
            bytes.extend(encode_float_compare(byte_size, 0, 1)?);
            bytes.extend(encode_movz_w(left_register, 0));
            bytes.extend(encode_conditional_branch_for_operator_bytes(
                ordered_operator,
                8,
                true,
            )?);
            bytes.extend(encode_movz_w(left_register, 1));
            return Ok(());
        }
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
        if scratch_register == base_register || byte_offset <= 4095 {
            append_add_constant_to_x_register(bytes, scratch_register, byte_offset)?;
        } else {
            // Preserve the historical leading-move width, but keep the actual
            // address formation inside the caller-supplied scratch contract.
            // The base is still intact, so the scratch may hold the constant
            // directly; no hidden x19/x26 register enters boundary marshalling.
            append_unsigned_immediate(bytes, scratch_register, byte_offset as u64);
            bytes.extend(encode_add_x_register(
                scratch_register,
                base_register,
                scratch_register,
            ));
        }
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

/// Compute `base + byte_offset` in the same fixed 24-byte envelope as
/// `append_fixed_width_load_x_from_x_offset`. Carrier text equality needs an
/// inline byte address where descriptor equality performs a pointer load; the
/// padded self-move keeps relocation offsets and operand widths identical.
fn append_fixed_width_address_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        destination_register,
        base_register,
        scratch_register,
    ));
    bytes.extend(encode_move_x_register(
        destination_register,
        destination_register,
    ));
}

/// Loads an unsigned array index at its declared width. Narrow loads target a
/// W register and therefore zero-extend into the full X register; an 8-byte
/// index uses the corresponding X load.
///
/// Emits the SAME 24-byte sequence as `append_fixed_width_load_x_from_x_offset`
/// (padded 4-instruction immediate = 16 bytes, ADD = 4, load = 4) — only the
/// final load differs (`LDR Wt` vs `LDR Xt`, both 4 bytes) — so width functions
/// are unchanged.
fn append_fixed_width_load_unsigned_index_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        scratch_register,
        base_register,
        scratch_register,
    ));
    match byte_size {
        1 | 2 | 4 => bytes.extend(
            encode_load_w_from_x(destination_register, scratch_register, 0, byte_size)
                .expect("zero-offset w-register load should always encode"),
        ),
        8 => bytes.extend(
            encode_load_x_from_x(destination_register, scratch_register, 0)
                .expect("zero-offset x-register load should always encode"),
        ),
        _ => unreachable!("validated index width"),
    }
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
    use super::super::primitives::encode_movk;
    use super::super::widths;
    use super::*;

    #[test]
    fn bounded_buffer_literal_write_rebases_large_machine_offsets() {
        let bytes = encode_runtime_machine_bounded_buffer_write(5072, "torch")
            .expect("large carrier offset encodes");
        assert_eq!(
            bytes.len(),
            widths::runtime_machine_bounded_buffer_write_width(5072, "torch")
        );
    }

    #[test]
    fn string_descriptor_write_materializes_large_machine_offsets() {
        encode_runtime_machine_string_write(37_024, 12)
            .expect("large String descriptor offset encodes");
    }

    #[test]
    fn float_classification_sequences_stay_in_width_lockstep() {
        for byte_size in [4usize, 8] {
            for operator in [
                StateGuardOperator::IsFinite,
                StateGuardOperator::IsInfinite,
                StateGuardOperator::IsNormal,
                StateGuardOperator::IsSubnormal,
            ] {
                let mut bytes = Vec::new();
                append_runtime_float_binary_operation(
                    &mut bytes,
                    byte_size,
                    17,
                    operator,
                    26,
                    psi_numerics::arithmetic::ArithmeticDomain::Exact,
                    [15, 14],
                )
                .expect("encode float classification");
                assert_eq!(
                    bytes.len(),
                    float_classification_predicate_width(operator, byte_size),
                    "f{} {operator:?} width",
                    byte_size * 8,
                );
            }
            let mut bytes = Vec::new();
            append_runtime_float_binary_operation(
                &mut bytes,
                byte_size,
                17,
                StateGuardOperator::FloatClassify,
                26,
                psi_numerics::arithmetic::ArithmeticDomain::Exact,
                [15, 14],
            )
            .expect("encode enum float classification");
            assert_eq!(
                bytes.len(),
                float_classify_width(byte_size),
                "f{} FloatClassify width",
                byte_size * 8,
            );
        }
    }

    /// `LDADDAL <Ws/Xs>, <Wt/Xt>, [<Xn>]` per width: the size field selects the
    /// access size, the acquire+release bits are set, and Rt receives the prior.
    #[test]
    fn ldadd_encodes_per_width_and_ordering() {
        // (byte_size, expected size field in bits 31:30)
        for &(byte_size, size) in &[(1usize, 0u32), (2, 1), (4, 2), (8, 3)] {
            let bytes = encode_ldadd(
                byte_size,
                17,
                26,
                16,
                psi_language_core::MemoryOrdering::ReceivePublish,
            )
            .expect("encode");
            assert_eq!(bytes.len(), 4, "atomic add is a single instruction");
            let word = u32::from_le_bytes(bytes[..].try_into().unwrap());
            let expected = 0x38E0_0000 | (size << 30) | (17u32 << 16) | (16u32 << 5) | 26;
            assert_eq!(word, expected, "byte_size={byte_size}");
            assert_eq!(word >> 30, size, "size field");
            assert_eq!((word >> 22) & 0b11, 0b11, "acquire+release ordering bits");
            assert_eq!((word >> 16) & 0x1F, 17, "Rs = add register");
            assert_eq!((word >> 5) & 0x1F, 16, "Rn = address register");
            assert_eq!(word & 0x1F, 26, "Rt = prior-value result register");
        }
        assert!(
            encode_ldadd(3, 17, 26, 16, psi_language_core::MemoryOrdering::NoOrdering,).is_err(),
            "non-power-of-two width must error, not miscompile"
        );
        let words = [
            psi_language_core::MemoryOrdering::NoOrdering,
            psi_language_core::MemoryOrdering::Receive,
            psi_language_core::MemoryOrdering::Publish,
            psi_language_core::MemoryOrdering::ReceivePublish,
            psi_language_core::MemoryOrdering::GlobalOrder,
        ]
        .map(|ordering| u32::from_le_bytes(encode_ldadd(4, 17, 26, 16, ordering).unwrap()));
        assert_eq!(
            words,
            [
                0xB831_021A,
                0xB8B1_021A,
                0xB871_021A,
                0xB8F1_021A,
                0xB8F1_021A,
            ]
        );
    }

    #[test]
    fn swp_encodes_per_width_and_ordering() {
        let words = [
            psi_language_core::MemoryOrdering::NoOrdering,
            psi_language_core::MemoryOrdering::Receive,
            psi_language_core::MemoryOrdering::Publish,
            psi_language_core::MemoryOrdering::ReceivePublish,
            psi_language_core::MemoryOrdering::GlobalOrder,
        ]
        .map(|ordering| u32::from_le_bytes(encode_swp(4, 17, 26, 16, ordering).unwrap()));
        assert_eq!(
            words,
            [
                0xB831_821A,
                0xB8B1_821A,
                0xB871_821A,
                0xB8F1_821A,
                0xB8F1_821A,
            ]
        );
        for byte_size in [1usize, 2, 4, 8] {
            assert!(
                encode_swp(
                    byte_size,
                    17,
                    26,
                    16,
                    psi_language_core::MemoryOrdering::ReceivePublish,
                )
                .is_ok()
            );
        }
        assert!(encode_swp(3, 17, 26, 16, psi_language_core::MemoryOrdering::NoOrdering).is_err());
    }

    #[test]
    fn atomic_load_store_select_no_ordering_and_ordered_encodings() {
        use psi_language_core::MemoryOrdering as O;

        assert_eq!(
            u32::from_le_bytes(encode_atomic_load(17, 16, 4, O::NoOrdering).unwrap()),
            0xB940_0211
        );
        assert_eq!(
            u32::from_le_bytes(encode_atomic_load(17, 16, 4, O::Receive).unwrap()),
            0x88DF_FE11
        );
        assert_eq!(
            u32::from_le_bytes(encode_atomic_store(17, 16, 4, O::NoOrdering).unwrap()),
            0xB900_0211
        );
        assert_eq!(
            u32::from_le_bytes(encode_atomic_store(17, 16, 4, O::Publish).unwrap()),
            0x889F_FE11
        );
        assert!(encode_atomic_load(17, 16, 4, O::Publish).is_err());
        assert!(encode_atomic_store(17, 16, 4, O::Receive).is_err());
    }

    /// The full `encode_atomic_fetch_add` path: the emitted length must equal
    /// its width function at every offset, and its RMW must be
    /// `LDADDAL w17, w26, [x16]`. The delta is an immediate so the operand load
    /// is offset-independent.
    #[test]
    fn atomic_fetch_add_encoder_matches_width_and_ends_in_ldaddal() {
        use omega_target_operations::RuntimeValueOperand;
        use psi_arena::Arena;

        for &target_offset in &[0usize, 8, 4095] {
            let mut operands: Arena<RuntimeValueOperand> = Arena::default();
            let delta = operands.insert(RuntimeValueOperand::Immediate(5));
            let result_offset = 24;
            let bytes = encode_atomic_fetch_add(
                &operands,
                target_offset,
                4,
                result_offset,
                delta,
                psi_language_core::MemoryOrdering::ReceivePublish,
            )
            .expect("encode");
            assert_eq!(
                bytes.len(),
                runtime_atomic_fetch_add_width(&operands, target_offset, 4, result_offset, delta,),
                "width mismatch at offset {target_offset}"
            );
            let atomic_end =
                runtime_atomic_fetch_add_result_address_offset(&operands, target_offset, delta);
            let last = u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap());
            assert_eq!(
                last, 0xB8F1_021A,
                "atomic instruction must be LDADDAL w17, w26, [x16] at offset {target_offset}"
            );
        }

        // An offset past the single ADD-immediate reach errors, not miscompiles.
        let mut operands: Arena<RuntimeValueOperand> = Arena::default();
        let delta = operands.insert(RuntimeValueOperand::Immediate(1));
        assert!(
            encode_atomic_fetch_add(
                &operands,
                4096,
                4,
                0,
                delta,
                psi_language_core::MemoryOrdering::NoOrdering,
            )
            .is_err()
        );
    }

    #[test]
    fn atomic_fetch_sub_negates_at_width_then_uses_ldaddal() {
        use omega_target_operations::RuntimeValueOperand;
        use psi_arena::Arena;

        let mut operands: Arena<RuntimeValueOperand> = Arena::default();
        let delta = operands.insert(RuntimeValueOperand::Immediate(12));
        let bytes = encode_atomic_fetch_sub(
            &operands,
            0,
            4,
            24,
            delta,
            psi_language_core::MemoryOrdering::ReceivePublish,
        )
        .expect("encode");
        assert_eq!(
            bytes.len(),
            runtime_atomic_fetch_sub_width(&operands, 0, 4, 24, delta)
        );
        let atomic_end = runtime_atomic_fetch_sub_result_address_offset(&operands, 0, delta);
        assert_eq!(
            u32::from_le_bytes(bytes[atomic_end - 8..atomic_end - 4].try_into().unwrap()),
            0x4B11_03F1,
            "fetch_sub must emit SUB w17,wzr,w17"
        );
        assert_eq!(
            u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap()),
            0xB8F1_021A,
            "fetch_sub must emit LDADDAL w17,w26,[x16]"
        );
    }

    #[test]
    fn atomic_fetch_xor_uses_ordered_ldeor_and_returns_prior() {
        use omega_target_operations::RuntimeValueOperand;
        use psi_arena::Arena;

        let mut operands: Arena<RuntimeValueOperand> = Arena::default();
        let value = operands.insert(RuntimeValueOperand::Immediate(12));
        let bytes = encode_atomic_fetch_xor(
            &operands,
            0,
            4,
            24,
            value,
            psi_language_core::MemoryOrdering::ReceivePublish,
        )
        .expect("encode");
        assert_eq!(
            bytes.len(),
            runtime_atomic_fetch_xor_width(&operands, 0, 4, 24, value)
        );
        let atomic_end = runtime_atomic_fetch_xor_result_address_offset(&operands, 0, value);
        assert_eq!(
            u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap()),
            0xB8F1_221A,
            "fetch_xor must emit LDEORAL w17,w26,[x16]"
        );
    }

    #[test]
    fn atomic_fetch_or_uses_ordered_ldset_and_returns_prior() {
        use omega_target_operations::RuntimeValueOperand;
        use psi_arena::Arena;

        let mut operands: Arena<RuntimeValueOperand> = Arena::default();
        let value = operands.insert(RuntimeValueOperand::Immediate(5));
        let bytes = encode_atomic_fetch_or(
            &operands,
            0,
            4,
            24,
            value,
            psi_language_core::MemoryOrdering::ReceivePublish,
        )
        .expect("encode");
        assert_eq!(
            bytes.len(),
            runtime_atomic_fetch_or_width(&operands, 0, 4, 24, value)
        );
        let atomic_end = runtime_atomic_fetch_or_result_address_offset(&operands, 0, value);
        assert_eq!(
            u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap()),
            0xB8F1_321A,
            "fetch_or must emit LDSETAL w17,w26,[x16]"
        );
    }

    /// `CASAL <Ws/Xs>, <Wt/Xt>, [<Xn>]` per width: size field selects the access
    /// size, Rs (bits 20:16) = compare/expected, Rn (bits 9:5) = address, Rt
    /// (bits 4:0) = new value, with the acquire(L)/release(o0)/Rt2 fixed bits set.
    #[test]
    fn casal_encodes_per_width() {
        use super::super::primitives::encode_cas;
        for &(byte_size, size) in &[(1usize, 0u32), (2, 1), (4, 2), (8, 3)] {
            let word = u32::from_le_bytes(
                encode_cas(
                    byte_size,
                    26,
                    17,
                    16,
                    psi_language_core::MemoryOrdering::ReceivePublish,
                )
                .expect("encode")[..]
                    .try_into()
                    .unwrap(),
            );
            let expected = 0x08E0_FC00 | (size << 30) | (26u32 << 16) | (16u32 << 5) | 17;
            assert_eq!(word, expected, "byte_size={byte_size}");
            assert_eq!(word >> 30, size, "size field");
            assert_eq!((word >> 16) & 0x1F, 26, "Rs = expected (compare/old)");
            assert_eq!((word >> 5) & 0x1F, 16, "Rn = address register");
            assert_eq!(word & 0x1F, 17, "Rt = new value");
            assert_eq!((word >> 10) & 0x1F, 0x1F, "Rt2 fixed 11111");
        }
        assert!(
            encode_cas(3, 26, 17, 16, psi_language_core::MemoryOrdering::NoOrdering,).is_err(),
            "non-power-of-two errors"
        );
        let words = [
            psi_language_core::MemoryOrdering::NoOrdering,
            psi_language_core::MemoryOrdering::Receive,
            psi_language_core::MemoryOrdering::Publish,
            psi_language_core::MemoryOrdering::ReceivePublish,
            psi_language_core::MemoryOrdering::GlobalOrder,
        ]
        .map(|ordering| u32::from_le_bytes(encode_cas(4, 26, 17, 16, ordering).unwrap()));
        assert_eq!(
            words,
            [
                0x88BA_7E11,
                0x88FA_7E11,
                0x88BA_FE11,
                0x88FA_FE11,
                0x88FA_FE11,
            ]
        );
    }

    /// Full `encode_atomic_compare_exchange`: emitted length equals the width fn
    /// at every offset, and the final instruction is `CASAL w26, w17, [x16]`.
    #[test]
    fn atomic_compare_exchange_encoder_matches_width_and_ends_in_casal() {
        use omega_target_operations::RuntimeValueOperand;
        use psi_arena::Arena;

        for &target_offset in &[0usize, 4, 4095] {
            let mut operands: Arena<RuntimeValueOperand> = Arena::default();
            let expected = operands.insert(RuntimeValueOperand::Immediate(10));
            let new_value = operands.insert(RuntimeValueOperand::Immediate(99));
            let result_offset = 32;
            let bytes = encode_atomic_compare_exchange(
                &operands,
                target_offset,
                4,
                result_offset,
                expected,
                new_value,
                psi_language_core::MemoryOrdering::ReceivePublish,
            )
            .expect("encode");
            assert_eq!(
                bytes.len(),
                runtime_atomic_compare_exchange_width(
                    &operands,
                    target_offset,
                    4,
                    result_offset,
                    expected,
                    new_value
                ),
                "width mismatch at offset {target_offset}"
            );
            let atomic_end = runtime_atomic_compare_exchange_result_address_offset(
                &operands,
                target_offset,
                expected,
                new_value,
            );
            let last = u32::from_le_bytes(bytes[atomic_end - 4..atomic_end].try_into().unwrap());
            assert_eq!(
                last, 0x88FA_FE11,
                "final instruction must be CASAL w26, w17, [x16] at offset {target_offset}"
            );
        }

        let mut operands: Arena<RuntimeValueOperand> = Arena::default();
        let expected = operands.insert(RuntimeValueOperand::Immediate(1));
        let new_value = operands.insert(RuntimeValueOperand::Immediate(2));
        assert!(
            encode_atomic_compare_exchange(
                &operands,
                4096,
                4,
                0,
                expected,
                new_value,
                psi_language_core::MemoryOrdering::NoOrdering,
            )
            .is_err()
        );
    }

    /// Every supported index width keeps the fixed-width address recipe, so
    /// width functions remain independent of the final load opcode.
    #[test]
    fn unsigned_index_loads_match_fixed_x_load_width() {
        let mut x_bytes = Vec::new();
        append_fixed_width_load_x_from_x_offset(&mut x_bytes, 17, 20, 0x40, 21);
        for byte_size in [1, 2, 4, 8] {
            let mut index_bytes = Vec::new();
            append_fixed_width_load_unsigned_index_from_x_offset(
                &mut index_bytes,
                17,
                20,
                0x40,
                byte_size,
                21,
            );
            assert_eq!(index_bytes.len(), x_bytes.len());
            assert_eq!(index_bytes.len(), 24);
        }
    }

    /// The final instruction must be `LDR Wt` (opcode family 0xB9400000), which
    /// zero-extends the upper 32 bits, NOT `LDR Xt` (0xF9400000).
    #[test]
    fn index_w_load_emits_w_register_load() {
        let mut bytes = Vec::new();
        append_fixed_width_load_unsigned_index_from_x_offset(&mut bytes, 17, 20, 0x40, 4, 21);
        let last = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
        // size field (bits 30-31) of LDR Wt is 0b10; LDR Xt is 0b11.
        assert_eq!(last & 0xFFC0_0000, 0xB940_0000, "expected LDR Wt (32-bit)");
    }

    #[test]
    fn index_load_uses_the_exact_declared_width() {
        for index_byte_size in [1usize, 2, 4, 8] {
            let mut bytes = Vec::new();
            append_fixed_width_load_unsigned_index_from_x_offset(
                &mut bytes,
                17,
                20,
                0x40,
                index_byte_size,
                21,
            );
            let emitted = &bytes[bytes.len() - 4..];
            let expected = match index_byte_size {
                1 | 2 | 4 => encode_load_w_from_x(17, 21, 0, index_byte_size).unwrap(),
                8 => encode_load_x_from_x(17, 21, 0).unwrap(),
                _ => unreachable!(),
            };
            assert_eq!(emitted, expected, "index width {index_byte_size}");
        }
    }

    /// The frame-index target-address setup width must match what the encoder
    /// emits for every exact-width index load.
    #[test]
    fn frame_index_setup_width_matches_emission() {
        for &(element_size, field_offset) in &[(1usize, 0usize), (4, 0), (8, 8), (24, 16), (40, 0)]
        {
            let mut bytes = Vec::new();
            append_runtime_frame_index_target_address(
                &mut bytes,
                16,
                0x10,
                0x40,
                4,
                element_size,
                field_offset,
                17,
                26,
            )
            .unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_frame_index_setup_width(element_size, field_offset),
                "element_size={element_size}, field_offset={field_offset}"
            );
        }
    }

    #[test]
    fn frame_indexed_operand_keeps_pointee_and_machine_index_bases_distinct() {
        let mut bytes = Vec::new();
        append_runtime_frame_index_target_address_with_index_region(
            &mut bytes,
            15,
            omega_target_operations::RuntimeStorageRegion::Machine,
            0x10,
            0x40,
            4,
            4,
            2,
            17,
            26,
        )
        .unwrap();

        let machine_adrp = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        assert_eq!(
            machine_adrp & 0x1f,
            19,
            "machine base must not overwrite x15"
        );
        assert_eq!(
            bytes.len(),
            widths::runtime_frame_index_setup_width(4, 2) + 8
        );
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
                4,
                element_size,
                source_field,
                pointer_offset,
                target_field,
                byte_count,
            )
            .unwrap();
            let expected =
                widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
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
                base,
                index_off,
                4,
                element_size,
                field,
                value_size,
                7,
            )
            .unwrap();
            assert_eq!(
                bytes.len(),
                widths::runtime_frame_base_indexed_integer_write_width(
                    base,
                    index_off,
                    4,
                    element_size,
                    field,
                    value_size,
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
        let single =
            encode_runtime_storage_compare_bytes(0x10, 0x20, 4, 8, StateGuardOperator::Less, true)
                .unwrap();
        let double =
            encode_runtime_storage_compare_bytes(0x10, 0x20, 8, 8, StateGuardOperator::Less, true)
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
        psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
        omega_target_operations::RuntimeValueOperandHandle,
    ) {
        let mut arena = psi_arena::Arena::new();
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
            (0x10, 8, 4, true, true, true), // f64 -> f32 (FCVT narrow)
            (0x20, 4, 8, true, true, true), // f32 -> f64 (FCVT widen)
            (0x18, 8, 8, true, true, true), // f64 -> f64 (no-op convert)
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
                true,
                false,
                false,
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
                true,
                false,
                false,
            );
            assert_eq!(
                bytes.len(),
                width,
                "len != width for target_offset={target_offset:#x}, src_size={src_size}, tgt_size={tgt_size}, src_float={src_float}, tgt_float={tgt_float}, src_signed={src_signed}"
            );
        }
    }

    #[test]
    fn float_to_int_policy_shapes_match_width_for_signedness_and_narrowing() {
        for target_byte_size in [1usize, 2, 4, 8] {
            for target_signed in [false, true] {
                for (trapping, saturating) in [(true, false), (false, true)] {
                    let (arena, source) = storage_source(8);
                    let bytes = encode_runtime_storage_convert(
                        &arena,
                        0x10,
                        target_byte_size,
                        source,
                        8,
                        true,
                        false,
                        false,
                        target_signed,
                        trapping,
                        saturating,
                    )
                    .expect("policy conversion encodes");
                    let width = widths::runtime_storage_convert_width(
                        &arena,
                        0x10,
                        source,
                        8,
                        target_byte_size,
                        true,
                        false,
                        false,
                        target_signed,
                        trapping,
                        saturating,
                    );
                    assert_eq!(
                        bytes.len(),
                        width,
                        "target={target_byte_size} signed={target_signed} trapping={trapping}"
                    );
                }
            }
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
        let bytes = encode_runtime_storage_convert(
            &arena, 0x10, 8, source, 4, false, true, true, true, false, false,
        )
        .unwrap();
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

        // unsigned int(x) -> double selects UCVTF, preserving the upper half
        // of u64 instead of treating it as a negative signed integer.
        let (arena, source) = storage_source(8);
        let bytes = encode_runtime_storage_convert(
            &arena, 0x10, 8, source, 8, false, true, false, true, false, false,
        )
        .unwrap();
        let ucvtf = word_at(&bytes, 12);
        assert_eq!(ucvtf, 0x9e63_0000 | (17 << 5), "UCVTF d0, x17");

        // double -> int(w): FMOV d0,x17 (0x9e67_0000) + FCVTZS w17,d0
        // (0x1e38_0000 family).
        let (arena, source) = storage_source(8);
        let bytes = encode_runtime_storage_convert(
            &arena, 0x10, 4, source, 8, true, false, true, true, false, false,
        )
        .unwrap();
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
            &arena, 0x10, source, 4, 8, false, false, true, true, false, false,
        );
        let unsigned_width = widths::runtime_storage_convert_width(
            &arena, 0x10, source, 4, 8, false, false, false, false, false, false,
        );
        assert_eq!(
            signed_width,
            unsigned_width + 4,
            "signed widen must be exactly one SXTW longer than unsigned"
        );
        let signed_bytes = encode_runtime_storage_convert(
            &arena, 0x10, 8, source, 4, false, false, true, true, false, false,
        )
        .unwrap();
        // SXTW x17, w17: 0x93407c00 | (17<<5) | 17 — it sits right before the store.
        let store_width = if signed_bytes.len() >= 8 { 4 } else { 0 };
        let _ = store_width;
        let sxtw_start = signed_bytes.len() - 8; // SXTW (4) + STR (4)
        let sxtw = u32::from_le_bytes(signed_bytes[sxtw_start..sxtw_start + 4].try_into().unwrap());
        assert_eq!(sxtw, 0x9340_7c00 | (17 << 5) | 17, "SXTW x17, w17");
    }

    /// Build a value-operand arena with two immediate operands (a deterministic,
    /// relocation-free load width) and return the arena and both handles.
    fn immediate_pair(
        left: i64,
        right: i64,
    ) -> (
        psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
        omega_target_operations::RuntimeValueOperandHandle,
        omega_target_operations::RuntimeValueOperandHandle,
    ) {
        let mut arena = psi_arena::Arena::new();
        let left = arena.insert(omega_target_operations::RuntimeValueOperand::Immediate(
            left,
        ));
        let right = arena.insert(omega_target_operations::RuntimeValueOperand::Immediate(
            right,
        ));
        (arena, left, right)
    }

    /// The saturating/trapping add/sub/mul encoder length must equal its width
    /// function for every (domain, operator, byte_size, signed) combination — the
    /// internal `debug_assert_eq!` also fires here. Covers all 1/2/4-byte widths.
    #[test]
    fn saturating_trapping_binary_write_width_matches_emission() {
        use psi_numerics::arithmetic::ArithmeticDomain;
        let (arena, left, right) = immediate_pair(100, 100);
        for &domain in &[ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
            for &operator in &[
                StateGuardOperator::Add,
                StateGuardOperator::Subtract,
                StateGuardOperator::Multiply,
            ] {
                for &byte_size in &[1usize, 2, 4] {
                    for &signed in &[false, true] {
                        let bytes = encode_runtime_storage_binary_write(
                            &arena, 0x10, byte_size, left, operator, right, false, domain, signed,
                        )
                        .unwrap();
                        let width = widths::runtime_storage_binary_write_width(
                            &arena, 0x10, byte_size, left, operator, right, false, domain, signed,
                        );
                        assert_eq!(
                            bytes.len(),
                            width,
                            "len != width for domain={domain:?}, operator={operator:?}, byte_size={byte_size}, signed={signed}"
                        );
                    }
                }
            }
        }
    }

    /// The signed saturating add at 1 byte must sign-extend BOTH operands (SXTB
    /// Xd,Wn = 0x9340_1C00 family) before the wide ADD, materialize the bounds
    /// with MOVZ/MOVK, and clamp with CMP + b.cond + MOV (no BRK).
    #[test]
    fn signed_saturating_add_byte_sign_extends_and_clamps() {
        use psi_numerics::arithmetic::ArithmeticDomain;
        let (arena, left, right) = immediate_pair(100, 100);
        let bytes = encode_runtime_storage_binary_write(
            &arena,
            0x10,
            1,
            left,
            StateGuardOperator::Add,
            right,
            false,
            ArithmeticDomain::Saturating,
            true,
        )
        .unwrap();
        // IMMEDIATE operands are loaded at their true wide value, so the
        // signed extension is SKIPPED for them (extending from the target
        // width corrupts a wide literal -- the MIN-idiom fix): expect ZERO
        // SXTB here. Storage operands keep their per-side extension (the
        // width twin mirrors the same per-side skip).
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        // SXTB Xd, Wn family is 0x9340_1C00.
        let sxtb_count = words
            .iter()
            .filter(|w| (*w & 0xFFFF_FC00) == 0x9340_1C00)
            .count();
        assert_eq!(sxtb_count, 0, "immediate operands must not re-extend");
        // Exactly one wide ADD Xd,Xn,Xm (0x8B00_0000 family) — the saturating op.
        let add_count = words
            .iter()
            .filter(|w| (*w & 0xFF20_0000) == 0x8B00_0000)
            .count();
        assert_eq!(add_count, 1, "expected one wide ADD");
        // Saturating must NOT emit a BRK (0xD420_0000 family).
        assert!(
            !words.iter().any(|w| (*w & 0xFFE0_001F) == 0xD420_0000),
            "saturating must not trap"
        );
    }

    /// Trapping add must emit BRK instructions (0xD420_0000) on the overflow
    /// paths, and unsigned must check both the 0 lower bound and the MAX upper
    /// bound.
    #[test]
    fn unsigned_trapping_narrow_brk_per_overflow_direction() {
        // Unsigned wide results overflow in ONE direction per operator, so
        // each narrow unsigned trapping arm emits exactly ONE brk (the old
        // both-checks tail emitted two -- and its SIGNED lower compare
        // misread 2^63+ products); signed arms keep both bound checks.
        use psi_numerics::arithmetic::ArithmeticDomain;
        let brk_count = |bytes: &[u8]| {
            bytes
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .filter(|w| (*w & 0xFFE0_001F) == 0xD420_0000)
                .count()
        };
        for (operator, signed, expected) in [
            (StateGuardOperator::Add, false, 1),
            (StateGuardOperator::Subtract, false, 1),
            (StateGuardOperator::Multiply, false, 1),
            (StateGuardOperator::Add, true, 2),
            (StateGuardOperator::Multiply, true, 2),
        ] {
            let (arena, left, right) = immediate_pair(200, 200);
            let bytes = encode_runtime_storage_binary_write(
                &arena,
                0x10,
                1,
                left,
                operator,
                right,
                false,
                ArithmeticDomain::Trapping,
                signed,
            )
            .unwrap();
            assert_eq!(
                brk_count(&bytes),
                expected,
                "brk count for {operator:?} (signed: {signed})"
            );
        }
    }

    /// 64-bit saturating/trapping arithmetic (the flag/MULH-based clamps):
    /// every (domain x signedness x operator) arm's emitted length must match
    /// the width helper, or relocation offsets drift.
    #[test]
    fn saturating_eight_byte_arithmetic_width_matches_emission() {
        use psi_numerics::arithmetic::ArithmeticDomain;
        for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
            for signed in [true, false] {
                for operator in [
                    StateGuardOperator::Add,
                    StateGuardOperator::Subtract,
                    StateGuardOperator::Multiply,
                    StateGuardOperator::ShiftLeft,
                ] {
                    let (arena, left, right) = immediate_pair(5, 5);
                    let bytes = encode_runtime_storage_binary_write(
                        &arena, 0x10, 8, left, operator, right, false, domain, signed,
                    )
                    .unwrap_or_else(|error| {
                        panic!(
                            "8-byte {domain:?} {operator:?} signed={signed} should encode: {error}"
                        )
                    });
                    assert_eq!(
                        bytes.len(),
                        widths::runtime_storage_binary_write_width(
                            &arena, 0x10, 8, left, operator, right, false, domain, signed,
                        ),
                        "width drift: {domain:?} {operator:?} signed={signed}"
                    );
                }
            }
        }
    }

    #[test]
    fn trapping_float_policy_is_one_result_only_guard() {
        use psi_numerics::arithmetic::ArithmeticDomain;

        for byte_size in [4usize, 8] {
            for operator in [
                StateGuardOperator::Add,
                StateGuardOperator::Min,
                StateGuardOperator::Max,
                StateGuardOperator::Sqrt,
            ] {
                let bytes = float_policy_guard_bytes(
                    ArithmeticDomain::Trapping,
                    operator,
                    byte_size,
                    17,
                    26,
                    None,
                    15,
                    14,
                )
                .expect("encode result-only policy guard");
                let brk_count = bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                    .filter(|word| (*word & 0xFFE0_001F) == 0xD420_0000)
                    .count();
                assert_eq!(brk_count, 1, "f{} {operator:?}", byte_size * 8);
                assert_eq!(
                    bytes.len(),
                    float_policy_guard_width(operator, byte_size, ArithmeticDomain::Trapping)
                );
            }
        }
    }

    #[test]
    fn multiply_then_add_emission_keeps_two_operations_and_width_lockstep() {
        use psi_numerics::arithmetic::ArithmeticDomain;

        for byte_size in [4usize, 8] {
            for domain in [
                ArithmeticDomain::Exact,
                ArithmeticDomain::Saturating,
                ArithmeticDomain::Trapping,
            ] {
                let mut bytes = Vec::new();
                append_runtime_float_binary_operation(
                    &mut bytes,
                    byte_size,
                    17,
                    StateGuardOperator::MultiplyThenAdd,
                    26,
                    domain,
                    [15, 14],
                )
                .expect("encode multiply-then-add");
                assert_eq!(
                    bytes.len(),
                    24 + float_policy_guard_width(
                        StateGuardOperator::MultiplyThenAdd,
                        byte_size,
                        domain,
                    ),
                    "f{} {domain:?} width",
                    byte_size * 8,
                );
                let instructions = bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect::<Vec<_>>();
                assert!(
                    instructions.contains(&u32::from_le_bytes(
                        encode_float_multiply(byte_size, 0, 0, 1).expect("encode scalar multiply"),
                    )),
                    "f{} must contain a scalar multiply",
                    byte_size * 8,
                );
                assert!(
                    instructions.contains(&u32::from_le_bytes(
                        encode_float_add(byte_size, 0, 0, 1).expect("encode scalar add"),
                    )),
                    "f{} must contain a separate scalar add",
                    byte_size * 8,
                );
            }
        }
    }

    #[test]
    fn directed_add_balances_fpcr_and_widths() {
        use psi_numerics::arithmetic::ArithmeticDomain;

        for (operator, fpcr) in [
            (StateGuardOperator::AddTowardPositive, 0x0040_0000_u64),
            (StateGuardOperator::AddTowardNegative, 0x0080_0000_u64),
            (StateGuardOperator::AddTowardZero, 0x00c0_0000_u64),
        ] {
            for byte_size in [4usize, 8] {
                let mut bytes = Vec::new();
                append_runtime_float_binary_operation(
                    &mut bytes,
                    byte_size,
                    17,
                    operator,
                    26,
                    ArithmeticDomain::Exact,
                    [15, 14],
                )
                .expect("encode directed add");
                assert_eq!(
                    bytes.len(),
                    widths::runtime_float_binary_operation_width_with_domain(
                        operator,
                        byte_size,
                        ArithmeticDomain::Exact,
                    ),
                    "f{} {operator:?}",
                    byte_size * 8,
                );
                let words = bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect::<Vec<_>>();
                assert_eq!(words[2], u32::from_le_bytes(encode_read_fpcr(13)));
                assert_eq!(words[3], u32::from_le_bytes(encode_movz(12, 0)));
                assert_eq!(
                    words[4],
                    u32::from_le_bytes(encode_movk(12, ((fpcr >> 16) & 0xffff) as u16, 1,))
                );
                assert_eq!(words[5], u32::from_le_bytes(encode_write_fpcr(12)));
                assert_eq!(words[7], u32::from_le_bytes(encode_write_fpcr(13)));
            }
        }
    }

    #[test]
    fn fused_multiply_add_emission_keeps_one_fmadd_and_width_lockstep() {
        use psi_numerics::arithmetic::ArithmeticDomain;

        for byte_size in [4usize, 8] {
            for domain in [
                ArithmeticDomain::Exact,
                ArithmeticDomain::Saturating,
                ArithmeticDomain::Trapping,
            ] {
                let mut bytes = Vec::new();
                append_runtime_float_binary_operation(
                    &mut bytes,
                    byte_size,
                    17,
                    StateGuardOperator::FusedMultiplyAdd,
                    26,
                    domain,
                    [15, 14],
                )
                .expect("encode fused multiply-add");
                assert_eq!(
                    bytes.len(),
                    20 + float_policy_guard_width(
                        StateGuardOperator::FusedMultiplyAdd,
                        byte_size,
                        domain,
                    ),
                    "f{} {domain:?} width",
                    byte_size * 8,
                );
                let instructions = bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect::<Vec<_>>();
                assert!(
                    instructions.contains(&u32::from_le_bytes(
                        encode_float_fused_multiply_add(byte_size, 0, 0, 1, 2)
                            .expect("encode scalar FMADD"),
                    )),
                    "f{} must contain one scalar FMADD",
                    byte_size * 8,
                );
                assert!(
                    !instructions.contains(&u32::from_le_bytes(
                        encode_float_multiply(byte_size, 0, 0, 1).expect("encode scalar multiply"),
                    )),
                    "f{} FMA must not contain a separately rounded multiply",
                    byte_size * 8,
                );
                assert!(
                    !instructions.contains(&u32::from_le_bytes(
                        encode_float_add(byte_size, 0, 0, 1).expect("encode scalar add"),
                    )),
                    "f{} FMA must not contain a separate add",
                    byte_size * 8,
                );
            }
        }
    }
}
