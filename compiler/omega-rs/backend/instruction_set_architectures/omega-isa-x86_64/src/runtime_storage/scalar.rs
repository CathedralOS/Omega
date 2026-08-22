use super::super::{
    Reg64, append_add_rax_r11, append_cmp_r10_r11, append_failure_branch, append_imul_r11_imm32,
    append_load_rax_from_r15, append_load_reg_from_r15, append_load_reg_from_rax,
    append_load_unsigned_reg_from_r15, append_load_unsigned_reg_from_rax, append_mov_r14_imm64,
    append_mov_r15_imm64, append_mov_rax_r15, append_mov_reg_imm64, append_mov_reg_reg,
    append_pop_r10, append_push_r10, append_store_r10_to_r14, disp32, element_scale, load_width,
    place_copy, rel32, store_width, unsigned_load_width,
};
use super::places::{append_runtime_bit_field_operand, runtime_bit_field_operand_width};
use omega_target_operations::{
    RuntimeStorageRegion, RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;

mod operand_footprints;
pub use operand_footprints::*;

#[cfg(test)]
#[path = "scalar_tests.rs"]
mod tests;

pub fn runtime_value_compare_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    // cmp (3; 4 with the 0x66 prefix at 2-byte width) + jcc rel32 (6).
    let compare_width = if byte_size == 2 { 4 } else { 3 };
    runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + compare_width
        + 6
}

pub fn encode_runtime_value_compare(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_value_compare_width(
        runtime_value_operands,
        byte_size,
        left,
        right,
    ));
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R11, right)?;
    append_cmp_r10_r11(&mut bytes, byte_size)?;
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4, false)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_value_compare_width(runtime_value_operands, byte_size, left, right)
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> usize {
    // The integer op is normally the default 64-bit op; Saturating/Trapping
    // instead emit a width-correct add/sub followed by the clamp/trap sequence.
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    let operation_width = if saturating_or_trapping && operator == StateGuardOperator::Multiply {
        saturating_trapping_multiply_width(
            domain,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )
    } else if saturating_or_trapping && operator == StateGuardOperator::ShiftLeft {
        saturating_trapping_shift_left_width(domain, byte_size, target_signed)
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add | StateGuardOperator::Subtract
        )
    {
        saturating_trapping_add_sub_width(
            domain,
            operator,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )
    } else if domain == ArithmeticDomain::Saturating
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Saturating SIGNED divide/modulo wraps the normal idiv in a TYPE_MIN/-1
        // guard (see append_saturating_signed_divide_modulo).
        saturating_signed_divide_modulo_width(byte_size, operator == StateGuardOperator::Modulo)
    } else if domain == ArithmeticDomain::Wrapping
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Wrapping SIGNED divide/modulo guards TYPE_MIN/-1 so idiv does not #DE
        // (see append_wrapping_signed_divide_modulo). Unsigned uses the *Unsigned
        // operators and cannot overflow, so it falls through.
        wrapping_signed_divide_modulo_width(byte_size, operator == StateGuardOperator::Modulo)
    } else if (domain == ArithmeticDomain::Wrapping && operator == StateGuardOperator::ShiftLeft)
        || (domain != ArithmeticDomain::Exact
            && matches!(
                operator,
                StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical
            ))
    {
        // Domain-governed shifts: F8b WRAPPING masks the COUNT (sub-word AND
        // only; the hardware mask IS the ruling at widths 4/8), while
        // Saturating/Trapping `>>` keep the floor-semantics count fixes
        // until F8c. Same operand-derived byte size as the emission arm.
        let operation_byte_size = runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        );
        let fix = if domain == ArithmeticDomain::Wrapping {
            wrapping_shift_count_mask_width(operation_byte_size)
        } else if domain == ArithmeticDomain::Trapping {
            SHIFT_COUNT_TRAP_GUARD_WIDTH
        } else if operator == StateGuardOperator::ShiftRight {
            WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH
        } else {
            WRAPPING_SHIFT_ZERO_CLAMP_WIDTH
        };
        runtime_binary_operation_width(operator, operation_byte_size) + fix
    } else if is_float {
        runtime_float_binary_operation_width_with_domain(operator, byte_size, domain)
    } else {
        // Trapping div/mod (idiv traps == Trapping semantics), Exact (proven
        // non-overflowing), and unsigned div/mod (cannot overflow) use the normal
        // op width -- derived from the OPERANDS exactly as the encoder does
        // (`runtime_binary_operation_byte_size`): div/mod/shift run at the
        // operand width, comparisons at the compared width. Pricing them at the
        // STORE's width instead diverges by a byte when e.g. a folded 8-byte
        // divide feeds a `% literal` into a 4-byte ranged slot (cqo idiv = 11,
        // 32-bit = 10).
        runtime_binary_operation_width(
            operator,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )
    };
    // 10 (mov r14,imm64) + left + push r10 (2) + right + mov r11,r10 (3)
    // + pop r10 (2) + operation + store.
    10 + runtime_value_operand_width(runtime_value_operands, left)
        + 2
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3
        + 2
        + operation_width
        + 7.max(store_width(byte_size))
}

/// Bytes of [`append_saturating_signed_divide_modulo`], for the relocation layout.
/// MUST equal the emitter exactly. cmp r11,-1 (4) + jne (2) + the divisor==-1
/// fixup + jmp (2) + the normal idiv core (the plain signed op width).
fn saturating_signed_divide_modulo_width(byte_size: usize, want_remainder: bool) -> usize {
    let fixup = if want_remainder {
        3 // xor r10d, r10d
    } else if byte_size <= 2 {
        16 // neg r10d (3) + mov r9d,imm32 (6) + cmp r10d,r9d (3) + cmovg r10d,r9d (4)
    } else if byte_size <= 4 {
        13 // neg r10d (3) + mov r9d,imm32 (6) + cmovo r10d,r9d (4)
    } else {
        17 // neg r10 (3) + mov r9,imm64 (10) + cmovo r10,r9 (4)
    };
    let normal = runtime_binary_operation_width(
        if want_remainder {
            StateGuardOperator::Modulo
        } else {
            StateGuardOperator::Divide
        },
        byte_size,
    );
    4 + 2 + fixup + 2 + normal
}

/// Bytes of [`append_wrapping_signed_divide_modulo`], for the relocation layout.
/// MUST equal the emitter exactly. cmp r11,-1 (4) + jne (2) + the divisor==-1
/// fixup (always 3: `neg r10` for divide, `xor r10d,r10d` for modulo) + jmp (2) +
/// the normal idiv core.
fn wrapping_signed_divide_modulo_width(byte_size: usize, want_remainder: bool) -> usize {
    let fixup = 3; // neg r10/r10d, or xor r10d,r10d
    let normal = runtime_binary_operation_width(
        if want_remainder {
            StateGuardOperator::Modulo
        } else {
            StateGuardOperator::Divide
        },
        byte_size,
    );
    4 + 2 + fixup + 2 + normal
}

/// The domain-honoring OPERAND-POSITION operation a fused `Binary` operand
/// needs, or `None` for the plain integer path. THE single dispatch shared by
/// the emission arm and its width twin so they can never disagree: Add/Sub and
/// Multiply under Saturating/Trapping clamp/trap; SIGNED div/mod under
/// Saturating take the TYPE_MIN/-1 clamp fixup and under Wrapping the idiv
/// #DE guard (unsigned div/mod use the *Unsigned operators, never overflow,
/// and fall through; Trapping div/mod fall through -- `idiv` traps on
/// overflow and /0, which IS Trapping semantics). Wrapping SHIFTS take the
/// at-width count fix (shift-domain ruling: shifts are value operations --
/// x * 2^n mod 2^w and floor(x / 2^n) -- but the hardware masks the count
/// instead): `<<` and logical `>>` clamp the result to zero, arithmetic `>>`
/// saturates the count to width-1 (= the sign-fill shift).
enum OperandDomainOperation {
    AddSub {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
    Multiply {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
    SaturatingSignedDivMod {
        want_remainder: bool,
    },
    WrappingSignedDivMod {
        want_remainder: bool,
    },
    // Carries the domain: Wrapping masks the COUNT (F8b), while
    // Saturating/Trapping `>>` keep the floor-semantics count fixes (F8c
    // pending) -- one variant, domain-dispatched at emission.
    DomainShift {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
    SaturatingTrappingShiftLeft {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
}

fn operand_position_domain_operation(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
) -> Option<OperandDomainOperation> {
    let (domain, operands_signed) = runtime_value_operands.binary_arithmetic_domain(operand)?;
    match (operator, domain) {
        (
            StateGuardOperator::Add | StateGuardOperator::Subtract,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::AddSub {
            domain,
            operands_signed,
        }),
        (
            StateGuardOperator::Multiply,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::Multiply {
            domain,
            operands_signed,
        }),
        (StateGuardOperator::Divide | StateGuardOperator::Modulo, ArithmeticDomain::Saturating)
            if operands_signed =>
        {
            Some(OperandDomainOperation::SaturatingSignedDivMod {
                want_remainder: operator == StateGuardOperator::Modulo,
            })
        }
        (StateGuardOperator::Divide | StateGuardOperator::Modulo, ArithmeticDomain::Wrapping)
            if operands_signed =>
        {
            Some(OperandDomainOperation::WrappingSignedDivMod {
                want_remainder: operator == StateGuardOperator::Modulo,
            })
        }
        (StateGuardOperator::ShiftLeft, ArithmeticDomain::Wrapping) => {
            Some(OperandDomainOperation::DomainShift {
                domain,
                operands_signed,
            })
        }
        (
            StateGuardOperator::ShiftLeft,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::SaturatingTrappingShiftLeft {
            domain,
            operands_signed,
        }),
        // `>>` cannot overflow: Wrapping masks the count (F8b); the
        // floor-semantics count fix survives under Saturating/Trapping
        // until F8c. Both dispatch on the carried domain at emission.
        (
            StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical,
            ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::DomainShift {
            domain,
            operands_signed,
        }),
        _ => None,
    }
}

/// Bytes of [`append_width_integer_add_sub`]: 4 for 16-bit (0x66 prefix), else 3.
fn width_integer_add_sub_width(byte_size: usize) -> usize {
    if byte_size == 2 { 4 } else { 3 }
}

/// Width of the in-register operation step, dispatching to the SSE float op when
/// the write is floating-point.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    ));
    // Hold the target base in r14, not r15: evaluating the operands below
    // reloads r15 with each source base, which would otherwise clobber the
    // target pointer before the store. r14 is untouched by operand evaluation.
    // `mov r14, imm64` and `mov r15, imm64` are both 10 bytes with the relocated
    // immediate at +2, so the target relocation offset is unchanged.
    append_mov_r14_imm64(&mut bytes, 0);
    append_binary_operands_op_and_store(
        runtime_value_operands,
        &mut bytes,
        target_offset,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    )?;
    Ok(bytes)
}

/// The target-address-AGNOSTIC half of every binary write: evaluate the
/// operand pair (r10 accumulator, left stashed across the right eval),
/// apply the operator under the arithmetic domain (floats, Saturating/
/// Trapping, shift-count policies), and store r10 to [r14 + target_offset].
/// The caller owns getting the target address into r14 (the retired
/// encoders' `mov r14,imm64`; the place materializer's walk + `mov r14,r15`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_binary_operands_op_and_store(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    // Each operand's evaluation accumulates in r10, so the right operand would
    // clobber the left result. Stash left on the stack across the right eval.
    append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, left)?;
    append_push_r10(bytes);
    append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, right)?;
    append_mov_reg_reg(bytes, Reg64::R11, Reg64::R10); // right -> r11
    append_pop_r10(bytes); // restore left -> r10
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    if is_float {
        // Comparisons run at the OPERAND width (a bool target is 1 byte, but
        // the xmm moves + ucomis need the f32/f64 width); arithmetic keeps the
        // target width, which equals the operand width for float targets.
        append_runtime_float_binary_operation(
            bytes,
            operator,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
            domain,
        )?;
    } else if saturating_or_trapping && operator == StateGuardOperator::Multiply {
        // Saturating/Trapping multiply: a 64-bit `imul` yields the EXACT product
        // for <=32-bit operands (it cannot exceed 64 bits), so compare the full
        // product against the target type's range and clamp / trap.
        append_saturating_trapping_multiply(
            bytes,
            domain,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )?;
    } else if saturating_or_trapping && operator == StateGuardOperator::ShiftLeft {
        // Saturating/Trapping `<<`: clamp/trap when the TRUE value x * 2^n
        // leaves the target range (shift slice C; mirrors aarch64).
        append_saturating_trapping_shift_left(bytes, domain, byte_size, target_signed)?;
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add | StateGuardOperator::Subtract
        )
    {
        // Decision 17: narrow targets wide-compute + range-tail (immune to
        // wide literal operands -- the MIN-idiom fix); 64-bit keeps the
        // flag-driven clamp inside the helper.
        append_saturating_trapping_add_sub(
            bytes,
            domain,
            operator,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )?;
    } else if domain == ArithmeticDomain::Saturating
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Saturating SIGNED divide/modulo: clamp the one overflowing corner
        // (TYPE_MIN / -1) to TYPE_MAX / 0 instead of trapping. The UNSIGNED variants
        // cannot overflow, so they are absent from this arm and fall through to the
        // normal path below. (Trapping div/mod also falls through, where `idiv`
        // traps on overflow and divide-by-zero -- exactly Trapping semantics.)
        append_saturating_signed_divide_modulo(
            bytes,
            byte_size,
            operator == StateGuardOperator::Modulo,
        )?;
    } else if domain == ArithmeticDomain::Wrapping
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Wrapping SIGNED divide/modulo: guard TYPE_MIN / -1 so the bare `idiv`
        // does not raise #DE -- produce the WRAPPED result (TYPE_MIN / 0) instead.
        // Unsigned div/mod uses the *Unsigned operators (cannot overflow) and
        // falls through to the normal path below.
        append_wrapping_signed_divide_modulo(
            bytes,
            byte_size,
            operator == StateGuardOperator::Modulo,
        )?;
    } else if (domain == ArithmeticDomain::Wrapping && operator == StateGuardOperator::ShiftLeft)
        || (domain != ArithmeticDomain::Exact
            && matches!(
                operator,
                StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical
            ))
    {
        // Domain-governed shifts: F8b (ch5 shift-count ruling) WRAPPING masks
        // the COUNT to the operand width -- the hardware `shl`/`shr`/`sar`
        // mask mod 32/64 already (the ruling at widths 4/8), sub-word widths
        // take the explicit AND. Saturating/Trapping `>>` keep the floor
        // fixes (arithmetic >> saturates the count to width-1 BEFORE the
        // sar; logical >> zero-clamps after) until F8c lands the count trap.
        // Matches interp + aarch64. The store's truncation remains the
        // in-range wrap.
        let operation_byte_size = runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        );
        if domain == ArithmeticDomain::Wrapping {
            append_wrapping_shift_count_mask(bytes, operation_byte_size);
            append_runtime_binary_operation(bytes, operator, operation_byte_size)?;
        } else if domain == ArithmeticDomain::Trapping {
            // F8c: an out-of-range count traps before the shift, value-blind.
            append_shift_count_trap_guard(bytes, operation_byte_size);
            append_runtime_binary_operation(bytes, operator, operation_byte_size)?;
        } else {
            if operator == StateGuardOperator::ShiftRight {
                append_wrapping_shift_right_count_saturate(bytes, operation_byte_size);
            }
            append_runtime_binary_operation(bytes, operator, operation_byte_size)?;
            if operator != StateGuardOperator::ShiftRight {
                append_wrapping_shift_zero_clamp(bytes, operation_byte_size);
            }
        }
    } else {
        append_runtime_binary_operation(
            bytes,
            operator,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )?;
    }
    append_store_r10_to_r14(bytes, target_offset, byte_size)?;
    Ok(())
}

/// Bytes of [`append_saturating_trapping_multiply`], for the relocation layout.
/// MUST equal what that function emits.
fn saturating_trapping_multiply_width(
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> usize {
    let imul = 4; // imul r10, r11
    if byte_size == 8 {
        // The 128-bit one-operand multiply sequences (see the emission's
        // byte_size == 8 arms). MUST equal them exactly.
        return match (domain, target_signed) {
            // mov+mov+imul+mov (12) + sar (4) + xor (3) + movabs (10)
            // + test (3) + mov (3) + not (3) + cmovns (4) + cmp (3) + cmovne (4)
            (ArithmeticDomain::Saturating, true) => 49,
            // mov+mul+mov (9) + movabs (10) + test (3) + cmovne (4)
            (ArithmeticDomain::Saturating, false) => 26,
            // mov+imul+mov (9) + sar (4) + cmp (3) + je (2) + ud2 (2)
            (ArithmeticDomain::Trapping, true) => 20,
            // mov+mul+mov (9) + test (3) + jz (2) + ud2 (2)
            (ArithmeticDomain::Trapping, false) => 16,
            _ => 0,
        };
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return imul; // emission errors; width is irrelevant then
    }
    // One sign-extension per SIGNED NON-IMMEDIATE operand (see emission):
    // movsx is 4 bytes (8/16-bit), movsxd is 3 (32-bit).
    let extend_one = |skip: bool| {
        if !target_signed || skip {
            0
        } else if byte_size == 4 {
            3
        } else {
            4
        }
    };
    let sign_extend = extend_one(left_is_wide_immediate) + extend_one(right_is_wide_immediate);
    imul + sign_extend + narrow_range_clamp_or_trap_width(domain, target_signed)
}

/// Saturating/Trapping multiply (decision 17). A 64-bit `imul r10, r11` produces
/// the EXACT product for <=32-bit operands (the product cannot exceed 64 bits),
/// so the full result is range-compared against the target type and clamped
/// (Saturating) or trapped (Trapping). 64-bit targets are not handled (the
/// product can exceed 64 bits -- needs the 128-bit `mul`/`imul` form). r11 (the
/// spent right operand) is the clamp-constant scratch.
fn append_saturating_trapping_multiply(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> Result<(), Diagnostic> {
    if byte_size == 8 {
        // 64-bit multiply overflow: the 128-bit one-operand forms make the
        // HIGH half the witness (RDX:RAX = RAX * r11), mirroring the aarch64
        // SMULH/UMULH arms. Signed overflow iff RDX != RAX>>63 (the low
        // half's sign broadcast); unsigned iff RDX != 0. r11 (the untouched
        // right operand), rax/rdx (clobbered by the multiply anyway), and
        // r9/r15 (free after operand evaluation) are the scratch. Branchless
        // (cmov), so the width is a constant per (domain, signedness).
        match (domain, target_signed) {
            (ArithmeticDomain::Saturating, true) => {
                // Boundary = MIN if the TRUE product sign (left^right) is
                // negative, else MAX = NOT(MIN); select it on overflow.
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x4d, 0x89, 0xd1]); // mov r9, r10  (save left)
                bytes.extend([0x49, 0xf7, 0xeb]); // imul r11    (rdx:rax)
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax (low)
                bytes.extend([0x48, 0xc1, 0xf8, 0x3f]); // sar rax, 63 (broadcast)
                bytes.extend([0x4d, 0x31, 0xd9]); // xor r9, r11 (true-sign witness)
                bytes.push(0x49);
                bytes.push(0xbf);
                bytes.extend((i64::MIN as u64).to_le_bytes()); // mov r15, MIN
                bytes.extend([0x4d, 0x85, 0xc9]); // test r9, r9
                bytes.extend([0x4d, 0x89, 0xf9]); // mov r9, r15 (MIN)
                bytes.extend([0x49, 0xf7, 0xd7]); // not r15     (MAX)
                bytes.extend([0x4d, 0x0f, 0x49, 0xcf]); // cmovns r9, r15 (positive -> MAX)
                bytes.extend([0x48, 0x39, 0xc2]); // cmp rdx, rax (high vs broadcast)
                bytes.extend([0x4d, 0x0f, 0x45, 0xd1]); // cmovne r10, r9
            }
            (ArithmeticDomain::Saturating, false) => {
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x49, 0xf7, 0xe3]); // mul r11     (rdx:rax)
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax
                bytes.push(0x49);
                bytes.push(0xbf);
                bytes.extend(u64::MAX.to_le_bytes()); // mov r15, u64::MAX
                bytes.extend([0x48, 0x85, 0xd2]); // test rdx, rdx
                bytes.extend([0x4d, 0x0f, 0x45, 0xd7]); // cmovne r10, r15
            }
            (ArithmeticDomain::Trapping, true) => {
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x49, 0xf7, 0xeb]); // imul r11
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax
                bytes.extend([0x48, 0xc1, 0xf8, 0x3f]); // sar rax, 63
                bytes.extend([0x48, 0x39, 0xc2]); // cmp rdx, rax
                bytes.extend([0x74, 0x02]); // je +2 (skip the trap)
                bytes.extend([0x0f, 0x0b]); // ud2
            }
            (ArithmeticDomain::Trapping, false) => {
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x49, 0xf7, 0xe3]); // mul r11
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax
                bytes.extend([0x48, 0x85, 0xd2]); // test rdx, rdx
                bytes.extend([0x74, 0x02]); // jz +2 (skip the trap)
                bytes.extend([0x0f, 0x0b]); // ud2
            }
            _ => unreachable!("only Saturating/Trapping reach this helper"),
        }
        return Ok(());
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping multiply cannot handle {byte_size}-byte targets yet"
        )));
    }
    // The 64-bit `imul` needs full-width-correct operands. Narrow STORAGE
    // operands are loaded ZERO-extended, so a SIGNED negative value (e.g. i8
    // -50 -> 0xCE = 206) would multiply wrong: sign-extend them from the
    // target width. IMMEDIATE operands are already their true wide value --
    // re-extending one corrupts wide literals (the MIN-idiom fix), so each
    // side skips when immediate.
    if target_signed {
        if !left_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xd2][..], // movsx r10, r10b
                2 => &[0x4d, 0x0f, 0xbf, 0xd2][..], // movsx r10, r10w
                _ => &[0x4d, 0x63, 0xd2][..],       // movsxd r10, r10d
            });
        }
        if !right_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xdb][..], // movsx r11, r11b
                2 => &[0x4d, 0x0f, 0xbf, 0xdb][..], // movsx r11, r11w
                _ => &[0x4d, 0x63, 0xdb][..],       // movsxd r11, r11d
            });
        }
    }
    bytes.extend([0x4d, 0x0f, 0xaf, 0xd3]); // imul r10, r11 (64-bit)
    append_narrow_range_clamp_or_trap(
        bytes,
        domain,
        StateGuardOperator::Multiply,
        byte_size,
        target_signed,
    );
    Ok(())
}

/// Saturating/Trapping ADD/SUB (decision 17). 64-bit targets keep the
/// FLAG-driven clamp (adds/subs carry/overflow at the full width is the only
/// exact witness there). Narrow targets wide-compute like multiply/shl:
/// sign-extend SIGNED NON-IMMEDIATE operands (an immediate is already its
/// true wide value -- re-extending it from the target width corrupts wide
/// literals, the MIN-idiom fix), one exact 64-bit add/sub, then the shared
/// range tail. This replaces the old width-correct-flags narrow path, which
/// could not hold a wide immediate at all (r11's low byte of 128 is -128).
fn append_saturating_trapping_add_sub(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> Result<(), Diagnostic> {
    if byte_size == 8 {
        append_width_integer_add_sub(bytes, operator, 8)?;
        append_arithmetic_domain_clamp(bytes, domain, operator, 8, target_signed)?;
        return Ok(());
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping add/sub cannot handle {byte_size}-byte targets yet"
        )));
    }
    if target_signed {
        if !left_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xd2][..], // movsx r10, r10b
                2 => &[0x4d, 0x0f, 0xbf, 0xd2][..], // movsx r10, r10w
                _ => &[0x4d, 0x63, 0xd2][..],       // movsxd r10, r10d
            });
        }
        if !right_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xdb][..], // movsx r11, r11b
                2 => &[0x4d, 0x0f, 0xbf, 0xdb][..], // movsx r11, r11w
                _ => &[0x4d, 0x63, 0xdb][..],       // movsxd r11, r11d
            });
        }
    }
    append_runtime_binary_operation(bytes, operator, 8)?; // exact 64-bit add/sub
    append_narrow_range_clamp_or_trap(bytes, domain, operator, byte_size, target_signed);
    Ok(())
}

/// Bytes of [`append_saturating_trapping_add_sub`]. MUST stay in lockstep.
fn saturating_trapping_add_sub_width(
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> usize {
    if byte_size == 8 {
        return width_integer_add_sub_width(8)
            + arithmetic_domain_clamp_width(domain, operator, 8, target_signed);
    }
    let extend_one = |skip: bool| {
        if !target_signed || skip {
            0
        } else if byte_size == 4 {
            3
        } else {
            4
        }
    };
    extend_one(left_is_wide_immediate)
        + extend_one(right_is_wide_immediate)
        + 3 // 64-bit add/sub
        + narrow_range_clamp_or_trap_width(domain, target_signed)
}

/// The narrow (<= 4-byte) exact-wide-value range tail shared by the
/// saturating/trapping MULTIPLY and SHIFT-LEFT: the 64-bit op computed the
/// exact result in r10, so compare it against the target range and clamp
/// (cmov) or trap (ud2). r11 is spent by then and serves as the bound
/// scratch. The unsigned arms take a SINGLE UNSIGNED upper compare -- a u32
/// product or shift can exceed 2^63 (signed reading negative), and unsigned
/// results cannot go below zero.
fn append_narrow_range_clamp_or_trap(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
) {
    let unsigned_max: u64 = (1u64 << (8 * byte_size)) - 1;
    let signed_min = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
    let signed_max = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
    fn mov_r11(bytes: &mut Vec<u8>, value: u64) {
        bytes.push(0x49);
        bytes.push(0xbb);
        bytes.extend(value.to_le_bytes());
    }
    match (domain, target_signed) {
        (ArithmeticDomain::Saturating, false) => {
            // Unsigned wide results overflow in ONE direction per operator
            // (the aarch64 tail's rule): subtract only DOWNWARD -- the
            // wrapped wide underflow reads signed-negative, so clamp to 0
            // with a SIGNED compare -- add/mul/shl only UPWARD, where the
            // compare must be UNSIGNED (a 2^63+ product reads negative).
            if operator == StateGuardOperator::Subtract {
                mov_r11(bytes, 0);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x4d, 0x0f, 0x4c, 0xd3]); // cmovl r10, r11 (<s 0 -> 0)
            } else {
                mov_r11(bytes, unsigned_max);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x4d, 0x0f, 0x47, 0xd3]); // cmova r10, r11 (r10 >u max -> max)
            }
        }
        (ArithmeticDomain::Saturating, true) => {
            mov_r11(bytes, signed_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x4d, 0x0f, 0x4f, 0xd3]); // cmovg r10, r11 (> imax -> imax)
            mov_r11(bytes, signed_min);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x4d, 0x0f, 0x4c, 0xd3]); // cmovl r10, r11 (< imin -> imin)
        }
        (ArithmeticDomain::Trapping, false) => {
            if operator == StateGuardOperator::Subtract {
                mov_r11(bytes, 0);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x7d, 0x02]); // jge +2 (>=s 0: ok)
                bytes.extend([0x0f, 0x0b]); // ud2
            } else {
                mov_r11(bytes, unsigned_max);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x76, 0x02]); // jbe +2 (<= max: ok)
                bytes.extend([0x0f, 0x0b]); // ud2
            }
        }
        (ArithmeticDomain::Trapping, true) => {
            mov_r11(bytes, signed_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x7f, 0x0f]); // jg +15 -> ud2 (skip mov+cmp+jge)
            mov_r11(bytes, signed_min);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x7d, 0x02]); // jge +2 (>= imin: ok)
            bytes.extend([0x0f, 0x0b]); // ud2
        }
        _ => {}
    }
}

/// Bytes of [`append_narrow_range_clamp_or_trap`]. MUST stay in lockstep.
/// (The unsigned direction split is width-neutral: one bound either way --
/// mov 10 + cmp 3 + cmov/jcc+ud2 4 -- so no operator parameter here.)
fn narrow_range_clamp_or_trap_width(domain: ArithmeticDomain, target_signed: bool) -> usize {
    match (domain, target_signed) {
        // mov r11,imm64 (10) + cmp (3) + cmova (4)
        (ArithmeticDomain::Saturating, false) => 17,
        // (mov + cmp + cmovg) + (mov + cmp + cmovl)
        (ArithmeticDomain::Saturating, true) => 34,
        // mov (10) + cmp (3) + jbe (2) + ud2 (2)
        (ArithmeticDomain::Trapping, false) => 17,
        // mov (10) + cmp (3) + jg (2) + mov (10) + cmp (3) + jge (2) + ud2 (2)
        (ArithmeticDomain::Trapping, true) => 32,
        _ => 0,
    }
}

/// Saturating/Trapping `<<` (shift slice C): the TRUE value is x * 2^n, so
/// clamp/trap when it leaves the target range. Narrow widths cap the COUNT
/// at the type width w -- any count >= w overflows every nonzero x, and the
/// cap keeps the 64-bit shl EXACT -- then take the shared range tail; only
/// the VALUE sign-extends (the count reads unsigned, so a negative signed
/// count is huge and caps to w, matching the interpreter). 64-bit uses the
/// RECOVERY witness (y >> n == x, arithmetic/logical by signedness) with
/// explicit checks for the two cases the hardware count mask hides: a count
/// >= 64 overflows every nonzero x, and x == 0 never overflows. Mirrors the
/// aarch64 sequences; r9/rax/rcx/r15 are scratch as in the multiply arms.
fn append_saturating_trapping_shift_left(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    // F8c: a TRAPPING out-of-range COUNT traps before the value math (the
    // count is invalid, not the result -- `0 << 40` traps). Saturating
    // cannot reach one post-F8a; its count cap below stays for robustness.
    if domain == ArithmeticDomain::Trapping {
        append_shift_count_trap_guard(bytes, byte_size);
    }
    if byte_size == 8 {
        let fixup: u8 = match (domain, target_signed) {
            // mov r15,MIN (10) + test r9 (3) + mov r10,r15 (3) + not r15 (3)
            // + cmovns r10,r15 (4).
            (ArithmeticDomain::Saturating, true) => 23,
            // mov r10, u64::MAX (10).
            (ArithmeticDomain::Saturating, false) => 10,
            // ud2.
            _ => 2,
        };
        bytes.extend([0x4d, 0x89, 0xd1]); // mov r9, r10 (save x)
        append_runtime_binary_operation(bytes, StateGuardOperator::ShiftLeft, 8)?;
        bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10 (y)
        bytes.extend(if target_signed {
            [0x48, 0xd3, 0xf8] // sar rax, cl (count still in cl)
        } else {
            [0x48, 0xd3, 0xe8] // shr rax, cl
        });
        bytes.extend([0x4c, 0x39, 0xc8]); // cmp rax, r9 (recovery == x ?)
        bytes.extend([0x75, 11]); // jne -> fixup
        bytes.extend([0x49, 0x83, 0xfb, 64]); // cmp r11, 64
        bytes.extend([0x72, 5 + fixup]); // jb -> keep (in-range count)
        bytes.extend([0x4d, 0x85, 0xc9]); // test r9, r9
        bytes.extend([0x74, fixup]); // je -> keep (x == 0)
        match (domain, target_signed) {
            (ArithmeticDomain::Saturating, true) => {
                bytes.push(0x49);
                bytes.push(0xbf);
                bytes.extend((i64::MIN as u64).to_le_bytes()); // mov r15, MIN
                bytes.extend([0x4d, 0x85, 0xc9]); // test r9, r9 (x's sign)
                bytes.extend([0x4d, 0x89, 0xfa]); // mov r10, r15 (MIN)
                bytes.extend([0x49, 0xf7, 0xd7]); // not r15 (MAX)
                bytes.extend([0x4d, 0x0f, 0x49, 0xd7]); // cmovns r10, r15 (x >= 0 -> MAX)
            }
            (ArithmeticDomain::Saturating, false) => {
                bytes.push(0x49);
                bytes.push(0xba);
                bytes.extend(u64::MAX.to_le_bytes()); // mov r10, u64::MAX
            }
            _ => bytes.extend([0x0f, 0x0b]), // ud2
        }
        return Ok(());
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping shift-left cannot handle {byte_size}-byte targets yet"
        )));
    }
    if target_signed {
        match byte_size {
            1 => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]), // movsx r10, r10b
            2 => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]), // movsx r10, r10w
            _ => bytes.extend([0x4d, 0x63, 0xd2]),       // movsxd r10, r10d
        }
    }
    let width_bits = (byte_size * 8) as u8;
    bytes.push(0xb8); // mov eax, imm32 (= w)
    bytes.extend(u32::from(width_bits).to_le_bytes());
    bytes.extend([0x49, 0x83, 0xfb, width_bits]); // cmp r11, w
    bytes.extend([0x4c, 0x0f, 0x43, 0xd8]); // cmovae r11, rax (cap count at w)
    append_runtime_binary_operation(bytes, StateGuardOperator::ShiftLeft, 8)?; // exact 64-bit shl
    append_narrow_range_clamp_or_trap(
        bytes,
        domain,
        StateGuardOperator::ShiftLeft,
        byte_size,
        target_signed,
    );
    Ok(())
}

/// Bytes of [`append_saturating_trapping_shift_left`]. MUST stay in lockstep.
fn saturating_trapping_shift_left_width(
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
) -> usize {
    // F8c: Trapping prepends the count trap guard (cmp + jb + ud2 = 8).
    let count_guard = if domain == ArithmeticDomain::Trapping {
        SHIFT_COUNT_TRAP_GUARD_WIDTH
    } else {
        0
    };
    if byte_size == 8 {
        // save (3) + shl op (6) + mov rax (3) + sar/shr (3) + cmp (3)
        // + jne (2) + cmp #64 (4) + jb (2) + test (3) + je (2) = 31 + fixup.
        return count_guard
            + 31
            + match (domain, target_signed) {
                (ArithmeticDomain::Saturating, true) => 23,
                (ArithmeticDomain::Saturating, false) => 10,
                _ => 2,
            };
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return 6; // emission errors; placeholder for the pre-error capacity
    }
    // movsx (4) / movsxd (3) for signed values only, + the count cap
    // (mov eax 5 + cmp 4 + cmovae 4 = 13) + the 64-bit shl (6) + the tail.
    let sign_extend = if target_signed {
        if byte_size == 4 { 3 } else { 4 }
    } else {
        0
    };
    count_guard + sign_extend + 13 + 6 + narrow_range_clamp_or_trap_width(domain, target_signed)
}

/// Width-correct integer `add`/`sub` of `r10 (op)= r11` so the carry/overflow
/// flags reflect the TARGET byte width (the default binary op is always 64-bit
/// and relies on the truncating store). Only `+`/`-` are supported for the
/// saturating/trapping domains today; other operators error.
fn append_width_integer_add_sub(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    // ADD r/m,r = 0x00 (8-bit) / 0x01 (wider); SUB = 0x28 / 0x29. ModRM 0xDA is
    // (r/m = r10, reg = r11); the REX prefix selects the width and extends both.
    let (op8, opw) = match operator {
        StateGuardOperator::Add => (0x00u8, 0x01u8),
        StateGuardOperator::Subtract => (0x28u8, 0x29u8),
        _ => {
            return Err(Diagnostic::error(
                "saturating/trapping arithmetic is only implemented for + and - so far".to_owned(),
            ));
        }
    };
    match byte_size {
        1 => bytes.extend([0x45, op8, 0xda]),
        2 => bytes.extend([0x66, 0x45, opw, 0xda]),
        4 => bytes.extend([0x45, opw, 0xda]),
        8 => bytes.extend([0x4d, opw, 0xda]),
        _ => {
            return Err(Diagnostic::error(format!(
                "saturating/trapping arithmetic cannot handle {byte_size}-byte targets yet"
            )));
        }
    }
    Ok(())
}

/// Bytes of [`append_arithmetic_domain_clamp`], for the relocation layout. MUST
/// equal what that function emits.
fn arithmetic_domain_clamp_width(
    domain: ArithmeticDomain,
    _operator: StateGuardOperator,
    _byte_size: usize,
    target_signed: bool,
) -> usize {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => 0,
        // jno/jnc rel8 (2) + ud2 (2)
        ArithmeticDomain::Trapping => 4,
        ArithmeticDomain::Saturating => {
            if target_signed {
                // mov r11,imm64 (10) + mov r9,imm64 (10) + cmovs r11,r9 (4) + cmovo r10,r11 (4)
                28
            } else {
                // mov r11,imm64 (10) + cmovc r10,r11 (4)
                14
            }
        }
    }
}

/// Clamp (Saturating) or trap (Trapping) the width-correct op's result in r10,
/// reading the flags it set. Unsigned overflow is the carry flag (add: clamp to
/// the unsigned max; sub: clamp to 0); signed overflow is the overflow flag
/// (clamp to the signed min/max, chosen by the result's sign bit). r11 (the
/// spent right operand) and r9 are used as scratch.
fn append_arithmetic_domain_clamp(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {}
        ArithmeticDomain::Trapping => {
            // Skip the 2-byte ud2 when there was NO overflow: unsigned watches the
            // carry flag (jnc/jae), signed watches the overflow flag (jno).
            let skip_when_ok = if target_signed { 0x71u8 } else { 0x73u8 };
            bytes.extend([skip_when_ok, 0x02, 0x0f, 0x0b]);
        }
        ArithmeticDomain::Saturating if target_signed => {
            let imin = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
            let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
            bytes.push(0x49);
            bytes.push(0xbb);
            bytes.extend(imin.to_le_bytes()); // mov r11, IMIN
            bytes.push(0x49);
            bytes.push(0xb9);
            bytes.extend(imax.to_le_bytes()); // mov r9, IMAX
            // On signed overflow the stored result's sign is inverted, so a
            // negative result means the true value overflowed POSITIVE -> IMAX.
            bytes.extend([0x4d, 0x0f, 0x48, 0xd9]); // cmovs r11, r9
            bytes.extend([0x4d, 0x0f, 0x40, 0xd3]); // cmovo r10, r11
        }
        ArithmeticDomain::Saturating => {
            let clamp_value: u64 = match operator {
                StateGuardOperator::Add => {
                    if byte_size >= 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * byte_size)) - 1
                    }
                }
                StateGuardOperator::Subtract => 0,
                _ => {
                    return Err(Diagnostic::error(
                        "saturating arithmetic is only implemented for + and - so far".to_owned(),
                    ));
                }
            };
            bytes.push(0x49);
            bytes.push(0xbb);
            bytes.extend(clamp_value.to_le_bytes()); // mov r11, clamp
            bytes.extend([0x4d, 0x0f, 0x42, 0xd3]); // cmovc r10, r11
        }
    }
    Ok(())
}

/// Bytes of the in-register conversion step for a numeric `as` cast (the source
/// bits are already in r10; the result is left in r10 for the store).
fn runtime_convert_operation_width(
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> usize {
    match (source_is_float, target_is_float) {
        // movq/movd xmm0,r10 (5), then either the bare conversion (Exact) or
        // an x86 policy fixup around cvttsd2si/cvttss2si. Unlike aarch64,
        // x86 returns one ambiguous integer-indefinite value for NaN and every
        // overflow, so Saturating and Trapping must classify the FP value.
        (true, false) => {
            5 + if trapping {
                float_to_int_trap_width(source_byte_size, target_byte_size, target_signed)
            } else if saturating {
                float_to_int_saturating_width(source_byte_size, target_byte_size, target_signed)
            } else {
                float_to_int_convert_width(source_byte_size, target_byte_size, target_signed)
            }
        }
        (false, true) => int_to_float_conversion_width(source_byte_size, source_signed),
        (true, true) => {
            if source_byte_size == target_byte_size {
                0 // f64->f64: bits already in r10
            } else {
                14 // movq/movd (5) + cvtsd2ss/cvtss2sd (4) + movd/movq (5)
            }
        }
        (false, false) => {
            // Widen a narrow integer source into r10. A 1/2-byte source was loaded
            // with movb/movw, which leave the upper bits GARBAGE, so it MUST be
            // movzx/movsx-extended (zero for unsigned, sign for signed). A 4-byte
            // source was loaded with movl (already zero-extended), so only a SIGNED
            // 4-byte source needs movsxd; an unsigned 4-byte source is already
            // correct. Narrowing/equal widths need nothing (the store truncates).
            if target_byte_size > source_byte_size {
                match source_byte_size {
                    1 | 2 => 4,              // movzx/movsx r10, r10b / r10w
                    4 if source_signed => 3, // movsxd r10, r10d
                    _ => 0,
                }
            } else {
                0
            }
        }
    }
}

/// Append the in-register conversion (see [`runtime_convert_operation_width`]).
pub(crate) fn append_runtime_convert_operation(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) {
    match (source_is_float, target_is_float) {
        (true, false) => {
            // float -> int: move bits into xmm0, truncating-convert to r10.
            if source_byte_size > 4 {
                bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
            }
            if trapping {
                append_float_to_int_trap(bytes, source_byte_size, target_byte_size, target_signed);
            } else if saturating {
                append_float_to_int_saturating(
                    bytes,
                    source_byte_size,
                    target_byte_size,
                    target_signed,
                );
            } else {
                append_float_to_int_convert(
                    bytes,
                    source_byte_size,
                    target_byte_size,
                    target_signed,
                );
            }
        }
        (false, true) => {
            append_int_to_float_conversion(
                bytes,
                source_byte_size,
                target_byte_size,
                source_signed,
            );
        }
        (true, true) => {
            if source_byte_size == target_byte_size {
                // f64 -> f64: nothing to do.
            } else if source_byte_size > target_byte_size {
                bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
                bytes.extend([0xf2, 0x0f, 0x5a, 0xc0]); // cvtsd2ss xmm0, xmm0
                bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
                bytes.extend([0xf3, 0x0f, 0x5a, 0xc0]); // cvtss2sd xmm0, xmm0
                bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
            }
        }
        (false, false) => {
            if target_byte_size > source_byte_size {
                match (source_byte_size, source_signed) {
                    // movb/movw left the upper bits garbage: extend r10b/r10w -> r10.
                    (1, true) => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]), // movsx r10, r10b
                    (1, false) => bytes.extend([0x4d, 0x0f, 0xb6, 0xd2]), // movzx r10, r10b
                    (2, true) => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]), // movsx r10, r10w
                    (2, false) => bytes.extend([0x4d, 0x0f, 0xb7, 0xd2]), // movzx r10, r10w
                    (4, true) => bytes.extend([0x4d, 0x63, 0xd2]),       // movsxd r10, r10d
                    // 4-byte unsigned (and 8-byte) sources were already zero-extended
                    // by the movl/movq load.
                    _ => {}
                }
            }
        }
    }
}

fn int_to_float_conversion_width(source_byte_size: usize, source_signed: bool) -> usize {
    if source_byte_size == 8 && !source_signed {
        // test + jns + unsigned slow path + jump + signed fast path + result move.
        39
    } else {
        let extension = match (source_byte_size, source_signed) {
            (1 | 2, _) => 4,
            (4, true) => 3,
            _ => 0,
        };
        extension + 5 + 5
    }
}

fn append_int_to_float_conversion(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    source_signed: bool,
) {
    let append_signed_conversion = |bytes: &mut Vec<u8>| {
        if target_byte_size > 4 {
            bytes.extend([0xf2, 0x49, 0x0f, 0x2a, 0xc2]); // cvtsi2sd xmm0, r10
        } else {
            bytes.extend([0xf3, 0x49, 0x0f, 0x2a, 0xc2]); // cvtsi2ss xmm0, r10
        }
    };

    if source_byte_size == 8 && !source_signed {
        // SSE2 has no u64->float instruction. Values below 2^63 use the signed
        // conversion directly. For the upper half, convert
        // ((value >> 1) | (value & 1)) and double the result; the sticky low
        // bit preserves nearest-even rounding.
        bytes.extend([0x4d, 0x85, 0xd2]); // test r10, r10
        bytes.extend([0x79, 0x18]); // jns fast_signed
        bytes.extend([0x4d, 0x89, 0xd3]); // mov r11, r10
        bytes.extend([0x49, 0xd1, 0xea]); // shr r10, 1
        bytes.extend([0x49, 0x83, 0xe3, 0x01]); // and r11, 1
        bytes.extend([0x4d, 0x09, 0xda]); // or r10, r11
        append_signed_conversion(bytes);
        if target_byte_size > 4 {
            bytes.extend([0xf2, 0x0f, 0x58, 0xc0]); // addsd xmm0, xmm0
        } else {
            bytes.extend([0xf3, 0x0f, 0x58, 0xc0]); // addss xmm0, xmm0
        }
        bytes.extend([0xeb, 0x05]); // jmp converted
        append_signed_conversion(bytes);
    } else {
        match (source_byte_size, source_signed) {
            (1, true) => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]), // movsx r10, r10b
            (1, false) => bytes.extend([0x4d, 0x0f, 0xb6, 0xd2]), // movzx r10, r10b
            (2, true) => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]), // movsx r10, r10w
            (2, false) => bytes.extend([0x4d, 0x0f, 0xb7, 0xd2]), // movzx r10, r10w
            (4, true) => bytes.extend([0x4d, 0x63, 0xd2]),       // movsxd r10, r10d
            _ => {}
        }
        append_signed_conversion(bytes);
    }

    if target_byte_size > 4 {
        bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
    }
}

fn float_compare_xmm0_width(source_byte_size: usize) -> usize {
    if source_byte_size > 4 { 4 } else { 3 }
}

fn append_float_compare_xmm0(bytes: &mut Vec<u8>, source_byte_size: usize, rhs: u8) {
    let modrm = 0xc0 | rhs;
    if source_byte_size > 4 {
        bytes.extend([0x66, 0x0f, 0x2e, modrm]); // ucomisd xmm0, xmm{rhs}
    } else {
        bytes.extend([0x0f, 0x2e, modrm]); // ucomiss xmm0, xmm{rhs}
    }
}

fn append_float_bound_xmm1(bytes: &mut Vec<u8>, source_byte_size: usize, bits: u64) {
    append_mov_reg_imm64(bytes, Reg64::R11, bits);
    if source_byte_size > 4 {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
    }
}

fn append_signed_float_to_int_convert(bytes: &mut Vec<u8>, source_byte_size: usize) {
    if source_byte_size > 4 {
        bytes.extend([0xf2, 0x4c, 0x0f, 0x2c, 0xd0]); // cvttsd2si r10, xmm0
    } else {
        bytes.extend([0xf3, 0x4c, 0x0f, 0x2c, 0xd0]); // cvttss2si r10, xmm0
    }
}

/// Bounds for truncating a source float into a signed target. `upper` is
/// exclusive. `lower` is either the exclusive `MIN - 1` threshold (when the
/// source format can represent it) or the inclusive `MIN` threshold.
fn float_to_int_bounds(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> (u64, u64, bool) {
    let target_bits = (target_byte_size * 8) as i32;
    let upper = 2.0_f64.powi(target_bits - i32::from(target_signed));
    if !target_signed {
        return if source_byte_size > 4 {
            (upper.to_bits(), (-1.0_f64).to_bits(), false)
        } else {
            (
                u64::from((upper as f32).to_bits()),
                u64::from((-1.0_f32).to_bits()),
                false,
            )
        };
    }
    let minimum = -upper;
    if source_byte_size > 4 {
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
        let minimum = minimum as f32;
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
    }
}

fn integer_clamps(target_byte_size: usize, target_signed: bool) -> (u64, u64) {
    let bits = target_byte_size * 8;
    if target_signed {
        let sign_bit = 1_u64 << (bits - 1);
        (sign_bit - 1, sign_bit)
    } else {
        (
            if bits == 64 {
                u64::MAX
            } else {
                (1_u64 << bits) - 1
            },
            0,
        )
    }
}

fn float_to_int_convert_width(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> usize {
    if target_signed || target_byte_size < 8 {
        5
    } else {
        // 2^63 materialization + compare + branch + subtract + two cvtt arms
        // + bts sign-bit reconstruction + two jumps.
        38 + float_compare_xmm0_width(source_byte_size)
    }
}

fn append_float_to_int_convert(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) {
    if target_signed || target_byte_size < 8 {
        append_signed_float_to_int_convert(bytes, source_byte_size);
        return;
    }

    let split = if source_byte_size > 4 {
        (9223372036854775808.0_f64).to_bits()
    } else {
        u64::from((9223372036854775808.0_f32).to_bits())
    };
    append_float_bound_xmm1(bytes, source_byte_size, split);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([0x72, 0x10]); // jb low-half
    bytes.extend([
        if source_byte_size > 4 { 0xf2 } else { 0xf3 },
        0x0f,
        0x5c,
        0xc1,
    ]); // subsd/subss xmm0, xmm1
    append_signed_float_to_int_convert(bytes, source_byte_size);
    bytes.extend([0x49, 0x0f, 0xba, 0xea, 0x3f]); // bts r10, 63
    bytes.extend([0xeb, 0x05]); // jmp done
    append_signed_float_to_int_convert(bytes, source_byte_size); // low-half
}

fn float_to_int_trap_width(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> usize {
    // Three compares + two bound materializations + four short branches +
    // cvtt + ud2. The final jump hops over the shared trap site.
    3 * float_compare_xmm0_width(source_byte_size)
        + 40
        + float_to_int_convert_width(source_byte_size, target_byte_size, target_signed)
}

fn append_float_to_int_trap(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) {
    let compare_width = float_compare_xmm0_width(source_byte_size);
    let (upper, lower, lower_inclusive) =
        float_to_int_bounds(source_byte_size, target_byte_size, target_signed);
    let convert_width =
        float_to_int_convert_width(source_byte_size, target_byte_size, target_signed);

    append_float_compare_xmm0(bytes, source_byte_size, 0);
    bytes.extend([0x7a, (36 + 2 * compare_width + convert_width) as u8]); // jp trap
    append_float_bound_xmm1(bytes, source_byte_size, upper);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([0x73, (19 + compare_width + convert_width) as u8]); // jae trap
    append_float_bound_xmm1(bytes, source_byte_size, lower);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([
        if lower_inclusive { 0x72 } else { 0x76 },
        (convert_width + 2) as u8,
    ]); // jb/jbe trap
    append_float_to_int_convert(bytes, source_byte_size, target_byte_size, target_signed);
    bytes.extend([0xeb, 0x02]); // jmp done
    bytes.extend([0x0f, 0x0b]); // trap: ud2
}

fn float_to_int_saturating_width(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> usize {
    // Three compares + two bound materializations + policy result arms.
    3 * float_compare_xmm0_width(source_byte_size)
        + 65
        + float_to_int_convert_width(source_byte_size, target_byte_size, target_signed)
}

fn append_float_to_int_saturating(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) {
    let compare_width = float_compare_xmm0_width(source_byte_size);
    let (upper, lower, lower_inclusive) =
        float_to_int_bounds(source_byte_size, target_byte_size, target_signed);
    let (maximum, minimum) = integer_clamps(target_byte_size, target_signed);
    let convert_width =
        float_to_int_convert_width(source_byte_size, target_byte_size, target_signed);

    append_float_compare_xmm0(bytes, source_byte_size, 0);
    bytes.extend([0x7a, (36 + 2 * compare_width + convert_width) as u8]); // jp nan
    append_float_bound_xmm1(bytes, source_byte_size, upper);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([0x73, (24 + compare_width + convert_width) as u8]); // jae high
    append_float_bound_xmm1(bytes, source_byte_size, lower);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([
        if lower_inclusive { 0x72 } else { 0x76 },
        (19 + convert_width) as u8,
    ]); // jb/jbe low
    append_float_to_int_convert(bytes, source_byte_size, target_byte_size, target_signed);
    bytes.extend([0xeb, 0x1b]); // jmp done
    bytes.extend([0x45, 0x31, 0xd2]); // nan: xor r10d, r10d
    bytes.extend([0xeb, 0x16]); // jmp done
    append_mov_reg_imm64(bytes, Reg64::R10, maximum); // high
    bytes.extend([0xeb, 0x0a]); // jmp done
    append_mov_reg_imm64(bytes, Reg64::R10, minimum); // low
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_convert_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> usize {
    // mov r14,imm64(target base) (10) + source operand load + convert + store.
    10 + runtime_value_operand_width(runtime_value_operands, source)
        + runtime_convert_operation_width(
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        )
        + store_width(target_byte_size)
}

/// `target = source as T`: hold the target base in r14 (untouched by operand
/// evaluation, which reloads r15), evaluate the source operand into r10, convert
/// it in place between integer/float representations, and store the result.
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
    append_mov_r14_imm64(&mut bytes, 0); // target base (imm64 @ +2 relocated)
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, source)?;
    append_runtime_convert_operation(
        &mut bytes,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    );
    append_store_r10_to_r14(&mut bytes, target_offset, target_byte_size)?;
    Ok(bytes)
}

/// Address-computation prefix before the value operands in a pointee binary
/// write -- CANONICALIZED by the place materializer (Binary rung 1b):
/// `mov r15,imm64(frame)` (10) + `mov r15,[r15+ptr]` (7) + `mov r14,r15` (3)
/// -- r14 then holds the dereferenced runtime pointer (the target base)
/// across operand evaluation, exactly as before.
pub fn runtime_pointee_binary_operand_start_width() -> usize {
    20
}

pub fn runtime_pointee_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    // 17 (frame base + deref ptr) + left + push r10 (2) + right + mov r11,r10 (3)
    // + pop r10 (2) + operation + store.
    runtime_pointee_binary_operand_start_width()
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3
        + 2
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

/// `*(frame[pointer_byte_offset]) + field_byte_offset = left OP right`, where the
/// operands resolve against the runtime frame. The dereferenced target pointer is
/// held in r14 (untouched by operand evaluation, which reloads r15/r10/r11).
pub fn encode_runtime_pointee_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    // Binary delegations (rung 1b): the place walk (mov r15,imm64; deref)
    // + the r14 hop -- the operand-start prefix grows 17 -> 20 and the
    // offset fn moves in lockstep. Exact-domain tail preserved via the
    // shared helper (Exact never enters the domain arms).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                pointer_byte_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a pointee place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    Ok(bytes)
}

/// Length of the address-computation prefix that precedes the value operands
/// in a frame-base-indexed binary write -- CANONICALIZED by the place
/// materializer (Binary rung 1b): `mov r15,imm64(frame)` (10) +
/// exact-width `load-zx r11,[r15+idx]` + `imul r11,r11,elem` (7) +
/// `add r15,r11` (3) + `mov r14,r15` (3).
pub fn runtime_frame_base_indexed_binary_left_operand_offset(index_byte_size: usize) -> usize {
    23 + unsigned_load_width(index_byte_size)
}

/// Length of the address-computation prefix that precedes the value operands
/// in a frame-INDEXED (slice-descriptor) binary write -- CANONICALIZED by
/// the place materializer (Binary rung 1b): `mov r15,imm64(frame)` (10) +
/// exact-width `load-zx r11,[r15+idx]` + `imul r11,r11,elem` (7) +
/// `mov r15,[r15+desc]` (7) + `add r15,r11` (3) + `mov r14,r15` (3).
/// The element address ends in r14, which operand evaluation never clobbers.
pub fn runtime_frame_indexed_binary_left_operand_offset(index_byte_size: usize) -> usize {
    30 + unsigned_load_width(index_byte_size)
}

pub fn runtime_frame_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    index_byte_size: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_indexed_binary_left_operand_offset(index_byte_size)
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2 // push r10
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3 // mov r11, r10
        + 2 // pop r10
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

/// `slice[i] = left OP right` through a frame-resident slice DESCRIPTOR with a
/// runtime index: deref the descriptor's data pointer, scale the index, and
/// run the same operand/binary/store tail as the frame-base-indexed binary
/// write (the inline-array twin above).
#[allow(clippy::too_many_arguments)]
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
    // Binary rung 1b: DELEGATES through the place materializer. The
    // selected-width index is zero-extended into r11, the descriptor deref
    // hops r15 in place, and the r14 hop preserves the computed address.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                descriptor_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_indexed_binary_write_width(
            runtime_value_operands,
            index_byte_size,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

pub fn runtime_frame_base_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    index_byte_size: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_base_indexed_binary_left_operand_offset(index_byte_size)
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2 // push r10
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3 // mov r11, r10
        + 2 // pop r10
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

#[allow(clippy::too_many_arguments)]
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
    // Binary rung 1b: DELEGATES through the place materializer. The r14 hop
    // preserves the computed address and the exact-width zero-extending load
    // cannot splice neighboring bytes into a narrow index or truncate a wide
    // index.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-base-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_binary_write_width(
            runtime_value_operands,
            index_byte_size,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

pub fn runtime_machine_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_byte_size: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    // Byte layout matches the frame-base binary write (only the base relocation
    // targets the machine symbol, handled by the relocations crate); a
    // FRAME-resident index inserts a `mov r15,imm64` frame-base load (+10)
    // before the index read.
    let frame_index_extra =
        if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            10
        } else {
            0
        };
    runtime_frame_base_indexed_binary_write_width(
        runtime_value_operands,
        index_byte_size,
        byte_size,
        left,
        operator,
        right,
    ) + frame_index_extra
}

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
    // Binary rung 1b: DELEGATES through the place materializer -- the
    // machine-region-index prefix moves 27 -> 30, the frame-region 37 -> 40
    // (the r14 hop); the frame-index base stays a `mov r11,imm64` at +10
    // (the retired `mov r15,imm64` position -- the walker's frame reloc and
    // +10 operand shift hold as-is), realigning the encoder with the shared
    // frame-base offset fn the walker consumes.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a machine-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_indexed_binary_write_width(
            runtime_value_operands,
            index_region,
            index_byte_size,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

/// Prologue width of the double-indexed binary write (the left operand starts
/// here): mov r14,imm64 (10) [+ mov index-base,imm64 (10) for each frame
/// index] + two exact-width zero-extending index loads + two imuls (7 each)
/// + add r14,r15 (3) + add r14,r11 (3).
pub fn runtime_machine_double_indexed_binary_left_operand_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
) -> usize {
    // Canonicalized by the place materializer (Binary rung 1b): mov r15,
    // imm64 (10) + per-index [cross-region mov+load (17) | same-region
    // load (7)] + imul (7) each + add r15,r11 (3) + add r15,r10 (3) +
    // mov r14,r15 (3). Each frame index adds its OWN base.
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    33 + unsigned_load_width(outer_index_byte_size)
        + unsigned_load_width(inner_index_byte_size)
        + if outer_index_region == frame { 10 } else { 0 }
        + if inner_index_region == frame { 10 } else { 0 }
}

pub fn runtime_machine_double_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_machine_double_indexed_binary_left_operand_offset(
        outer_index_region,
        outer_index_byte_size,
        inner_index_region,
        inner_index_byte_size,
    ) + runtime_value_operand_width(runtime_value_operands, left)
        + 2 // push r10
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3 // mov r11, r10
        + 2 // pop r10
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

/// Binary value into a BOTH-RUNTIME nested target (`grid[i][j] = a OP b`):
/// r14 = base + outer*outer_stride + inner*inner_stride, computed FIRST with
/// BOTH indices loaded before r14 is biased (the r14-before-bias key), then
/// the exact operand-evaluation tail of the single-index sibling -- operand
/// evaluation clobbers r15/r10/r11 but never r14.
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
    for region in [outer_index_region, inner_index_region] {
        if !matches!(
            region,
            omega_target_operations::RuntimeStorageRegion::Machine
                | omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        ) {
            return Err(Diagnostic::error(
                "X86_64 MVP encoder cannot write a double-indexed binary with this index region yet",
            ));
        }
    }
    // Binary rung 1b: DELEGATES through the place materializer -- each
    // frame-resident index materializes its OWN base (r11 outer, r10 inner;
    // the retired layout shared one r10 mov); prefixes move 44 -> 47
    // (both machine), 54 -> 57 (one frame), 54 -> 67 (both frame); the
    // offset fn becomes per-region sums and the walker arm splits per-index
    // relocs, all in the SAME commit (the shared-constant lesson).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: outer_index_region,
                    index_offset: outer_index_offset,
                    index_byte_size: outer_index_byte_size,
                    element_byte_size: outer_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: inner_index_region,
                    index_offset: inner_index_offset,
                    index_byte_size: inner_index_byte_size,
                    element_byte_size: inner_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a double-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_double_indexed_binary_write_width(
            runtime_value_operands,
            outer_index_region,
            outer_index_byte_size,
            inner_index_region,
            inner_index_byte_size,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

/// Width of [`encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage`].
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width()
-> usize {
    // mov r14,imm64(frame) (10) + mov eax,[r14+outer] (7) + mov r11d,[r14+inner] (7)
    // + imul rax,imm32 (7) + imul r11,imm32 (7) + add r14,rax (3) + add r14,r11 (3)
    // + load rax,[r14+base+field] (7) + mov r15,imm64(target) (10) + store [r15+target] (7)
    68
}

/// Target-region relocation start (the `mov r15,imm64` before the store,
/// pre-`+2`) inside the frame-base double-indexed read.
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset() -> usize {
    68 - 17
}

/// Fixed width of a value-position text-equals operand (the `TextEquals` arm
/// of `append_runtime_value_operand`): two relocated descriptor-base imm64
/// movs (10 each) with two 7-byte disp32 descriptor word loads apiece, then a
/// fixed 39-byte length-compare + bounded byte loop block and the 3-byte
/// result mov. MUST stay in lockstep with that encoder (it ends with a
/// `debug_assert_eq!` against this function) and with
/// `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET` below.
pub fn runtime_text_equals_operand_width() -> usize {
    (10 + 7 + 7) + (10 + 7 + 7) + 39 + 3
}

/// Byte offset of the RIGHT descriptor's base `mov r15, imm64` inside a
/// text-equals operand (the relocation planner adds the +2 imm offset itself).
pub const RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET: usize = 10 + 7 + 7;

/// Width of a guard-position text-vs-literal content compare operand (the
/// `TextEqualsLiteral` arm of `append_runtime_value_operand`): the place's
/// descriptor-address setup (13 bytes for a storage base, 17 for a pointee or
/// fixed-indexed deref, 30 for a frame-base-indexed element address, 34 for a
/// frame-indexed element address, each starting with the relocated
/// `mov r15, imm64`), then a fixed 30-byte head (two disp32 descriptor word
/// loads, result zero, length compare + branch), one 13-byte disp32 byte
/// compare + branch per literal byte, and the fixed 9-byte tail (equal-result
/// mov + result move into the destination). MUST stay in lockstep with that
/// encoder (it ends with a `debug_assert_eq!` against this function).
pub fn runtime_text_equals_literal_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    place: RuntimeValueOperandHandle,
    literal: &[u8],
) -> usize {
    let place_setup_width = if runtime_value_operands.storage(place).is_some() {
        // mov r15,imm64 (10) + mov rax,r15 (3)
        13
    } else if runtime_value_operands.pointee(place).is_some() {
        // mov r15,imm64 (10) + mov rax,[r15+ptr_off] (7)
        17
    } else if let Some((_, index_region, _, index_byte_size, _, _, _)) =
        runtime_value_operands.frame_indexed(place)
    {
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7) + index load
        // + imul r11,r11,elem (7) + add rax,r11 (3)
        27 + unsigned_load_width(index_byte_size)
            + usize::from(index_region == RuntimeStorageRegion::Machine) * 10
    } else if let Some((_, _, index_byte_size, _, _, _)) =
        runtime_value_operands.frame_base_indexed(place)
    {
        // mov r15,imm64 (10) + index load + imul r11,r11,elem (7)
        // + mov rax,r15 (3) + add rax,r11 (3)
        23 + unsigned_load_width(index_byte_size)
    } else if runtime_value_operands.frame_fixed_indexed(place).is_some() {
        // Constant element index folds into the descriptor displacement:
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7)
        17
    } else {
        // Selection only builds this operand over storage/pointee/indexed
        // text places; the encoder rejects anything else with a hard
        // diagnostic before this width could be compared against emitted
        // bytes.
        0
    };
    place_setup_width + 30 + 13 * literal.len() + 9
}

pub fn runtime_value_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    if runtime_value_operands.immediate_integer(operand).is_some() {
        10
    } else if let Some((_, _, byte_size)) = runtime_value_operands.storage(operand) {
        10 + load_width(byte_size)
    } else if let Some((_, _, _, fragments)) = runtime_value_operands.bit_field(operand) {
        runtime_bit_field_operand_width(&fragments).unwrap_or(0)
    } else if let Some((_, _, byte_size)) = runtime_value_operands.pointee(operand) {
        // mov r15,imm64 (10) + mov rax,[r15+ptr_off] (7) + load dest,[rax+field].
        // A 16-bit load has the extra 0x66 operand-size prefix.
        17 + load_width(byte_size)
    } else if let Some((_, index_region, _, index_byte_size, _, _, byte_size)) =
        runtime_value_operands.frame_indexed(operand)
    {
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7) + index load
        // + imul r11,r11,elem (7) + add rax,r11 (3) + load dest,[rax+field].
        27 + unsigned_load_width(index_byte_size)
            + usize::from(index_region == RuntimeStorageRegion::Machine) * 10
            + load_width(byte_size)
    } else if let Some((_, _, index_byte_size, _, _, byte_size)) =
        runtime_value_operands.frame_base_indexed(operand)
    {
        // mov r15,imm64 (10) + index load + imul r11,r11,elem (7)
        // + mov rax,r15 (3) + add rax,r11 (3) + load dest,[rax+base+field].
        23 + unsigned_load_width(index_byte_size) + load_width(byte_size)
    } else if let Some((_, _, _, _, byte_size)) =
        runtime_value_operands.frame_fixed_indexed(operand)
    {
        // Constant element index folds into the load displacement, so the shape
        // matches the pointee case: mov r15,imm64 (10) + mov rax,[r15+desc] (7)
        // + load dest,[rax+const].
        17 + load_width(byte_size)
    } else if let Some((_, index_region, _, index_byte_size, _, _, byte_size)) =
        runtime_value_operands.machine_indexed(operand)
    {
        // MUST mirror the machine-indexed emission arm: mov r15,imm64 (10)
        // + mov rax,r15 (3) + [frame index: mov r15,imm64 (10)] + index
        // load (7) + imul (7) + add rax,r11 (3) + element load.
        let frame_base =
            if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                10
            } else {
                0
            };
        10 + 3 + frame_base + unsigned_load_width(index_byte_size) + 7 + 3 + load_width(byte_size)
    } else if runtime_value_operands.text_equals(operand).is_some() {
        runtime_text_equals_operand_width()
    } else if let Some((place, literal, _is_bounded_buffer)) =
        runtime_value_operands.text_equals_literal(operand)
    {
        // Carrier vs descriptor place are byte-width identical, so the width is
        // independent of `is_bounded_buffer`.
        runtime_text_equals_literal_operand_width(runtime_value_operands, place, &literal)
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let operation_width = if runtime_value_operands.binary_is_float(operand) {
            // Float operands: the SSE sequence width is PER-OPERATOR (comparisons
            // materialize 0/1) but f32/f64-identical at each operator. MUST match
            // the emission below or the recorded relocation offsets drift (silent
            // runtime segfault).
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            let domain = runtime_value_operands
                .binary_arithmetic_domain(operand)
                .map(|(domain, _)| domain)
                .unwrap_or(ArithmeticDomain::Exact);
            runtime_float_binary_operation_width_with_domain(operator, byte_width, domain)
        } else if let Some(domain_operation) =
            operand_position_domain_operation(runtime_value_operands, operand, operator)
        {
            // Domain-honoring operand-position arithmetic: MUST mirror the
            // emission arm's dispatch exactly or relocation offsets drift.
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            match domain_operation {
                OperandDomainOperation::AddSub {
                    domain,
                    operands_signed,
                } => saturating_trapping_add_sub_width(
                    domain,
                    operator,
                    byte_width,
                    operands_signed,
                    runtime_value_operands.immediate_integer(left).is_some(),
                    runtime_value_operands.immediate_integer(right).is_some(),
                ),
                OperandDomainOperation::Multiply {
                    domain,
                    operands_signed,
                } => saturating_trapping_multiply_width(
                    domain,
                    byte_width,
                    operands_signed,
                    runtime_value_operands.immediate_integer(left).is_some(),
                    runtime_value_operands.immediate_integer(right).is_some(),
                ),
                OperandDomainOperation::SaturatingSignedDivMod { want_remainder } => {
                    saturating_signed_divide_modulo_width(byte_width, want_remainder)
                }
                OperandDomainOperation::WrappingSignedDivMod { want_remainder } => {
                    wrapping_signed_divide_modulo_width(byte_width, want_remainder)
                }
                OperandDomainOperation::DomainShift { domain, .. } => {
                    let fix = if domain == ArithmeticDomain::Wrapping {
                        wrapping_shift_count_mask_width(byte_width)
                    } else if domain == ArithmeticDomain::Trapping {
                        SHIFT_COUNT_TRAP_GUARD_WIDTH
                    } else if operator == StateGuardOperator::ShiftRight {
                        WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH
                    } else {
                        WRAPPING_SHIFT_ZERO_CLAMP_WIDTH
                    };
                    runtime_binary_operation_width(operator, byte_width)
                        + fix
                        + wrapping_node_width_extension_width(byte_width)
                }
                OperandDomainOperation::SaturatingTrappingShiftLeft {
                    domain,
                    operands_signed,
                } => saturating_trapping_shift_left_width(domain, byte_width, operands_signed),
            }
        } else {
            // Use the SAME byte_size the emission picks (runtime_binary_operation_byte_size):
            // div/mod run at the operand width so a negative i32 dividend is handled
            // correctly, which changes the idiv/div core length -- the width MUST track
            // it or relocation offsets drift (silent segfault). Other ops keep 64-bit.
            // A nested WRAPPING node < 8 bytes appends one truncation move
            // (movzx/movsx: 4 bytes; the width-4 forms: 3) -- MUST stay in
            // lockstep with the emission arm.
            let wrapping_truncation = match (
                runtime_value_operands.binary_arithmetic_domain(operand),
                runtime_value_operands.binary_byte_width(operand),
            ) {
                (Some((psi_numerics::arithmetic::ArithmeticDomain::Wrapping, _)), Some(width))
                    if width < 8 =>
                {
                    wrapping_node_width_extension_width(width)
                }
                _ => 0,
            };
            runtime_binary_operation_width(
                operator,
                runtime_binary_operation_byte_size(
                    runtime_value_operands,
                    operator,
                    left,
                    right,
                    8,
                ),
            ) + wrapping_truncation
        };
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + operation_width
            // push r10 (2) + mov r11,r10 (3) + pop r10 (2) + mov dest,r10 (3)
            + 10
    } else if let Some((source, src_bytes, tgt_bytes, src_float, tgt_float, src_signed)) =
        runtime_value_operands.convert(operand)
    {
        // Load source into r10, convert it in place, then mov dest,r10 (3). MUST
        // match the emission below or relocation offsets drift (runtime segfault).
        runtime_value_operand_width(runtime_value_operands, source)
            + runtime_convert_operation_width(
                src_bytes,
                tgt_bytes,
                src_float,
                tgt_float,
                src_signed,
                runtime_value_operands.convert_target_signed(operand),
                runtime_value_operands.convert_trapping(operand),
                runtime_value_operands.convert_saturating(operand),
            )
            + 3
    } else {
        0
    }
}

pub(crate) fn append_runtime_value_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination: Reg64,
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    if let Some(value) = runtime_value_operands.immediate_integer(operand) {
        append_mov_reg_imm64(bytes, destination, value as u64);
        Ok(())
    } else if let Some((_, byte_offset, byte_size)) = runtime_value_operands.storage(operand) {
        append_mov_r15_imm64(bytes, 0);
        append_load_reg_from_r15(bytes, destination, byte_offset, byte_size)
    } else if let Some((_, base_byte_offset, _, fragments)) =
        runtime_value_operands.bit_field(operand)
    {
        append_runtime_bit_field_operand(bytes, destination, base_byte_offset, &fragments)
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        runtime_value_operands.pointee(operand)
    {
        // r15 = frame base (relocated). rax = the stored pointer; load through it.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, pointer_byte_offset)?;
        append_load_reg_from_rax(bytes, destination, field_byte_offset, byte_size)
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
        // r15 = frame base (relocated). rax = slice data pointer from the descriptor;
        // r11 = index; rax += index*element + ... then load [rax + field].
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        if index_region == RuntimeStorageRegion::Machine {
            append_mov_r15_imm64(bytes, 0);
        }
        append_load_unsigned_reg_from_r15(bytes, Reg64::R11, index_offset, index_byte_size)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(bytes, destination, field_byte_offset, byte_size)
    } else if let Some((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        // r15 = frame base (relocated). The base lives inline in the frame at
        // base_byte_offset; rax = frame base, then add scaled index + base + field.
        append_mov_r15_imm64(bytes, 0);
        append_load_unsigned_reg_from_r15(bytes, Reg64::R11, index_offset, index_byte_size)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_mov_rax_r15(bytes);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(
            bytes,
            destination,
            base_byte_offset + field_byte_offset,
            byte_size,
        )
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_fixed_indexed(operand)
    {
        // Descriptor-based access with a constant element index: r15 = frame base
        // (relocated), rax = the slice data pointer, then load through it at the
        // constant displacement `element_index*element + field`.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        let displacement = element_index
            .checked_mul(element_byte_size)
            .and_then(|scaled| scaled.checked_add(field_byte_offset))
            .ok_or_else(|| {
                Diagnostic::error("X86_64 fixed indexed value operand offset overflow")
            })?;
        append_load_reg_from_rax(bytes, destination, displacement, byte_size)
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
        // MACHINE-owned array element in operand position: machine base
        // (relocated at the operand start) copied into rax as the address
        // accumulator; a FRAME-resident index re-materializes r15 with the
        // frame base at the PINNED offset 13 (mov imm64 10 + mov rax,r15 3;
        // see machine_indexed_operand_frame_index_base_offset). r11 is the
        // index/scale scratch (safe: the binary evaluator stashes the left
        // result on the stack across right-operand evaluation).
        append_mov_r15_imm64(bytes, 0);
        append_mov_rax_r15(bytes);
        let index_from_frame =
            index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
        if index_from_frame {
            append_mov_r15_imm64(bytes, 0);
            append_load_unsigned_reg_from_r15(bytes, Reg64::R11, index_offset, index_byte_size)?;
        } else {
            append_load_unsigned_reg_from_rax(bytes, Reg64::R11, index_offset, index_byte_size)?;
        }
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(
            bytes,
            destination,
            base_byte_offset
                .checked_add(field_byte_offset)
                .ok_or_else(|| Diagnostic::error("machine-indexed operand offset overflow"))?,
            byte_size,
        )?;
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
            destination,
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
            destination,
            place,
            &literal,
            place_is_bounded_buffer,
        )?;
        Ok(())
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        // Every comparison/operation accumulates its result in r10, so evaluating
        // the right operand clobbers the left result. Stash left on the stack
        // across the right evaluation, then combine.
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, left)?;
        append_push_r10(bytes);
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, right)?;
        append_mov_reg_reg(bytes, Reg64::R11, Reg64::R10); // right -> r11
        append_pop_r10(bytes); // restore left -> r10
        if runtime_value_operands.binary_is_float(operand) {
            // Float operands carry their IEEE bits in r10/r11; do the SSE op on the
            // bits (addss/addsd/...) rather than an integer add over them. The width
            // is threaded from build time (set once from the operands' scalar type),
            // so f32 picks `addss`/`movss` (4) and f64 picks `addsd`/`movsd` (8) —
            // no longer hardcoded. The encoded length is identical for both widths
            // at a given policy; the width twin emits the same policy guard to keep
            // relocation offsets in lockstep.
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            let domain = runtime_value_operands
                .binary_arithmetic_domain(operand)
                .map(|(domain, _)| domain)
                .unwrap_or(ArithmeticDomain::Exact);
            append_runtime_float_binary_operation(bytes, operator, byte_width, domain)?;
        } else if let Some(domain_operation) =
            operand_position_domain_operation(runtime_value_operands, operand, operator)
        {
            // Decision 17 in OPERAND position: reuse the binary WRITE path's
            // r10/r11 sequences verbatim. Add/Sub take the width-correct op
            // whose flags reflect the operand width + the flag-driven
            // clamp/trap; Multiply takes the wide multiply + range clamp/trap;
            // signed Saturating div/mod take the TYPE_MIN/-1 fixup; signed
            // Wrapping div/mod take the idiv #DE guard (the byte-width
            // compare truncates the negated value exactly as the write path's
            // store would). The operand's byte_width is its REAL scalar width
            // here (set at construction for non-Exact domains). Upper r10
            // bits may be stale on the non-overflow path, which every
            // consumer tolerates (compares/stores run at width).
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            match domain_operation {
                OperandDomainOperation::AddSub {
                    domain,
                    operands_signed,
                } => {
                    append_saturating_trapping_add_sub(
                        bytes,
                        domain,
                        operator,
                        byte_width,
                        operands_signed,
                        runtime_value_operands.immediate_integer(left).is_some(),
                        runtime_value_operands.immediate_integer(right).is_some(),
                    )?;
                }
                OperandDomainOperation::Multiply {
                    domain,
                    operands_signed,
                } => {
                    append_saturating_trapping_multiply(
                        bytes,
                        domain,
                        byte_width,
                        operands_signed,
                        runtime_value_operands.immediate_integer(left).is_some(),
                        runtime_value_operands.immediate_integer(right).is_some(),
                    )?;
                }
                OperandDomainOperation::SaturatingSignedDivMod { want_remainder } => {
                    append_saturating_signed_divide_modulo(bytes, byte_width, want_remainder)?;
                }
                OperandDomainOperation::WrappingSignedDivMod { want_remainder } => {
                    append_wrapping_signed_divide_modulo(bytes, byte_width, want_remainder)?;
                }
                OperandDomainOperation::DomainShift {
                    domain,
                    operands_signed,
                } => {
                    // Width-correct shift + the domain count fix (F8b:
                    // Wrapping masks the count -- sub-word AND only, the
                    // hardware mask IS the ruling at widths 4/8; Sat/Trap
                    // `>>` keep the floor fixes until F8c) + the node-width
                    // extension the parent contract requires.
                    if domain == ArithmeticDomain::Wrapping {
                        append_wrapping_shift_count_mask(bytes, byte_width);
                        append_runtime_binary_operation(bytes, operator, byte_width)?;
                    } else if domain == ArithmeticDomain::Trapping {
                        // F8c: an out-of-range count traps before the shift.
                        append_shift_count_trap_guard(bytes, byte_width);
                        append_runtime_binary_operation(bytes, operator, byte_width)?;
                    } else {
                        if operator == StateGuardOperator::ShiftRight {
                            append_wrapping_shift_right_count_saturate(bytes, byte_width);
                        }
                        append_runtime_binary_operation(bytes, operator, byte_width)?;
                        if operator != StateGuardOperator::ShiftRight {
                            append_wrapping_shift_zero_clamp(bytes, byte_width);
                        }
                    }
                    append_wrapping_node_width_extension(bytes, byte_width, operands_signed);
                }
                OperandDomainOperation::SaturatingTrappingShiftLeft {
                    domain,
                    operands_signed,
                } => {
                    // The write path's clamp/trap sequence verbatim; the result
                    // is range-correct at the node width (clamped bounds carry
                    // the right extension), so no width extension follows --
                    // the AddSub/Multiply operand contract.
                    append_saturating_trapping_shift_left(
                        bytes,
                        domain,
                        byte_width,
                        operands_signed,
                    )?;
                }
            }
        } else {
            // Comparisons use the operand width; other nested binaries do not carry
            // their result width, so assume 64-bit (matches runtime_value_operand_
            // width above for relocation consistency).
            append_runtime_binary_operation(
                bytes,
                operator,
                runtime_binary_operation_byte_size(
                    runtime_value_operands,
                    operator,
                    left,
                    right,
                    8,
                ),
            )?;
            // A nested WRAPPING binary must hand its PARENT the width-wrapped
            // VALUE in r10: the plain 64-bit op leaves the untruncated result
            // (0u32 - 2 = 0xFFFF_FFFF_FFFF_FFFE), and a sign/width-sensitive
            // parent (>>, /, %, comparisons) then reads it wrong -- the
            // interpreter wraps AT THE NODE (decision 17); the
            // store-truncation-is-the-wrap shortcut only holds at the WRITE.
            // Extension follows the node's own signedness. Width tracked in
            // runtime_value_operand_width -- MUST stay in lockstep (4 bytes,
            // except 3 for the width-4 forms).
            if let Some((psi_numerics::arithmetic::ArithmeticDomain::Wrapping, operands_signed)) =
                runtime_value_operands.binary_arithmetic_domain(operand)
                && let Some(byte_width) = runtime_value_operands.binary_byte_width(operand)
                && byte_width < 8
            {
                append_wrapping_node_width_extension(bytes, byte_width, operands_signed);
            }
        }
        append_mov_reg_reg(bytes, destination, Reg64::R10);
        Ok(())
    } else if let Some((source, src_bytes, tgt_bytes, src_float, tgt_float, src_signed)) =
        runtime_value_operands.convert(operand)
    {
        // Load the cast's source into r10, convert it in place (cvttsd2si /
        // cvtsi2sd / cvtsd2ss / movsxd), then move the result to `destination`.
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, source)?;
        append_runtime_convert_operation(
            bytes,
            src_bytes,
            tgt_bytes,
            src_float,
            tgt_float,
            src_signed,
            runtime_value_operands.convert_target_signed(operand),
            runtime_value_operands.convert_trapping(operand),
            runtime_value_operands.convert_saturating(operand),
        );
        append_mov_reg_reg(bytes, destination, Reg64::R10);
        Ok(())
    } else {
        Err(Diagnostic::error(
            "X86_64 runtime value operand is not implemented yet",
        ))
    }
}

/// Value-position text content equality: `destination = (left == right)` as
/// bool 0/1, where both sides are `{ptr @ +0, len @ +8}` text descriptors at
/// relocated region bases. FIXED-WIDTH (`runtime_text_equals_operand_width`):
/// every descriptor word loads through a disp32 form, keeping the relocation
/// offsets (left base mov at the operand start, right base mov at
/// `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET`) pinned.
///
/// Register use: r15 = descriptor base, then the right length, then the byte
/// scratch in the loop; rax/rcx = left ptr/len, rdx = right ptr, r9 = the
/// bool result (moved into `destination` last). r12/r13/r14 stay untouched
/// (dispatch state and the binary-write shapes' target base live there).
fn append_runtime_text_equals_operand(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    left_offset: usize,
    left_is_bounded_buffer: bool,
    right_offset: usize,
    right_is_bounded_buffer: bool,
) -> Result<(), Diagnostic> {
    let operand_start = bytes.len();

    // Left descriptor: base (imm64 relocated at the operand start), ptr, len.
    append_mov_r15_imm64(bytes, 0);
    if left_is_bounded_buffer {
        bytes.extend([0x49, 0x8d, 0x87]); // lea rax, [r15+disp32] (left bytes)
        bytes.extend(disp32(left_offset + 8)?.to_le_bytes());
        bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15+disp32] (left len)
        bytes.extend(disp32(left_offset)?.to_le_bytes());
    } else {
        bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15+disp32] (left ptr)
        bytes.extend(disp32(left_offset)?.to_le_bytes());
        bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15+disp32] (left len)
        bytes.extend(disp32(left_offset + 8)?.to_le_bytes());
    }

    // Right descriptor: base relocated at the pinned right-base offset; the
    // length load consumes r15 LAST (the base is no longer needed after it).
    debug_assert_eq!(
        bytes.len() - operand_start,
        RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
        "right descriptor base must sit at the pinned relocation offset"
    );
    append_mov_r15_imm64(bytes, 0);
    if right_is_bounded_buffer {
        bytes.extend([0x49, 0x8d, 0x97]); // lea rdx, [r15+disp32] (right bytes)
        bytes.extend(disp32(right_offset + 8)?.to_le_bytes());
        bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15+disp32] (right len)
        bytes.extend(disp32(right_offset)?.to_le_bytes());
    } else {
        bytes.extend([0x49, 0x8b, 0x97]); // mov rdx, [r15+disp32] (right ptr)
        bytes.extend(disp32(right_offset)?.to_le_bytes());
        bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15+disp32] (right len)
        bytes.extend(disp32(right_offset + 8)?.to_le_bytes());
    }

    // result = 0; unequal lengths are unequal text. The jne also means a
    // zero-length pair never enters the loop, so an all-zero (default)
    // descriptor's null pointer is never dereferenced. Fixed 39-byte block:
    //         xor   r9d, r9d
    //         cmp   rcx, r15
    //         jne   done            (+31)
    //   loop: test  rcx, rcx
    //         je    equal           (+20: all bytes matched)
    //         movzx r15d, byte [rax]
    //         cmp   r15b, [rdx]
    //         jne   done            (+17)
    //         inc   rax
    //         inc   rdx
    //         dec   rcx
    //         jmp   loop            (-25)
    //  equal: mov   r9d, 1
    //   done:
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x4c, 0x39, 0xf9]); // cmp rcx, r15
    bytes.extend([0x75, 0x1f]); // jne +31 -> done
    bytes.extend([0x48, 0x85, 0xc9]); // test rcx, rcx
    bytes.extend([0x74, 0x14]); // je +20 -> equal
    bytes.extend([0x44, 0x0f, 0xb6, 0x38]); // movzx r15d, byte [rax]
    bytes.extend([0x44, 0x3a, 0x3a]); // cmp r15b, [rdx]
    bytes.extend([0x75, 0x11]); // jne +17 -> done
    bytes.extend([0x48, 0xff, 0xc0]); // inc rax
    bytes.extend([0x48, 0xff, 0xc2]); // inc rdx
    bytes.extend([0x48, 0xff, 0xc9]); // dec rcx
    bytes.extend([0xeb, 0xe7]); // jmp -25 -> loop
    bytes.extend([0x41, 0xb9]); // mov r9d, imm32 (equal: result = 1)
    bytes.extend(1i32.to_le_bytes());

    // done: move the bool into the requested destination register.
    match destination {
        Reg64::R10 => bytes.extend([0x4d, 0x89, 0xca]), // mov r10, r9
        Reg64::R11 => bytes.extend([0x4d, 0x89, 0xcb]), // mov r11, r9
    }

    debug_assert_eq!(
        bytes.len() - operand_start,
        runtime_text_equals_operand_width(),
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
/// `runtime_text_equals_literal_operand_width`
/// (place-setup plus a fixed head plus 13 bytes per literal byte; every
/// memory operand uses the disp32 form so the shape never varies with the
/// offsets).
///
/// Register use: r15 = relocated base, rax = descriptor address base,
/// r11 = index scratch (frame-indexed setup), rcx/rdx = ptr/len, r9 = the
/// bool result (moved into `destination` last). r12/r13/r14 stay untouched.
fn append_runtime_text_equals_literal_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination: Reg64,
    place: RuntimeValueOperandHandle,
    literal: &[u8],
    place_is_bounded_buffer: bool,
) -> Result<(), Diagnostic> {
    let operand_start = bytes.len();

    // Descriptor address base -> rax (+ `descriptor_disp` displacement). The
    // relocated `mov r15, imm64` sits at the operand start (the relocation
    // planner targets it there).
    let descriptor_disp;
    if let Some((_, byte_offset, _)) = runtime_value_operands.storage(place) {
        append_mov_r15_imm64(bytes, 0);
        append_mov_rax_r15(bytes);
        descriptor_disp = byte_offset;
    } else if let Some((pointer_byte_offset, field_byte_offset, _)) =
        runtime_value_operands.pointee(place)
    {
        // r15 = frame base (relocated); rax = the stored pointer. The
        // descriptor sits in the POINTEE at the field offset -- never read
        // the pointer slot's own bytes as a descriptor.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, pointer_byte_offset)?;
        descriptor_disp = field_byte_offset;
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
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        if index_region == RuntimeStorageRegion::Machine {
            append_mov_r15_imm64(bytes, 0);
        }
        append_load_unsigned_reg_from_r15(bytes, Reg64::R11, index_offset, index_byte_size)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        descriptor_disp = field_byte_offset;
    } else if let Some((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_base_indexed(place)
    {
        // Inline frame fixed array: the elements live in the frame itself at
        // base_byte_offset; rax = frame base + index*element (same shape as
        // the frame-base-indexed load operand above).
        append_mov_r15_imm64(bytes, 0);
        append_load_unsigned_reg_from_r15(bytes, Reg64::R11, index_offset, index_byte_size)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_mov_rax_r15(bytes);
        append_add_rax_r11(bytes);
        descriptor_disp = base_byte_offset + field_byte_offset;
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_fixed_indexed(place)
    {
        // Constant element index: rax = the slice data pointer; the scaled
        // index folds into the descriptor displacement.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        descriptor_disp = element_index
            .checked_mul(element_byte_size)
            .and_then(|scaled| scaled.checked_add(field_byte_offset))
            .ok_or_else(|| {
                Diagnostic::error("X86_64 fixed indexed text descriptor offset overflow")
            })?;
    } else {
        return Err(Diagnostic::error(
            "X86_64 MVP encoder cannot compare this text place against a literal yet",
        ));
    }

    if place_is_bounded_buffer {
        // Owned carrier `{len@0, bytes@8}`: rcx = bytes ADDRESS (rax+disp+8,
        // computed, not a stored pointer); rdx = len read at offset 0. Same widths
        // as the descriptor path (lea/mov are both `48 .. 88/90 disp32` = 7 bytes),
        // so the byte-compare loop, branch offsets, and operand width are all
        // unchanged.
        bytes.extend([0x48, 0x8d, 0x88]); // lea rcx, [rax+disp32] (carrier bytes addr)
        bytes.extend(disp32(descriptor_disp + 8)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x90]); // mov rdx, [rax+disp32] (carrier.len @ 0)
        bytes.extend(disp32(descriptor_disp)?.to_le_bytes());
    } else {
        bytes.extend([0x48, 0x8b, 0x88]); // mov rcx, [rax+disp32] (ptr)
        bytes.extend(disp32(descriptor_disp)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x90]); // mov rdx, [rax+disp32] (len)
        bytes.extend(disp32(descriptor_disp + 8)?.to_le_bytes());
    }

    // result = 0; a length mismatch is unequal text. The jne also means an
    // all-zero (default) descriptor never has its null pointer dereferenced
    // when the literal is non-empty.
    let literal_bytes = literal;
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x48, 0x81, 0xfa]); // cmp rdx, imm32 (literal length)
    bytes.extend(disp32(literal_bytes.len())?.to_le_bytes());
    // Forward distances to `done` (the result move at the end): each byte
    // compare block is 13 bytes, plus the 6-byte equal-result mov.
    bytes.extend([0x0f, 0x85]); // jne rel32 -> done
    bytes.extend(disp32(13 * literal_bytes.len() + 6)?.to_le_bytes());
    for (byte_index, expected_byte) in literal_bytes.iter().enumerate() {
        bytes.extend([0x80, 0xb9]); // cmp byte [rcx+disp32], imm8
        bytes.extend(disp32(byte_index)?.to_le_bytes());
        bytes.push(*expected_byte);
        let remaining_blocks = literal_bytes.len() - 1 - byte_index;
        bytes.extend([0x0f, 0x85]); // jne rel32 -> done
        bytes.extend(disp32(13 * remaining_blocks + 6)?.to_le_bytes());
    }
    bytes.extend([0x41, 0xb9]); // mov r9d, imm32 (equal: result = 1)
    bytes.extend(1i32.to_le_bytes());

    // done: move the bool into the requested destination register.
    match destination {
        Reg64::R10 => bytes.extend([0x4d, 0x89, 0xca]), // mov r10, r9
        Reg64::R11 => bytes.extend([0x4d, 0x89, 0xcb]), // mov r11, r9
    }

    debug_assert_eq!(
        bytes.len() - operand_start,
        runtime_text_equals_literal_operand_width(runtime_value_operands, place, literal),
        "text-equals-literal operand encoder length must match its width"
    );
    Ok(())
}

/// Value width of a runtime operand, looking through nested binary operands.
/// `None` for immediates (which carry no width). Used to size comparisons, whose
/// result type (bool) does not reflect the compared operands' width.
fn runtime_value_operand_value_byte_size(
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
        // Division/modulo are NOT modular: a 64-bit idiv/div on a zero-extended
        // negative i32 dividend yields a wrong quotient. Run at the OPERAND width (an
        // immediate has no width, so use the non-immediate operand's), so a 32-bit
        // op handles the i32 dividend correctly -- signed via cdq, unsigned via the
        // resolver mapping Divide->DivideUnsigned. Add/sub/mul are modular and keep
        // the default 64-bit form. See [[guard-negative-i32-arithmetic]].
        //
        // SHIFTS join this branch for the same reason: a 64-bit `sar` on a
        // zero-extended negative i32 reads the high bit wrong (`-320 >> 2` would
        // become 0x3FFFFFB0, not -80), so run the shift at the shifted VALUE's width
        // (its left operand). A 32-bit `sar`/`shr`/`shl` honors the i32 sign/high bit,
        // and `<<` at the operand width also drops i32 overflow (wrapping semantics)
        // instead of leaking into the upper 32 bits. Both width encodings are the same
        // length, so relocation offsets are unaffected.
        //
        // When BOTH operands are immediates (a constant/constant divide that did not
        // fold) neither has a storage width, so fall back to the TARGET (declared)
        // width -- NOT 4. An i64 constant divide must run 64-bit; a 32-bit core would
        // truncate the dividend (e.g. -9_000_000_000) and the planned/emitted widths
        // would disagree (`runtime_storage_binary_write_width` uses the target size).
        runtime_value_operand_value_byte_size(operands, left)
            .or_else(|| runtime_value_operand_value_byte_size(operands, right))
            .unwrap_or(target_byte_size)
    } else {
        target_byte_size
    }
}

/// The width-correct integer idiv/div core: dividend in r10, divisor in r11,
/// quotient (or remainder, when `want_remainder`) back in r10. A 32-bit divide
/// reads only the low dword, so the width must match the operands. Signed uses
/// cdq/cqo + `idiv`; unsigned zeroes the dividend-high half + `div`. Shared by the
/// normal binary-op path and the saturating divide/modulo helper.
fn append_integer_divide_modulo_core(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
    signed: bool,
) {
    if byte_size <= 4 {
        // Narrow SIGNED operands may arrive ZERO-extended (e.g. the guard-subject
        // load path; see append_saturating_trapping_multiply), so a 32-bit idiv would
        // divide i8 -20 as 236. Sign-extend both to 32 bits first. Idempotent when
        // they are already sign-extended (the storage-write path); unsigned div is
        // correct zero-extended and skips this.
        if signed && byte_size == 1 {
            bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]); // movsx r10, r10b
            bytes.extend([0x4d, 0x0f, 0xbe, 0xdb]); // movsx r11, r11b
        } else if signed && byte_size == 2 {
            bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]); // movsx r10, r10w
            bytes.extend([0x4d, 0x0f, 0xbf, 0xdb]); // movsx r11, r11w
        }
        bytes.extend([0x41, 0x8b, 0xc2]); // mov eax, r10d
        if signed {
            bytes.push(0x99); // cdq (sign-extend eax -> edx)
            bytes.extend([0x41, 0xf7, 0xfb]); // idiv r11d
        } else {
            bytes.extend([0x31, 0xd2]); // xor edx, edx
            bytes.extend([0x41, 0xf7, 0xf3]); // div r11d
        }
        if want_remainder {
            bytes.extend([0x41, 0x89, 0xd2]); // mov r10d, edx (remainder)
        } else {
            bytes.extend([0x41, 0x89, 0xc2]); // mov r10d, eax (quotient)
        }
    } else {
        bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
        if signed {
            bytes.extend([0x48, 0x99]); // cqo (sign-extend rax -> rdx)
            bytes.extend([0x49, 0xf7, 0xfb]); // idiv r11
        } else {
            bytes.extend([0x31, 0xd2]); // xor edx, edx (clears rdx)
            bytes.extend([0x49, 0xf7, 0xf3]); // div r11
        }
        if want_remainder {
            bytes.extend([0x49, 0x89, 0xd2]); // mov r10, rdx (remainder)
        } else {
            bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax (quotient)
        }
    }
}

/// Saturating SIGNED divide/modulo (dividend r10, divisor r11, result r10).
/// Integer division overflows only at TYPE_MIN / -1, the one corner `idiv`
/// hardware-traps on; guard the `divisor == -1` case so Saturating clamps instead
/// of trapping: `a % -1 == 0`, and `a / -1 == -a` saturating TYPE_MIN -> TYPE_MAX.
/// Every other divisor goes through the normal idiv (division reduces magnitude,
/// so no quotient/remainder can overflow). Unsigned div/mod never overflow and so
/// never reach here -- they fall through to the normal path.
fn append_saturating_signed_divide_modulo(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
) -> Result<(), Diagnostic> {
    // cmp r11, -1 (sized): the only divisor needing the saturating fixup.
    if byte_size <= 4 {
        bytes.extend([0x41, 0x83, 0xfb, 0xff]); // cmp r11d, -1
    } else {
        bytes.extend([0x49, 0x83, 0xfb, 0xff]); // cmp r11, -1
    }
    // The divisor == -1 fixup block.
    let mut special: Vec<u8> = Vec::new();
    if want_remainder {
        special.extend([0x45, 0x31, 0xd2]); // xor r10d, r10d  (a % -1 == 0)
    } else if byte_size <= 2 {
        // i8/i16: the dividend rides sign-extended in a 32-bit register, so `neg`
        // does NOT wrap at the narrow width -- a == TYPE_MIN yields -TYPE_MIN ==
        // TYPE_MAX + 1 (e.g. 128 for i8), the only overflow. The i32/i64 path below
        // detects TYPE_MIN via `neg`'s overflow flag, which a narrow TYPE_MIN cannot
        // set; here instead clamp any result above TYPE_MAX down to TYPE_MAX.
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u32;
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (-a; a==TYPE_MIN -> TYPE_MAX+1)
        special.push(0x41);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9d, TYPE_MAX
        special.extend([0x45, 0x39, 0xca]); // cmp r10d, r9d
        special.extend([0x45, 0x0f, 0x4f, 0xd1]); // cmovg r10d, r9d  (> TYPE_MAX -> TYPE_MAX)
    } else if byte_size <= 4 {
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u32;
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (sets OF iff r10d == TYPE_MIN)
        special.push(0x41);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9d, TYPE_MAX
        special.extend([0x45, 0x0f, 0x40, 0xd1]); // cmovo r10d, r9d  (TYPE_MIN -> TYPE_MAX)
    } else {
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
        special.extend([0x49, 0xf7, 0xda]); // neg r10
        special.push(0x49);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9, TYPE_MAX
        special.extend([0x4d, 0x0f, 0x40, 0xd1]); // cmovo r10, r9
    }
    // The normal idiv (every divisor except -1).
    let mut normal: Vec<u8> = Vec::new();
    append_integer_divide_modulo_core(&mut normal, byte_size, want_remainder, true);
    // jne over (special + the jmp) to the idiv; run special; jmp past the idiv.
    // Both blocks are well under 128 bytes, so rel8 offsets suffice.
    bytes.push(0x75);
    bytes.push((special.len() + 2) as u8); // jne -> normal
    bytes.extend(special);
    bytes.push(0xeb);
    bytes.push(normal.len() as u8); // jmp -> done
    bytes.extend(normal);
    Ok(())
}

/// WRAPPING signed divide/modulo. x86 `idiv` raises #DE (integer-overflow trap)
/// for TYPE_MIN / -1; the Wrapping domain must instead produce the WRAPPED result
/// (TYPE_MIN for divide -- the true quotient TYPE_MAX+1 wraps to TYPE_MIN -- and 0
/// for modulo). Guard the single overflowing divisor (-1) and avoid idiv for it:
/// `a / -1 == -a` via `neg r10` (and `neg` of TYPE_MIN naturally wraps to
/// TYPE_MIN, so no clamp is needed, unlike the saturating variant); `a % -1 == 0`.
/// Narrow widths (i8/i16) let the store truncate the negated 32-bit value back to
/// the correct wrapped byte. Divide-by-zero still reaches `idiv` and traps,
/// matching the interpreter. (aarch64 `sdiv` does not trap on overflow, so this
/// guard is x86_64-only.)
fn append_wrapping_signed_divide_modulo(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
) -> Result<(), Diagnostic> {
    // cmp r11, -1 (sized): the only divisor that would overflow idiv.
    if byte_size <= 4 {
        bytes.extend([0x41, 0x83, 0xfb, 0xff]); // cmp r11d, -1
    } else {
        bytes.extend([0x49, 0x83, 0xfb, 0xff]); // cmp r11, -1
    }
    // The divisor == -1 fixup block (always 3 bytes).
    let mut special: Vec<u8> = Vec::new();
    if want_remainder {
        special.extend([0x45, 0x31, 0xd2]); // xor r10d, r10d  (a % -1 == 0)
    } else if byte_size <= 4 {
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (-a; TYPE_MIN wraps to TYPE_MIN)
    } else {
        special.extend([0x49, 0xf7, 0xda]); // neg r10
    }
    // The normal idiv (every divisor except -1).
    let mut normal: Vec<u8> = Vec::new();
    append_integer_divide_modulo_core(&mut normal, byte_size, want_remainder, true);
    bytes.push(0x75);
    bytes.push((special.len() + 2) as u8); // jne -> normal
    bytes.extend(special);
    bytes.push(0xeb);
    bytes.push(normal.len() as u8); // jmp -> done
    bytes.extend(normal);
    Ok(())
}

pub(crate) fn append_runtime_binary_operation(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    match operator {
        StateGuardOperator::Add => bytes.extend([0x4d, 0x01, 0xda]), // add r10, r11
        StateGuardOperator::And => bytes.extend([0x4d, 0x21, 0xda]), // and r10, r11
        StateGuardOperator::Or => bytes.extend([0x4d, 0x09, 0xda]),  // or r10, r11
        StateGuardOperator::BitwiseAnd => bytes.extend([0x4d, 0x21, 0xda]), // and r10, r11
        StateGuardOperator::BitwiseOr => bytes.extend([0x4d, 0x09, 0xda]), // or r10, r11
        StateGuardOperator::BitwiseXor => bytes.extend([0x4d, 0x31, 0xda]), // xor r10, r11
        StateGuardOperator::Subtract => bytes.extend([0x4d, 0x29, 0xda]), // sub r10, r11
        StateGuardOperator::Multiply => bytes.extend([0x4d, 0x0f, 0xaf, 0xd3]), // imul r10, r11
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => {
            // Compare at the operand width (32-bit for i32, else 64-bit) so an
            // i32 sign/high bit is read correctly, then conditionally take r11.
            // Max keeps the larger (cmovl signed / cmovb unsigned: replace when
            // r10 < r11); Min keeps the smaller (cmovg / cmova: replace when
            // r10 > r11).
            let keep_smaller = matches!(
                operator,
                StateGuardOperator::Min | StateGuardOperator::MinUnsigned
            );
            let unsigned = matches!(
                operator,
                StateGuardOperator::MaxUnsigned | StateGuardOperator::MinUnsigned
            );
            // cmov opcode byte: signed below/above use 4c/4f; unsigned 42/47.
            let cmov = match (keep_smaller, unsigned) {
                (false, false) => 0x4c, // cmovl
                (true, false) => 0x4f,  // cmovg
                (false, true) => 0x42,  // cmovb
                (true, true) => 0x47,   // cmova
            };
            if byte_size <= 4 {
                bytes.extend([0x45, 0x39, 0xda]); // cmp r10d, r11d
                bytes.extend([0x45, 0x0f, cmov, 0xd3]); // cmovcc r10d, r11d
            } else {
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x4d, 0x0f, cmov, 0xd3]); // cmovcc r10, r11
            }
        }
        StateGuardOperator::Divide
        | StateGuardOperator::Modulo
        | StateGuardOperator::DivideUnsigned
        | StateGuardOperator::ModuloUnsigned => {
            // Quotient -> (r/e)ax, remainder -> (r/e)dx; the width-correct idiv
            // sequence lives in the shared core (also used by saturating div/mod).
            let want_remainder = matches!(
                operator,
                StateGuardOperator::Modulo | StateGuardOperator::ModuloUnsigned
            );
            let signed = matches!(
                operator,
                StateGuardOperator::Divide | StateGuardOperator::Modulo
            );
            append_integer_divide_modulo_core(bytes, byte_size, want_remainder, signed);
        }
        StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => {
            // Shift count must live in cl. Right shift is arithmetic (`sar`) for
            // signed operands and logical (`shr`) for unsigned; sized to the
            // operands so an i32 high bit is honored.
            let arithmetic_right = matches!(operator, StateGuardOperator::ShiftRight);
            let logical_right = matches!(operator, StateGuardOperator::ShiftRightLogical);
            if byte_size <= 4 {
                bytes.extend([0x44, 0x89, 0xd9]); // mov ecx, r11d
                if arithmetic_right {
                    bytes.extend([0x41, 0xd3, 0xfa]); // sar r10d, cl
                } else if logical_right {
                    bytes.extend([0x41, 0xd3, 0xea]); // shr r10d, cl
                } else {
                    bytes.extend([0x41, 0xd3, 0xe2]); // shl r10d, cl
                }
            } else {
                bytes.extend([0x4c, 0x89, 0xd9]); // mov rcx, r11
                if arithmetic_right {
                    bytes.extend([0x49, 0xd3, 0xfa]); // sar r10, cl
                } else if logical_right {
                    bytes.extend([0x49, 0xd3, 0xea]); // shr r10, cl
                } else {
                    bytes.extend([0x49, 0xd3, 0xe2]); // shl r10, cl
                }
            }
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
            // width, not the bool result) so an i32 sign bit is read correctly.
            // Ordering uses signed setcc (setl/setg/...) or unsigned (setb/seta/
            // ...) per the operand type.
            append_cmp_r10_r11(bytes, byte_size)?;
            bytes.extend(match operator {
                StateGuardOperator::Equal => [0x0f, 0x94, 0xc0], // sete
                StateGuardOperator::NotEqual => [0x0f, 0x95, 0xc0], // setne
                StateGuardOperator::Greater => [0x0f, 0x9f, 0xc0], // setg
                StateGuardOperator::GreaterOrEqual => [0x0f, 0x9d, 0xc0], // setge
                StateGuardOperator::Less => [0x0f, 0x9c, 0xc0],  // setl
                StateGuardOperator::LessOrEqual => [0x0f, 0x9e, 0xc0], // setle
                StateGuardOperator::GreaterUnsigned => [0x0f, 0x97, 0xc0], // seta
                StateGuardOperator::GreaterOrEqualUnsigned => [0x0f, 0x93, 0xc0], // setae
                StateGuardOperator::LessUnsigned => [0x0f, 0x92, 0xc0], // setb
                StateGuardOperator::LessOrEqualUnsigned => [0x0f, 0x96, 0xc0], // setbe
                _ => unreachable!(),
            });
            bytes.extend([0x44, 0x0f, 0xb6, 0xd0]); // movzx r10d, al
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime binary operator `{operator:?}` is not implemented yet"
            )));
        }
    }
    Ok(())
}

/// Saturating/Trapping logical `>>` zero clamp (floor semantics, until F8c):
/// floor(x / 2^n) with a count at/above the TYPE width yields 0, but the
/// hardware `shr` masks the count to the op width instead (40 & 31 = 8). The
/// FULL count survives in r11 (the shift arm only copies it to cl), so
/// compare it UNSIGNED against the bit width and cmov zero over the shifted
/// result -- a negative signed count is huge unsigned and clamps. rax is
/// scratch mid-operation, as in the div/setcc arms. WRAPPING shifts no
/// longer take this fix: F8b masks their count instead (ch5 ruling).
fn append_wrapping_shift_zero_clamp(bytes: &mut Vec<u8>, byte_size: usize) {
    bytes.extend([0x31, 0xc0]); // xor eax, eax
    bytes.extend([0x49, 0x83, 0xfb, (byte_size * 8) as u8]); // cmp r11, width_bits
    bytes.extend([0x4c, 0x0f, 0x43, 0xd0]); // cmovae r10, rax
}

/// Bytes of [`append_wrapping_shift_zero_clamp`]: xor (2) + cmp (4) + cmov (4).
const WRAPPING_SHIFT_ZERO_CLAMP_WIDTH: usize = 10;

/// Saturating/Trapping arithmetic `>>` count saturation (floor semantics,
/// until F8c): floor(x / 2^n) SIGN-FILLS for an at/above-width count, and a
/// post-fix cannot recover the sign once the hardware-masked `sar` has
/// consumed the value -- so saturate the COUNT to width-1 first (`sar` by
/// width-1 IS the sign-fill). Runs BEFORE the plain shift arm, which copies
/// the (now saturated) r11 into cl. rax is scratch. WRAPPING `>>` no longer
/// takes this fix: F8b masks its count instead (ch5 ruling).
fn append_wrapping_shift_right_count_saturate(bytes: &mut Vec<u8>, byte_size: usize) {
    let width_bits = (byte_size * 8) as u8;
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend(u32::from(width_bits - 1).to_le_bytes());
    bytes.extend([0x49, 0x83, 0xfb, width_bits]); // cmp r11, width_bits
    bytes.extend([0x4c, 0x0f, 0x43, 0xd8]); // cmovae r11, rax
}

/// Bytes of [`append_wrapping_shift_right_count_saturate`]: mov (5) + cmp (4)
/// + cmov (4).
const WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH: usize = 13;

/// F8b (ch5 shift-count ruling): a WRAPPING shift masks the COUNT to the
/// operand width (`k & (width - 1)`). The hardware `shl`/`shr`/`sar` already
/// mask mod the OP width (32/64) -- exactly the ruling at widths 4/8 -- so
/// only sub-word operands need the explicit mask. Runs BEFORE the plain
/// shift arm (which copies r11 into cl); masks r11 in place.
fn append_wrapping_shift_count_mask(bytes: &mut Vec<u8>, byte_size: usize) {
    if matches!(byte_size, 1 | 2) {
        let mask = (byte_size * 8 - 1) as u8;
        bytes.extend([0x41, 0x83, 0xe3, mask]); // and r11d, mask
    }
}

/// Bytes of [`append_wrapping_shift_count_mask`]: and r11d, imm8 (4) for
/// sub-word operands; 0 at widths 4/8 (the hardware mask IS the ruling).
const fn wrapping_shift_count_mask_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 2 => 4,
        _ => 0,
    }
}

/// F8c count guard: `cmp r11, width ; jb +2 ; ud2` -- a TRAPPING shift's
/// out-of-range count traps BEFORE the shift runs, regardless of the shifted
/// value (`0 << 40` traps; the count is invalid, not the result). The full
/// count survives in r11 (the shift arm only copies it to cl), and reads
/// UNSIGNED so a negative signed count is huge and traps.
fn append_shift_count_trap_guard(bytes: &mut Vec<u8>, byte_size: usize) {
    bytes.extend([0x49, 0x83, 0xfb, (byte_size * 8) as u8]); // cmp r11, width_bits
    bytes.extend([0x72, 0x02]); // jb +2 (an in-range count hops the ud2)
    bytes.extend([0x0f, 0x0b]); // ud2
}

/// Bytes of [`append_shift_count_trap_guard`]: cmp (4) + jb (2) + ud2 (2).
const SHIFT_COUNT_TRAP_GUARD_WIDTH: usize = 8;

/// A nested WRAPPING binary hands its PARENT the width-wrapped VALUE in r10
/// (the interpreter wraps AT THE NODE, decision 17; the store-truncation
/// shortcut only holds at the WRITE): extend r10 from the node's width by the
/// node's signedness. No-op at full width.
fn append_wrapping_node_width_extension(
    bytes: &mut Vec<u8>,
    byte_width: usize,
    operands_signed: bool,
) {
    match (byte_width, operands_signed) {
        (1, false) => bytes.extend([0x4d, 0x0f, 0xb6, 0xd2]), // movzx r10, r10b
        (2, false) => bytes.extend([0x4d, 0x0f, 0xb7, 0xd2]), // movzx r10, r10w
        (4, false) => bytes.extend([0x45, 0x89, 0xd2]),       // mov r10d, r10d
        (1, true) => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]),  // movsx r10, r10b
        (2, true) => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]),  // movsx r10, r10w
        (4, true) => bytes.extend([0x4d, 0x63, 0xd2]),        // movsxd r10, r10d
        _ => {}
    }
}

/// Bytes of [`append_wrapping_node_width_extension`]: 4, except 3 for the
/// width-4 forms and 0 at full width.
fn wrapping_node_width_extension_width(byte_width: usize) -> usize {
    match byte_width {
        4 => 3,
        1 | 2 => 4,
        _ => 0,
    }
}

fn float_policy_applies(operator: StateGuardOperator, domain: ArithmeticDomain) -> bool {
    matches!(
        domain,
        ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
    ) && matches!(
        operator,
        StateGuardOperator::Add
            | StateGuardOperator::AddTowardZero
            | StateGuardOperator::AddTowardPositive
            | StateGuardOperator::AddTowardNegative
            | StateGuardOperator::Subtract
            | StateGuardOperator::SubtractTowardZero
            | StateGuardOperator::SubtractTowardPositive
            | StateGuardOperator::SubtractTowardNegative
            | StateGuardOperator::Multiply
            | StateGuardOperator::MultiplyTowardZero
            | StateGuardOperator::MultiplyTowardPositive
            | StateGuardOperator::MultiplyTowardNegative
            | StateGuardOperator::MultiplyThenAdd
            | StateGuardOperator::Divide
            | StateGuardOperator::DivideTowardZero
            | StateGuardOperator::DivideTowardPositive
            | StateGuardOperator::DivideTowardNegative
            | StateGuardOperator::Min
            | StateGuardOperator::Max
            | StateGuardOperator::Sqrt
            | StateGuardOperator::SqrtTowardZero
            | StateGuardOperator::SqrtTowardPositive
            | StateGuardOperator::SqrtTowardNegative
    )
}

fn directed_float_mxcsr(operator: StateGuardOperator) -> Option<u32> {
    match operator {
        // Canonical 0x1f80 plus MXCSR.RC in bits 13..14.
        StateGuardOperator::AddTowardNegative
        | StateGuardOperator::SubtractTowardNegative
        | StateGuardOperator::MultiplyTowardNegative
        | StateGuardOperator::DivideTowardNegative
        | StateGuardOperator::SqrtTowardNegative => Some(0x3f80),
        StateGuardOperator::AddTowardPositive
        | StateGuardOperator::SubtractTowardPositive
        | StateGuardOperator::MultiplyTowardPositive
        | StateGuardOperator::DivideTowardPositive
        | StateGuardOperator::SqrtTowardPositive => Some(0x5f80),
        StateGuardOperator::AddTowardZero
        | StateGuardOperator::SubtractTowardZero
        | StateGuardOperator::MultiplyTowardZero
        | StateGuardOperator::DivideTowardZero
        | StateGuardOperator::SqrtTowardZero => Some(0x7f80),
        _ => None,
    }
}

fn append_directed_float_control_prefix(bytes: &mut Vec<u8>, mxcsr: u32) {
    bytes.extend([0x48, 0x83, 0xec, 0x10]); // sub rsp,16
    bytes.extend([0x0f, 0xae, 0x1c, 0x24]); // stmxcsr [rsp]
    bytes.extend([0xc7, 0x44, 0x24, 0x04]); // mov dword [rsp+4],imm32
    bytes.extend(mxcsr.to_le_bytes());
    bytes.extend([0x0f, 0xae, 0x54, 0x24, 0x04]); // ldmxcsr [rsp+4]
}

fn append_directed_float_control_suffix(bytes: &mut Vec<u8>) {
    bytes.extend([0x0f, 0xae, 0x14, 0x24]); // ldmxcsr [rsp]
    bytes.extend([0x48, 0x83, 0xc4, 0x10]); // add rsp,16
}

const DIRECTED_FLOAT_CONTROL_WIDTH: usize = 29;

#[derive(Clone, Copy)]
enum FloatPolicySource {
    Result,
    Left,
    Middle,
    Right,
}

/// Copy one raw f32/f64 bit pattern to rax and clear its sign bit. The F5
/// policy guard classifies floats entirely as unsigned integers: below/equal/
/// above the positive-infinity pattern means finite/infinite/NaN.
fn append_float_abs_to_rax(bytes: &mut Vec<u8>, source: FloatPolicySource, byte_size: usize) {
    match source {
        FloatPolicySource::Result => bytes.extend([0x4c, 0x89, 0xd0]), // mov rax,r10
        FloatPolicySource::Left => bytes.extend([0x4c, 0x89, 0xc0]),   // mov rax,r8
        FloatPolicySource::Middle => bytes.extend([0x48, 0x89, 0xd0]), // mov rax,rdx
        FloatPolicySource::Right => bytes.extend([0x4c, 0x89, 0xd8]),  // mov rax,r11
    }
    if byte_size > 4 {
        bytes.extend([0x48, 0x0f, 0xba, 0xf0, 0x3f]); // btr rax,63
    } else {
        bytes.push(0x25); // and eax,0x7fff_ffff
        bytes.extend(0x7fff_ffff_u32.to_le_bytes());
    }
}

fn append_cmp_rax_r9(bytes: &mut Vec<u8>) {
    bytes.extend([0x4c, 0x39, 0xc8]); // cmp rax,r9
}

fn append_policy_branch_placeholder(bytes: &mut Vec<u8>, opcode: u8) -> usize {
    let start = bytes.len();
    bytes.extend([0x0f, opcode, 0, 0, 0, 0]);
    start
}

fn patch_policy_branch(
    bytes: &mut [u8],
    branch_start: usize,
    target: usize,
) -> Result<(), Diagnostic> {
    let displacement = target as isize - (branch_start + 6) as isize;
    let displacement = rel32(displacement)?;
    bytes[branch_start + 2..branch_start + 6].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

/// F5 float-arithmetic policy guard. Entry: r10=result bits, r8=preserved
/// left bits, r11=right bits, and optionally rdx=middle bits for MTA. Exit:
/// r10 is unchanged or clamped; r8/r9/rdx/r11 and rax are scratch. The branch
/// targets are patched from the emitted byte stream, so the width twin can use
/// this function's actual length.
fn float_policy_guard_bytes(
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    include_middle: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !float_policy_applies(operator, domain) {
        return Ok(Vec::new());
    }

    let (inf_bits, max_bits, sign_bits) = if byte_size > 4 {
        (
            0x7ff0_0000_0000_0000_u64,
            0x7fef_ffff_ffff_ffff_u64,
            0x8000_0000_0000_0000_u64,
        )
    } else {
        (0x7f80_0000_u64, 0x7f7f_ffff_u64, 0x8000_0000_u64)
    };
    let mut bytes = Vec::new();
    bytes.extend([0x49, 0xb9]); // mov r9,imm64 (positive infinity bits)
    bytes.extend(inf_bits.to_le_bytes());
    append_float_abs_to_rax(&mut bytes, FloatPolicySource::Result, byte_size);
    append_cmp_rax_r9(&mut bytes);

    match domain {
        ArithmeticDomain::Saturating => {
            let mut end_branches = Vec::new();
            // Only an exactly infinite result is magnitude overflow. Finite
            // results and NaNs pass through (invalid remains a Finite duty).
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x85)); // jne end

            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Right, byte_size);
            if operator == StateGuardOperator::Divide {
                bytes.extend([0x48, 0x83, 0xf8, 0x00]); // cmp rax,0
                // Division by +/-0 keeps IEEE infinity; it is not overflow.
                end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x84)); // je end
            }
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end

            if include_middle {
                append_float_abs_to_rax(&mut bytes, FloatPolicySource::Middle, byte_size);
                append_cmp_rax_r9(&mut bytes);
                end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end
            }

            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Left, byte_size);
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end

            // Clamp to MAX_FINITE with the result's sign.
            bytes.extend([0x49, 0xb9]);
            bytes.extend(sign_bits.to_le_bytes());
            bytes.extend([0x4d, 0x21, 0xca]); // and r10,r9
            bytes.extend([0x49, 0xb9]);
            bytes.extend(max_bits.to_le_bytes());
            bytes.extend([0x4d, 0x09, 0xca]); // or r10,r9

            let end = bytes.len();
            for branch in end_branches {
                patch_policy_branch(&mut bytes, branch, end)?;
            }
        }
        ArithmeticDomain::Trapping => {
            // Trapping is result-only: every NaN or infinity traps, including
            // a non-finite value propagated from a non-finite operand.
            let finite_branch = append_policy_branch_placeholder(&mut bytes, 0x82); // jb end
            bytes.extend([0x0f, 0x0b]); // ud2: non-finite result
            let end = bytes.len();
            patch_policy_branch(&mut bytes, finite_branch, end)?;
        }
        _ => unreachable!("policy applicability gated above"),
    }
    Ok(bytes)
}

/// Internal ternary carrier: preserve the second MTA operand in rdx and
/// return the third in r10. The surrounding binary evaluator already
/// preserves the first operand on its stack.
fn append_float_pair(bytes: &mut Vec<u8>) {
    bytes.extend([0x4c, 0x89, 0xd2]); // mov rdx,r10 (middle)
    bytes.extend([0x4d, 0x89, 0xda]); // mov r10,r11 (third/result)
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

/// Classify the raw IEEE operand bits in r10 and leave a canonical bool in
/// r10. Signless bit-pattern ordering makes the format boundaries exact and
/// avoids changing the host FP environment.
fn float_classification_predicate_bytes(
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let (infinity, minimum_normal) = if byte_size > 4 {
        (0x7ff0_0000_0000_0000_u64, 0x0010_0000_0000_0000_u64)
    } else {
        (0x7f80_0000_u64, 0x0080_0000_u64)
    };
    let mut bytes = Vec::new();
    append_float_abs_to_rax(&mut bytes, FloatPolicySource::Result, byte_size);
    bytes.extend([0x45, 0x31, 0xd2]); // xor r10d,r10d (default false)
    let mut end_branches = Vec::new();
    match operator {
        StateGuardOperator::IsFinite => {
            bytes.extend([0x49, 0xb9]); // mov r9,imm64
            bytes.extend(infinity.to_le_bytes());
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end
        }
        StateGuardOperator::IsInfinite => {
            bytes.extend([0x49, 0xb9]);
            bytes.extend(infinity.to_le_bytes());
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x85)); // jne end
        }
        StateGuardOperator::IsNormal => {
            bytes.extend([0x49, 0xb9]);
            bytes.extend(minimum_normal.to_le_bytes());
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x82)); // jb end
            bytes.extend([0x49, 0xb9]);
            bytes.extend(infinity.to_le_bytes());
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end
        }
        StateGuardOperator::IsSubnormal => {
            bytes.extend([0x48, 0x83, 0xf8, 0x00]); // cmp rax,0
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x84)); // je end
            bytes.extend([0x49, 0xb9]);
            bytes.extend(minimum_normal.to_le_bytes());
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end
        }
        _ => unreachable!("classification helper is predicate-only"),
    }
    bytes.extend([0x41, 0xba]); // mov r10d,1
    bytes.extend(1_u32.to_le_bytes());
    let end = bytes.len();
    for branch in end_branches {
        patch_policy_branch(&mut bytes, branch, end)?;
    }
    Ok(bytes)
}

fn append_rel32_jump_placeholder(bytes: &mut Vec<u8>) -> usize {
    let start = bytes.len();
    bytes.extend([0xe9, 0, 0, 0, 0]);
    start
}

fn patch_rel32_jump(
    bytes: &mut [u8],
    branch_start: usize,
    target: usize,
) -> Result<(), Diagnostic> {
    let displacement = target as isize - (branch_start + 5) as isize;
    let displacement = rel32(displacement)?;
    bytes[branch_start + 1..branch_start + 5].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

fn append_packed_float_class(bytes: &mut Vec<u8>, tag: u8) {
    bytes.extend([0x4d, 0x89, 0xc2]); // mov r10,r8 (negative payload at bit 32)
    bytes.extend([0x49, 0x83, 0xca, tag]); // or r10,tag
}

/// Return the stable native `FloatClass` carrier in r10: i32 tag at bits
/// 0..31 and the overlaid `negative: bool` payload at bit 32. Tags follow the
/// source declaration order: NaN=0, Infinity=1, Normal=2, Subnormal=3, Zero=4.
fn float_classify_bytes(byte_size: usize) -> Result<Vec<u8>, Diagnostic> {
    let (infinity, minimum_normal, sign_shift) = if byte_size > 4 {
        (0x7ff0_0000_0000_0000_u64, 0x0010_0000_0000_0000_u64, 63)
    } else {
        (0x7f80_0000_u64, 0x0080_0000_u64, 31)
    };
    let mut bytes = Vec::new();
    append_float_abs_to_rax(&mut bytes, FloatPolicySource::Result, byte_size);
    bytes.extend([0x4d, 0x89, 0xd0]); // mov r8,r10
    bytes.extend([0x49, 0xc1, 0xe8, sign_shift]); // shr r8,31/63
    bytes.extend([0x49, 0xc1, 0xe0, 0x20]); // shl r8,32

    bytes.extend([0x49, 0xb9]);
    bytes.extend(infinity.to_le_bytes());
    append_cmp_rax_r9(&mut bytes);
    let nan_branch = append_policy_branch_placeholder(&mut bytes, 0x87); // ja nan
    let infinity_branch = append_policy_branch_placeholder(&mut bytes, 0x84); // je infinity
    bytes.extend([0x48, 0x83, 0xf8, 0x00]); // cmp rax,0
    let zero_branch = append_policy_branch_placeholder(&mut bytes, 0x84); // je zero
    bytes.extend([0x49, 0xb9]);
    bytes.extend(minimum_normal.to_le_bytes());
    append_cmp_rax_r9(&mut bytes);
    let subnormal_branch = append_policy_branch_placeholder(&mut bytes, 0x82); // jb subnormal

    append_packed_float_class(&mut bytes, 2);
    let normal_end = append_rel32_jump_placeholder(&mut bytes);
    let subnormal = bytes.len();
    append_packed_float_class(&mut bytes, 3);
    let subnormal_end = append_rel32_jump_placeholder(&mut bytes);
    let zero = bytes.len();
    append_packed_float_class(&mut bytes, 4);
    let zero_end = append_rel32_jump_placeholder(&mut bytes);
    let infinity_label = bytes.len();
    append_packed_float_class(&mut bytes, 1);
    let infinity_end = append_rel32_jump_placeholder(&mut bytes);
    let nan = bytes.len();
    bytes.extend([0x45, 0x31, 0xd2]); // xor r10d,r10d (NaN tag, no payload)
    let end = bytes.len();

    patch_policy_branch(&mut bytes, nan_branch, nan)?;
    patch_policy_branch(&mut bytes, infinity_branch, infinity_label)?;
    patch_policy_branch(&mut bytes, zero_branch, zero)?;
    patch_policy_branch(&mut bytes, subnormal_branch, subnormal)?;
    for branch in [normal_end, subnormal_end, zero_end, infinity_end] {
        patch_rel32_jump(&mut bytes, branch, end)?;
    }
    Ok(bytes)
}

fn float_multiply_then_add_bytes(
    byte_size: usize,
    domain: ArithmeticDomain,
) -> Result<Vec<u8>, Diagnostic> {
    let wide = byte_size > 4;
    let mut bytes = Vec::new();
    if float_policy_applies(StateGuardOperator::MultiplyThenAdd, domain) {
        bytes.extend([0x4d, 0x89, 0xd0]); // mov r8,r10 (first)
    }
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0,r10
        bytes.extend([0x66, 0x48, 0x0f, 0x6e, 0xca]); // movq xmm1,rdx
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xd3]); // movq xmm2,r11
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0,r10d
        bytes.extend([0x66, 0x0f, 0x6e, 0xca]); // movd xmm1,edx
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xd3]); // movd xmm2,r11d
    }
    let scalar_prefix = if wide { 0xf2 } else { 0xf3 };
    bytes.extend([scalar_prefix, 0x0f, 0x59, 0xc1]); // muls{d,s} xmm0,xmm1
    bytes.extend([scalar_prefix, 0x0f, 0x58, 0xc2]); // adds{d,s} xmm0,xmm2
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10,xmm0
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d,xmm0
    }
    if float_policy_applies(StateGuardOperator::MultiplyThenAdd, domain) {
        bytes.extend(float_policy_guard_bytes(
            domain,
            StateGuardOperator::MultiplyThenAdd,
            byte_size,
            true,
        )?);
    }
    Ok(bytes)
}

/// Floating-point binary op (f64/f32) that reuses the integer operand pipeline:
/// the operand bit patterns are already loaded in r10 (left) and r11 (right).
/// Move them into xmm0/xmm1, run the SSE arithmetic op, then move the result
/// bits back to r10 so the shared store path writes them out. `byte_size > 4`
/// selects f64 (`movq` + `*sd`); otherwise f32 (`movd` + `*ss`). Always the
/// base per-operator width plus any emitted policy guard; the domain-aware
/// width twin calls the same guard emitter.
fn append_runtime_float_binary_operation(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
    domain: ArithmeticDomain,
) -> Result<(), Diagnostic> {
    if operator == StateGuardOperator::FloatPair {
        append_float_pair(bytes);
        return Ok(());
    }
    if operator == StateGuardOperator::MultiplyThenAdd {
        bytes.extend(float_multiply_then_add_bytes(byte_size, domain)?);
        return Ok(());
    }
    if is_integer_float_classification_predicate(operator) {
        bytes.extend(float_classification_predicate_bytes(operator, byte_size)?);
        return Ok(());
    }
    if operator == StateGuardOperator::FloatClassify {
        bytes.extend(float_classify_bytes(byte_size)?);
        return Ok(());
    }
    let wide = byte_size > 4;
    let guarded = float_policy_applies(operator, domain);
    if guarded {
        // The result overwrites r10. Saturating still classifies both source
        // operands; keeping the left operand here also preserves a single
        // width path for both checked policies.
        bytes.extend([0x4d, 0x89, 0xd0]); // mov r8,r10
    }
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
    }
    // F2 = scalar-double prefix (`*sd`), F3 = scalar-single (`*ss`).
    let scalar_prefix = if wide { 0xf2 } else { 0xf3 };
    let directed_mxcsr = directed_float_mxcsr(operator);
    if let Some(mxcsr) = directed_mxcsr {
        append_directed_float_control_prefix(bytes, mxcsr);
    }
    let opcode = match operator {
        StateGuardOperator::Add
        | StateGuardOperator::AddTowardZero
        | StateGuardOperator::AddTowardPositive
        | StateGuardOperator::AddTowardNegative => 0x58, // addsd/addss
        StateGuardOperator::Subtract
        | StateGuardOperator::SubtractTowardZero
        | StateGuardOperator::SubtractTowardPositive
        | StateGuardOperator::SubtractTowardNegative => 0x5c, // subsd/subss
        StateGuardOperator::Multiply
        | StateGuardOperator::MultiplyTowardZero
        | StateGuardOperator::MultiplyTowardPositive
        | StateGuardOperator::MultiplyTowardNegative => 0x59, // mulsd/mulss
        StateGuardOperator::Divide
        | StateGuardOperator::DivideTowardZero
        | StateGuardOperator::DivideTowardPositive
        | StateGuardOperator::DivideTowardNegative => 0x5e, // divsd/divss
        // `maxsd a, b` / `minsd a, b` return b on unordered (NaN) or equal, so
        // they realize `if a > b { a } else { b }` (and the min mirror) --
        // which the interpreter's float min/max matches exactly. This is what
        // makes float min/max, and hence abs/clamp over floats, lower.
        StateGuardOperator::Max => 0x5f, // maxsd/maxss
        StateGuardOperator::Min => 0x5d, // minsd/minss
        // sqrt is UNARY, carried with both operands = x: `sqrtsd xmm0, xmm1`
        // computes sqrt(xmm1) = sqrt(x) into xmm0, so the shared final line
        // below (op on xmm0, xmm1) already produces the right result.
        StateGuardOperator::Sqrt
        | StateGuardOperator::SqrtTowardZero
        | StateGuardOperator::SqrtTowardPositive
        | StateGuardOperator::SqrtTowardNegative => 0x51, // sqrtsd/sqrtss
        StateGuardOperator::IsNan => {
            if wide {
                bytes.extend([0x66, 0x0f, 0x2e, 0xc0]); // ucomisd xmm0,xmm0
            } else {
                bytes.extend([0x0f, 0x2e, 0xc0]); // ucomiss xmm0,xmm0
                bytes.push(0x90); // keep f32/f64 widths identical
            }
            bytes.extend([0x0f, 0x9a, 0xc0]); // setp al (unordered)
            bytes.extend([0x44, 0x0f, 0xb6, 0xd0]); // movzx r10d, al
            return Ok(());
        }
        // COMPARISON into a 0/1 result in r10 (`let ok: bool = self.a >
        // self.b` with float operands), the aarch64 twin. `ucomis*` sets
        // ZF/PF/CF (unordered = all three): ordering picks the operand ORDER
        // so an unsigned-above condition is FALSE on unordered for free
        // (`>`/`>=` compare (xmm0,xmm1) + seta/setae; `<`/`<=` swap to
        // (xmm1,xmm0)); equality needs the parity dance (unordered sets ZF,
        // so a bare sete/setne would call NaN == NaN true) -- a short
        // branch-over pattern keeps it register-free. f32's 3-byte `ucomiss`
        // takes a 1-byte NOP pad so the sequence length stays f32/f64
        // identical (the relocation-offset invariant). Widths tracked by
        // `runtime_float_binary_operation_width` -- MUST stay in lockstep.
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
            let swapped = matches!(
                operator,
                StateGuardOperator::Less
                    | StateGuardOperator::LessOrEqual
                    | StateGuardOperator::LessUnsigned
                    | StateGuardOperator::LessOrEqualUnsigned
            );
            let modrm = if swapped { 0xc8 } else { 0xc1 }; // xmm1,xmm0 / xmm0,xmm1
            if wide {
                bytes.extend([0x66, 0x0f, 0x2e, modrm]); // ucomisd
            } else {
                bytes.extend([0x0f, 0x2e, modrm]); // ucomiss
                bytes.push(0x90); // pad: keep f32/f64 sequence lengths equal
            }
            match operator {
                StateGuardOperator::Equal => {
                    bytes.extend([0xb0, 0x00]); // mov al, 0
                    bytes.extend([0x7a, 0x04]); // jp  +4 (unordered -> false)
                    bytes.extend([0x75, 0x02]); // jne +2 (not equal -> false)
                    bytes.extend([0xb0, 0x01]); // mov al, 1
                }
                StateGuardOperator::NotEqual => {
                    bytes.extend([0xb0, 0x01]); // mov al, 1
                    bytes.extend([0x7a, 0x04]); // jp  +4 (unordered -> TRUE)
                    bytes.extend([0x75, 0x02]); // jne +2 (not equal -> true)
                    bytes.extend([0xb0, 0x00]); // mov al, 0
                }
                StateGuardOperator::Greater
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::Less
                | StateGuardOperator::LessUnsigned => {
                    bytes.extend([0x0f, 0x97, 0xc0]); // seta (CF=0 && ZF=0)
                }
                StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::LessOrEqualUnsigned => {
                    bytes.extend([0x0f, 0x93, 0xc0]); // setae (CF=0)
                }
                _ => unreachable!(),
            }
            bytes.extend([0x44, 0x0f, 0xb6, 0xd0]); // movzx r10d, al
            return Ok(());
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime float binary operator `{operator:?}` is not implemented yet"
            )));
        }
    };
    bytes.extend([scalar_prefix, 0x0f, opcode, 0xc1]); // <op> xmm0, xmm1
    if directed_mxcsr.is_some() {
        append_directed_float_control_suffix(bytes);
    }
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
    }
    if guarded {
        bytes.extend(float_policy_guard_bytes(
            domain, operator, byte_size, false,
        )?);
    }
    Ok(())
}

/// Width of [`append_runtime_float_binary_operation`]: two operand moves
/// (5 each) + per operator -- the SSE op (4) + the result move (5) = 19 for
/// arithmetic/min/max/sqrt; comparisons are ucomis (4, f32 NOP-padded) +
/// setcc (3) or the equality branch pattern (8) + movzx (4). Identical for
/// f32 and f64 at every operator (the relocation-offset invariant). MUST
/// stay in lockstep with the emission.
fn runtime_float_binary_operation_width_with_domain(
    operator: StateGuardOperator,
    byte_size: usize,
    domain: ArithmeticDomain,
) -> usize {
    if operator == StateGuardOperator::FloatPair {
        return 6;
    }
    if operator == StateGuardOperator::MultiplyThenAdd {
        return float_multiply_then_add_bytes(byte_size, domain)
            .map(|bytes| bytes.len())
            .unwrap_or(0);
    }
    if is_integer_float_classification_predicate(operator) {
        return float_classification_predicate_bytes(operator, byte_size)
            .map(|bytes| bytes.len())
            .unwrap_or(0);
    }
    if operator == StateGuardOperator::FloatClassify {
        return float_classify_bytes(byte_size)
            .map(|bytes| bytes.len())
            .unwrap_or(0);
    }
    let policy_width = if float_policy_applies(operator, domain) {
        3 + float_policy_guard_bytes(domain, operator, byte_size, false)
            .map(|bytes| bytes.len())
            .unwrap_or(0)
    } else {
        0
    };
    policy_width + runtime_float_binary_operation_width_base(operator)
}

fn runtime_float_binary_operation_width_base(operator: StateGuardOperator) -> usize {
    match operator {
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
        | StateGuardOperator::SqrtTowardNegative => 19 + DIRECTED_FLOAT_CONTROL_WIDTH,
        StateGuardOperator::Equal | StateGuardOperator::NotEqual => 10 + 4 + 8 + 4,
        StateGuardOperator::IsNan
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => 10 + 4 + 3 + 4,
        _ => 19,
    }
}

pub(crate) fn runtime_binary_operation_width(
    operator: StateGuardOperator,
    byte_size: usize,
) -> usize {
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::And
        | StateGuardOperator::Or
        | StateGuardOperator::BitwiseAnd
        | StateGuardOperator::BitwiseOr
        | StateGuardOperator::BitwiseXor
        | StateGuardOperator::Subtract => 3,
        StateGuardOperator::Multiply => 4,
        // cmp (3) + cmov (4), same at 32-bit or 64-bit.
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => 7,
        // signed 32-bit: mov(3)+cdq(1)+idiv(3)+mov(3)=10; signed 64-bit: cqo(2)=11.
        // Narrow signed (i8/i16) prepends two movsx (8) to sign-extend the operands
        // to the 32-bit op width; see append_integer_divide_modulo_core.
        StateGuardOperator::Divide | StateGuardOperator::Modulo => {
            let sign_extend = if byte_size <= 2 { 8 } else { 0 };
            sign_extend + if byte_size <= 4 { 10 } else { 11 }
        }
        // unsigned: mov(3)+xor edx,edx(2)+div(3)+mov(3)=11 at either size.
        StateGuardOperator::DivideUnsigned | StateGuardOperator::ModuloUnsigned => 11,
        // mov c-reg, r11 (3) + shift r10, cl (3), same width at either size.
        StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => 6,
        // cmp (3; 4 with the 0x66 prefix at 2-byte width) + setcc (3) + movzx (4).
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
            if byte_size == 2 {
                11
            } else {
                10
            }
        }
        _ => 0,
    }
}
