use omega_target_operations::{RuntimeValueOperandHandle, RuntimeValueOperandSource};
use psi_diagnostics::Diagnostic;

use super::{
    RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS, append_add_constant_to_x_register,
    append_double_index_address_math, append_double_index_bases,
    append_runtime_frame_base_index_target_address_with_index_region,
    append_runtime_frame_index_target_address_with_index_region,
    append_runtime_machine_index_target_address, append_runtime_storage_load,
    append_runtime_storage_result_write, append_runtime_value_operand,
};
use crate::aarch64::primitives::{
    append_unsigned_immediate_padded, encode_add_page_offset_placeholder, encode_adrp_placeholder,
    encode_brk, encode_compare_x_register, encode_conditional_branch_greater,
    encode_conditional_branch_greater_or_equal, encode_conditional_branch_less,
    encode_conditional_branch_no_overflow, encode_csel_x, encode_float_compare,
    encode_float_convert_double_to_single, encode_float_convert_single_to_double,
    encode_float_move_from_gpr, encode_float_move_to_gpr, encode_float_to_signed_int,
    encode_float_to_unsigned_int, encode_sign_extend_byte_to_w, encode_sign_extend_byte_to_x,
    encode_sign_extend_halfword_to_w, encode_sign_extend_halfword_to_x,
    encode_sign_extend_word_to_x, encode_signed_int_to_float, encode_unsigned_int_to_float,
    encode_zero_extend_byte_to_w, encode_zero_extend_halfword_to_w,
};
use crate::aarch64::widths::runtime_storage_convert_width;

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

/// Convert one runtime value and store it through a frame-held pointer. The
/// pointee address remains in x16 while recursive operand evaluation and the
/// conversion use x17 plus the ordinary scratch bank.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_pointee_convert_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
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

/// Convert one runtime value and store it through a frame-held indexed
/// descriptor. Address setup leaves the element address in x16 while the
/// ordinary conversion evaluator uses the disjoint left-operand scratch bank.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_indexed_convert_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    descriptor_offset: usize,
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

/// Convert one runtime value and store it into a runtime-indexed inline frame
/// array. The shared frame pair supplies both the array and index bases.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_convert_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
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
    encode_runtime_frame_base_indexed_convert_write_with_index_region(
        runtime_value_operands,
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_byte_size,
        source,
        source_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_convert_write_with_index_region(
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
    append_runtime_frame_base_index_target_address_with_index_region(
        &mut bytes,
        16,
        base_byte_offset,
        index_region,
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

/// Convert one runtime value and store it into a double-runtime-indexed
/// machine array. The fixed address program finishes before operand
/// evaluation, leaving only the element address live in x16.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_convert_write(
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

/// Convert one runtime value into an all-frame double-indexed element. The
/// collection and both index slots share the one relocated frame base in x16;
/// operand evaluation starts after the fixed 36-byte address program.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_convert_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
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
    bytes.extend(encode_adrp_placeholder(16));
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
pub(super) fn append_runtime_convert_operation(
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
