use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

use super::{
    RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS, RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
    append_add_constant_to_x_register, append_load_data_from_x_offset,
    append_runtime_binary_operation, append_runtime_binary_operation_with_domain,
    append_runtime_float_binary_operation, append_runtime_storage_load,
    append_runtime_storage_result_write, append_runtime_value_operand,
    append_shift_count_trap_guard, append_store_data_to_x_offset,
    runtime_binary_operation_byte_size, runtime_value_compare_register_write_ceiling,
};
use crate::aarch64::primitives::{
    append_unsigned_immediate, append_unsigned_immediate_padded,
    append_unsigned_immediate_w_padded, encode_add_page_offset_placeholder, encode_add_x_register,
    encode_adds_x_register, encode_adrp_placeholder, encode_and_x_register, encode_asrv_x_register,
    encode_brk, encode_compare_x_immediate, encode_compare_x_register,
    encode_compare_x_register_sign_broadcast, encode_conditional_branch_equal,
    encode_conditional_branch_greater_or_equal, encode_conditional_branch_higher_or_same,
    encode_conditional_branch_less_or_equal, encode_conditional_branch_lower,
    encode_conditional_branch_lower_or_same, encode_conditional_branch_no_overflow,
    encode_conditional_branch_not_equal, encode_csel_x, encode_csinv_x, encode_eor_x_register,
    encode_lslv_x_register, encode_lsrv_x_register, encode_move_x_register, encode_movz,
    encode_msub_x_register, encode_mul_x_register, encode_orr_x_register, encode_sdiv_x_register,
    encode_sign_extend_byte_to_x, encode_sign_extend_halfword_to_x, encode_sign_extend_word_to_x,
    encode_smulh_x, encode_store_w_to_x, encode_store_w17_to_x16, encode_store_x_to_x,
    encode_store_x17_to_x16, encode_sub_x_register, encode_subs_x_immediate,
    encode_subs_x_register, encode_umulh_x, encode_unconditional_branch,
};
use crate::aarch64::widths::{
    bit_fragment_container_bytes, runtime_machine_integer_write_width,
    runtime_pointee_binary_write_width, runtime_pointee_integer_write_width,
    runtime_storage_binary_write_width, runtime_storage_bit_field_write_width,
};

pub fn place_binary_write_register_write_ceiling() -> RegisterSet {
    let mut registers = runtime_value_compare_register_write_ceiling()
        .as_slice()
        .to_vec();
    registers.push(MachineRegister::Aarch64X(16));
    RegisterSet::new(registers)
}

/// Closed may-write ceiling of a direct conversion write. It shares the
/// recursive runtime-value evaluator with binary writes and preserves the
/// relocated destination in x16 while conversion policy may use x15/v0/v1.
pub fn storage_convert_write_register_write_ceiling() -> RegisterSet {
    place_binary_write_register_write_ceiling()
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
                | StateGuardOperator::SubtractTowardZero
                | StateGuardOperator::SubtractTowardPositive
                | StateGuardOperator::SubtractTowardNegative
                | StateGuardOperator::MultiplyTowardZero
                | StateGuardOperator::MultiplyTowardPositive
                | StateGuardOperator::MultiplyTowardNegative
                | StateGuardOperator::DivideTowardZero
                | StateGuardOperator::DivideTowardPositive
                | StateGuardOperator::DivideTowardNegative
                | StateGuardOperator::SqrtTowardZero
                | StateGuardOperator::SqrtTowardPositive
                | StateGuardOperator::SqrtTowardNegative
                | StateGuardOperator::FusedMultiplyAddTowardZero
                | StateGuardOperator::FusedMultiplyAddTowardPositive
                | StateGuardOperator::FusedMultiplyAddTowardNegative
        ) || runtime_value_operand_uses_control_state(runtime_value_operands, left)
            || runtime_value_operand_uses_control_state(runtime_value_operands, right)
    } else if let Some((source, ..)) = runtime_value_operands.convert(operand) {
        runtime_value_operand_uses_control_state(runtime_value_operands, source)
    } else {
        false
    }
}

/// Machine state touched while materializing one runtime value operand, before
/// the enclosing operation applies its own effects. AArch64 integer address
/// and bit-field materialization uses non-flag-setting instructions; flags are
/// needed only by comparison-shaped values and guarded arithmetic/conversions.
pub fn runtime_value_operand_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> MachineStateSet {
    let mut state = MachineStateSet::empty();
    let binary_writes_flags = runtime_value_operands.binary(operand).is_some();
    let conversion_writes_flags = runtime_value_operands.convert(operand).is_some()
        && (runtime_value_operands.convert_trapping(operand)
            || runtime_value_operands.convert_saturating(operand));
    if binary_writes_flags
        || conversion_writes_flags
        || runtime_value_operands.text_equals(operand).is_some()
        || runtime_value_operands
            .text_equals_literal(operand)
            .is_some()
    {
        state = state.union(MachineStateSet::new([MachineState::Flags]));
    }
    if runtime_value_operand_uses_control_state(runtime_value_operands, operand) {
        state = state.union(MachineStateSet::new([MachineState::ControlState]));
    }
    state
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

pub fn storage_convert_write_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    source: RuntimeValueOperandHandle,
) -> MachineStateSet {
    runtime_value_compare_additional_machine_state(runtime_value_operands, source, source)
}

/// Machine state touched by a direct place-shaped binary write. Integer
/// policy/comparison paths may write flags; directed floating operations
/// temporarily change FPCR before restoring it.
pub fn place_binary_write_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> MachineStateSet {
    let mut state = MachineStateSet::new([MachineState::Flags]);
    let operator_uses_control_state = matches!(
        operator,
        StateGuardOperator::AddTowardZero
            | StateGuardOperator::AddTowardPositive
            | StateGuardOperator::AddTowardNegative
            | StateGuardOperator::SubtractTowardZero
            | StateGuardOperator::SubtractTowardPositive
            | StateGuardOperator::SubtractTowardNegative
            | StateGuardOperator::MultiplyTowardZero
            | StateGuardOperator::MultiplyTowardPositive
            | StateGuardOperator::MultiplyTowardNegative
            | StateGuardOperator::DivideTowardZero
            | StateGuardOperator::DivideTowardPositive
            | StateGuardOperator::DivideTowardNegative
            | StateGuardOperator::SqrtTowardZero
            | StateGuardOperator::SqrtTowardPositive
            | StateGuardOperator::SqrtTowardNegative
            | StateGuardOperator::FusedMultiplyAddTowardZero
            | StateGuardOperator::FusedMultiplyAddTowardPositive
            | StateGuardOperator::FusedMultiplyAddTowardNegative
    );
    if operator_uses_control_state
        || runtime_value_operand_uses_control_state(runtime_value_operands, left)
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

/// Exact scratch footprint of a direct immediate integer write. x16 owns the
/// relocated destination address, x17 materializes the value, and a large
/// destination offset uses the shared x19 constant-address scratch.
pub fn runtime_machine_integer_write_clobbers(byte_offset: usize) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)];
    if byte_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    RegisterSet::new(registers)
}

/// Exact scratch footprint of an immediate integer write through a pointer
/// held in the runtime frame. x16 materializes and dereferences the frame
/// slot, x17 carries the value, and either large offset uses x19 through the
/// shared constant-address helper.
pub fn runtime_pointee_integer_write_clobbers(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)];
    if pointer_byte_offset > 4095 || field_byte_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    RegisterSet::new(registers)
}

pub(super) fn bit_width_mask(width: u16) -> Result<u64, Diagnostic> {
    match width {
        1..=63 => Ok((1_u64 << width) - 1),
        64 => Ok(u64::MAX),
        _ => Err(Diagnostic::error("AArch64 bit-field width must be 1..=64")),
    }
}

pub(super) fn validate_runtime_bit_field_fragment(
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

/// Closed may-write ceiling of the immediate bit-field read/modify/write
/// encoder. x16 owns the relocated base, x17 stages each container, x20
/// materializes masks, and x19/x26 cover the shared large-offset recipe.
pub fn runtime_storage_bit_field_write_register_write_ceiling() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

pub const fn runtime_storage_bit_field_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
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
pub(super) fn append_saturating_trapping_arithmetic(
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
pub(super) fn append_saturating_signed_divide_modulo(
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
