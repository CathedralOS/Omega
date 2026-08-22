use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

use super::{
    append_add_constant_to_x_register, append_fixed_width_address_from_x_offset,
    append_fixed_width_load_x_from_x_offset, append_load_data_from_x_offset,
    append_runtime_frame_base_index_target_address_with_index_width,
    append_runtime_frame_fixed_index_target_address,
    append_runtime_frame_index_target_address_with_index_width, append_runtime_storage_load,
    append_scale_x_register_by_constant, comparison, conversion, scalar_writes,
};
use crate::aarch64::primitives::{
    append_unsigned_immediate, append_unsigned_immediate_padded,
    encode_add_page_offset_placeholder, encode_add_x_immediate, encode_add_x_register,
    encode_adrp_placeholder, encode_and_w_low_ones, encode_and_w_top_bit, encode_and_x_low_ones,
    encode_and_x_register, encode_and_x_top_bit, encode_asrv_w_register, encode_asrv_x_register,
    encode_brk, encode_cbz_x, encode_compare_w_immediate, encode_compare_w_register,
    encode_compare_x_immediate, encode_compare_x_register, encode_conditional_branch_equal,
    encode_conditional_branch_greater, encode_conditional_branch_greater_or_equal,
    encode_conditional_branch_higher, encode_conditional_branch_higher_or_same,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_lower, encode_conditional_branch_lower_or_same,
    encode_conditional_branch_no_overflow, encode_conditional_branch_not_equal, encode_csel_x,
    encode_csinv_x, encode_eor_x_register, encode_float_add, encode_float_compare,
    encode_float_conditional_select, encode_float_divide, encode_float_fused_multiply_add,
    encode_float_move_from_gpr, encode_float_move_to_gpr, encode_float_multiply, encode_float_sqrt,
    encode_float_subtract, encode_load_byte_w_from_x, encode_load_byte_w_post_increment,
    encode_load_w_from_x, encode_load_x_from_x, encode_lsl_x_immediate, encode_lslv_w_register,
    encode_lslv_x_register, encode_lsr_x_immediate, encode_lsrv_w_register, encode_lsrv_x_register,
    encode_move_w_register, encode_move_x_register, encode_movz, encode_movz_w,
    encode_msub_w_register, encode_msub_x_register, encode_mul_x_register, encode_orr_x_register,
    encode_read_fpcr, encode_sdiv_w_register, encode_sdiv_x_register, encode_sign_extend_byte_to_w,
    encode_sign_extend_byte_to_x, encode_sign_extend_halfword_to_w,
    encode_sign_extend_halfword_to_x, encode_sign_extend_word_to_x, encode_sub_x_register,
    encode_subs_x_immediate, encode_udiv_w_register, encode_udiv_x_register,
    encode_unconditional_branch, encode_write_fpcr, encode_zero_extend_byte_to_w,
    encode_zero_extend_halfword_to_w,
};

pub(super) fn append_runtime_value_operand(
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
            let container_bytes = scalar_writes::validate_runtime_bit_field_fragment(fragment)?;
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
            append_unsigned_immediate_padded(
                bytes,
                21,
                scalar_writes::bit_width_mask(fragment.width)?,
            );
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
            scalar_writes::append_saturating_trapping_arithmetic(
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
            scalar_writes::append_saturating_signed_divide_modulo(
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
        conversion::append_runtime_convert_operation(
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
        crate::aarch64::widths::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
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
        crate::aarch64::widths::runtime_text_equals_operand_width(),
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
    literal: &[u8],
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
    let literal_bytes = literal;
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
        crate::aarch64::widths::runtime_text_equals_literal_operand_width(
            runtime_value_operands,
            place,
            literal
        ),
        "text-equals-literal operand encoder length must match its width"
    );
    Ok(())
}

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
pub(super) fn append_runtime_binary_operation_with_domain(
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
            StateGuardOperator::MultiplyThenAdd
                | StateGuardOperator::FusedMultiplyAdd
                | StateGuardOperator::FusedMultiplyAddTowardZero
                | StateGuardOperator::FusedMultiplyAddTowardPositive
                | StateGuardOperator::FusedMultiplyAddTowardNegative
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
pub(super) fn float_policy_guard_bytes(
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
            | StateGuardOperator::SubtractTowardZero
            | StateGuardOperator::SubtractTowardPositive
            | StateGuardOperator::SubtractTowardNegative
            | StateGuardOperator::Multiply
            | StateGuardOperator::MultiplyTowardZero
            | StateGuardOperator::MultiplyTowardPositive
            | StateGuardOperator::MultiplyTowardNegative
            | StateGuardOperator::MultiplyThenAdd
            | StateGuardOperator::FusedMultiplyAdd
            | StateGuardOperator::FusedMultiplyAddTowardZero
            | StateGuardOperator::FusedMultiplyAddTowardPositive
            | StateGuardOperator::FusedMultiplyAddTowardNegative
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

/// F8c count guard: `cmp count, #width ; b.lo +8 ; brk #0` -- a TRAPPING
/// shift's out-of-range count traps before the shift runs. 12 bytes; the
/// width fns add SHIFT_COUNT_TRAP_GUARD_WIDTH in lockstep.
pub(super) fn append_shift_count_trap_guard(
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

pub(super) fn append_runtime_binary_operation(
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

pub(super) fn append_runtime_float_binary_operation(
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
    if matches!(
        operator,
        StateGuardOperator::FusedMultiplyAdd
            | StateGuardOperator::FusedMultiplyAddTowardZero
            | StateGuardOperator::FusedMultiplyAddTowardPositive
            | StateGuardOperator::FusedMultiplyAddTowardNegative
    ) {
        // x9 was populated by the structural FloatPair. FMADD performs the
        // multiply and add with one final rounding; do not split this into
        // FMUL/FADD or reuse the distinct multiply-then-add arm.
        bytes.extend(encode_float_move_from_gpr(byte_size, 0, left_register)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 1, 9)?);
        bytes.extend(encode_float_move_from_gpr(byte_size, 2, right_register)?);
        let directed_fpcr = match operator {
            StateGuardOperator::FusedMultiplyAddTowardPositive => Some(0x0040_0000),
            StateGuardOperator::FusedMultiplyAddTowardNegative => Some(0x0080_0000),
            StateGuardOperator::FusedMultiplyAddTowardZero => Some(0x00c0_0000),
            _ => None,
        };
        if let Some(fpcr) = directed_fpcr {
            bytes.extend(encode_read_fpcr(13));
            append_unsigned_immediate(bytes, 12, fpcr);
            bytes.extend(encode_write_fpcr(12));
        }
        bytes.extend(encode_float_fused_multiply_add(byte_size, 0, 0, 1, 2)?);
        if directed_fpcr.is_some() {
            bytes.extend(encode_write_fpcr(13));
        }
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
        StateGuardOperator::AddTowardPositive
        | StateGuardOperator::SubtractTowardPositive
        | StateGuardOperator::MultiplyTowardPositive
        | StateGuardOperator::DivideTowardPositive
        | StateGuardOperator::SqrtTowardPositive => Some(0x0040_0000),
        StateGuardOperator::AddTowardNegative
        | StateGuardOperator::SubtractTowardNegative
        | StateGuardOperator::MultiplyTowardNegative
        | StateGuardOperator::DivideTowardNegative
        | StateGuardOperator::SqrtTowardNegative => Some(0x0080_0000),
        StateGuardOperator::AddTowardZero
        | StateGuardOperator::SubtractTowardZero
        | StateGuardOperator::MultiplyTowardZero
        | StateGuardOperator::DivideTowardZero
        | StateGuardOperator::SqrtTowardZero => Some(0x00c0_0000),
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
        StateGuardOperator::Subtract
        | StateGuardOperator::SubtractTowardZero
        | StateGuardOperator::SubtractTowardPositive
        | StateGuardOperator::SubtractTowardNegative => {
            bytes.extend(encode_float_subtract(byte_size, 0, 0, 1)?);
            if directed_rounding.is_some() {
                bytes.extend(encode_write_fpcr(13));
            }
            guard(bytes)?;
        }
        StateGuardOperator::Multiply
        | StateGuardOperator::MultiplyTowardZero
        | StateGuardOperator::MultiplyTowardPositive
        | StateGuardOperator::MultiplyTowardNegative => {
            bytes.extend(encode_float_multiply(byte_size, 0, 0, 1)?);
            if directed_rounding.is_some() {
                bytes.extend(encode_write_fpcr(13));
            }
            guard(bytes)?;
        }
        StateGuardOperator::Divide
        | StateGuardOperator::DivideTowardZero
        | StateGuardOperator::DivideTowardPositive
        | StateGuardOperator::DivideTowardNegative => {
            bytes.extend(encode_float_divide(byte_size, 0, 0, 1)?);
            if directed_rounding.is_some() {
                bytes.extend(encode_write_fpcr(13));
            }
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
        StateGuardOperator::Sqrt
        | StateGuardOperator::SqrtTowardZero
        | StateGuardOperator::SqrtTowardPositive
        | StateGuardOperator::SqrtTowardNegative => {
            bytes.extend(encode_float_sqrt(byte_size, 0, 1)?);
            if directed_rounding.is_some() {
                bytes.extend(encode_write_fpcr(13));
            }
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
            bytes.extend(comparison::encode_conditional_branch_for_operator_bytes(
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
