use crate::Aarch64CallOperand;
use crate::Aarch64CallOperand::*;
use omega_calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

pub fn host_call_sequence_width(operands: &[Aarch64CallOperand]) -> usize {
    host_call_sequence_width_from_operands(operands.iter().copied())
}

pub fn syscall_sequence_width(operands: &[Aarch64CallOperand], syscall_number: u32) -> usize {
    syscall_sequence_width_from_operands(operands.iter().copied(), syscall_number)
}

pub fn host_call_sequence_width_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand>,
) -> usize {
    operands
        .map(|operand| operand_width(&operand))
        .sum::<usize>()
        + 4
}

pub fn host_call_stack_prefix_width_for_placements(
    placements: &[ValuePlacement],
    argument_count: usize,
) -> usize {
    let has_stack = placements
        .iter()
        .flat_map(|placement| &placement.locations)
        .any(|location| {
            matches!(
                location,
                ValueLocation::Stack { .. } | ValueLocation::Indirect { .. }
            )
        });
    usize::from(has_stack) * 4
        + placements
            .iter()
            .take(argument_count)
            .map(|placement| {
                placement
                    .locations
                    .iter()
                    .map(|location| match location {
                        ValueLocation::Stack { .. }
                            if !matches!(
                                placement.shape.class,
                                omega_calling_conventions::ValueClass::HomogeneousFloatAggregate {
                                    ..
                                }
                            ) =>
                        {
                            4
                        }
                        ValueLocation::Indirect {
                            pointer,
                            copy_stack_byte_offset: Some(copy_stack_byte_offset),
                            byte_size,
                            ..
                        } => {
                            let copy_stores = aggregate_copy_fragment_count(usize::from(*byte_size)) * 4;
                            let pointer_address = add_constant_width(*copy_stack_byte_offset as usize).max(4);
                            let pointer_store = usize::from(matches!(
                                pointer,
                                IndirectPointerLocation::Stack { .. }
                            )) * 4;
                            copy_stores + pointer_address + pointer_store
                        }
                        _ => 0,
                    })
                    .sum::<usize>()
            })
            .sum::<usize>()
}

pub fn host_call_stack_total_width_for_placements(placements: &[ValuePlacement]) -> usize {
    host_call_stack_prefix_width_for_placements(placements, placements.len())
        + usize::from(
            placements
                .iter()
                .flat_map(|placement| &placement.locations)
                .any(|location| {
                    matches!(
                        location,
                        ValueLocation::Stack { .. } | ValueLocation::Indirect { .. }
                    )
                }),
        ) * 4
}

fn aggregate_copy_fragment_count(byte_count: usize) -> usize {
    byte_count / 8 + (byte_count % 8).count_ones() as usize
}

pub fn syscall_sequence_width_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand>,
    syscall_number: u32,
) -> usize {
    operands
        .map(|operand| operand_width(&operand))
        .sum::<usize>()
        + unsigned_immediate_width(u64::from(syscall_number))
        + 4
}

pub fn function_enter_width() -> usize {
    40
}

pub fn return_width() -> usize {
    36
}

pub fn machine_halt_width() -> usize {
    4
}

pub fn return_register_integer_write_width() -> usize {
    4
}

pub fn runtime_storage_copy_to_return_register_width(
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    // adrp+add (8) + scalar load into w0/x0 + sign extension for narrow operands
    // (SXTB/SXTH, 4) so a negative i8/i16 terminal survives the widening read.
    let extend_width = if matches!(byte_size, 1 | 2) { 4 } else { 0 };
    8 + load_data_offset_width(byte_offset, byte_size) + extend_width
}

pub fn dispatch_loop_enter_width() -> usize {
    4
}

pub fn dispatch_case_enter_width() -> usize {
    8
}

pub fn dispatch_state_write_width() -> usize {
    8
}

pub fn dispatch_case_leave_width() -> usize {
    4
}

pub fn dispatch_guard_compare_static_width(
    byte_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    // adrp+add (8) + guard load + [SXTB/SXTH for narrow operands (4)] + expected
    // materialization (padded W = 8, padded X = 16) + [2 FMOVs for floats (8)]
    // + compare (CMP or FCMP, 4) + conditional branch (4).
    let extend_width = if !is_float && matches!(byte_size, 1 | 2) {
        4
    } else {
        0
    };
    let materialize_width = if byte_size == 8 { 16 } else { 8 };
    let float_move_width = if is_float { 8 } else { 0 };
    16 + extend_width
        + materialize_width
        + float_move_width
        + load_data_offset_width(byte_offset, byte_size)
}

pub fn runtime_text_literal_compare_width(literal: &[u8]) -> usize {
    8 + literal.len() * 12 + runtime_text_input_delimiter_check_width()
}

pub fn runtime_text_storage_compare_width(source_offset: usize) -> usize {
    // adrp pairs (16) + descriptor loads + the padded literal-length
    // immediate (8) + the fixed 22-instruction compare body (88).
    112 + runtime_text_descriptor_load_pair_width(source_offset)
}

pub fn runtime_storage_compare_width(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    // Float adds two `FMOV` (GPR -> FP) instructions (8 bytes) before the FCMP.
    // 2-byte integer operands add two `SXTH` instructions before the compare.
    let float_move_width = if is_float { 8 } else { 0 };
    let extend_width = if !is_float && byte_size == 2 { 8 } else { 0 };
    24 + float_move_width
        + extend_width
        + load_data_offset_width(left_offset, byte_size)
        + load_data_offset_width(right_offset, byte_size)
}

pub fn runtime_storage_value_compare_width(byte_offset: usize, byte_size: usize) -> usize {
    // adrp+add (8) + load + [SXTB/SXTH for narrow operands (4)] + expected
    // materialization (padded W = 8, padded X = 16) + compare (4) + branch (4).
    let extend_width = if matches!(byte_size, 1 | 2) { 4 } else { 0 };
    let materialize_width = if byte_size == 8 { 16 } else { 8 };
    16 + extend_width + materialize_width + load_data_offset_width(byte_offset, byte_size)
}

pub fn runtime_value_compare_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    // Narrow (1/2-byte) compares normalize both registers to the compare
    // width first (one SXT/UXT per side).
    let narrow_normalization = if matches!(byte_size, 1 | 2) { 8 } else { 0 };
    runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + narrow_normalization
        + 8
}

pub(in crate::aarch64) fn runtime_text_input_delimiter_check_width() -> usize {
    32
}

pub fn runtime_text_literal_write_width(literal: &[u8]) -> usize {
    8 + literal.len() * 8
}

pub fn runtime_text_literal_segment_write_width(literal: &[u8]) -> usize {
    runtime_text_literal_write_width(literal)
}

pub fn runtime_text_stored_suffix_append_width(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> usize {
    48 + runtime_text_descriptor_load_pair_width(source_offset)
        + add_constant_width(buffer_offset)
        + runtime_text_descriptor_store_pair_width(target_offset)
        + add_constant_width(length_delta)
}

pub fn runtime_text_stored_place_append_width(source_offset: usize, target_offset: usize) -> usize {
    60 + load_data_offset_width(target_offset + 8, 8)
        + runtime_text_descriptor_load_pair_width(source_offset)
        + runtime_text_descriptor_store_pair_width(target_offset)
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    source_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 68
        + runtime_text_descriptor_load_pair_width(source_offset)
}

pub fn runtime_text_stored_place_append_to_runtime_frame_base_indexed_width(
    source_offset: usize,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 68
        + runtime_text_descriptor_load_pair_width(source_offset)
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    60 + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(field_byte_offset)
        + load_data_offset_width(8, 8)
        + runtime_text_descriptor_load_pair_width(source_offset)
        + runtime_text_descriptor_store_pair_width(0)
}

pub fn runtime_text_literal_append_width(target_offset: usize, literal: &[u8]) -> usize {
    24 + load_data_offset_width(target_offset + 8, 8)
        + runtime_text_descriptor_store_pair_width(target_offset)
        + add_constant_width(literal.len())
        + literal.len() * 8
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    24 + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(field_byte_offset)
        + load_data_offset_width(8, 8)
        + runtime_text_descriptor_store_pair_width(0)
        + add_constant_width(literal.len())
        + literal.len() * 8
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 28
        + add_constant_width(literal.len())
        + literal.len() * 8
}

pub fn runtime_text_literal_append_to_runtime_frame_base_indexed_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 28
        + add_constant_width(literal.len())
        + literal.len() * 8
}

pub fn runtime_text_buffer_materialize_width(target_offset: usize) -> usize {
    44 + runtime_text_descriptor_load_pair_width(target_offset)
        + runtime_text_descriptor_store_pair_width(target_offset)
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_text_buffer_materialize_to_runtime_frame_indexed_with_index_region_width(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        element_byte_size,
        field_byte_offset,
    )
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_with_index_region_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_indexed_string_data_address_offset_with_index_region(
        index_region,
        element_byte_size,
        field_byte_offset,
    ) + 52
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_base_indexed_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 52
}

pub const fn runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed_width() -> usize {
    8 + 36 + 52
}

pub const fn runtime_text_frame_base_double_indexed_materialize_buffer_address_offset() -> usize {
    8 + 36 + 12
}

pub fn runtime_text_literal_append_to_runtime_frame_base_double_indexed_width(
    literal: &[u8],
) -> usize {
    8 + 36 + 28 + add_constant_width(literal.len()) + literal.len() * 8
}

pub const fn runtime_text_frame_base_double_indexed_literal_append_buffer_address_offset() -> usize
{
    8 + 36 + 4
}

pub fn runtime_text_stored_place_append_to_runtime_frame_base_double_indexed_width(
    source_offset: usize,
) -> usize {
    8 + 36 + 68 + runtime_text_descriptor_load_pair_width(source_offset)
}

pub const fn runtime_text_frame_base_double_indexed_stored_place_buffer_address_offset() -> usize {
    8 + 36 + 8
}

pub const fn runtime_text_frame_base_double_indexed_stored_place_source_address_offset() -> usize {
    8 + 36 + 24
}

pub fn runtime_text_indexed_literal_append_buffer_address_offset(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 4
}

pub fn runtime_text_frame_base_indexed_literal_append_buffer_address_offset(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 4
}

pub fn runtime_text_stored_place_pointee_source_address_offset(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    16 + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(field_byte_offset)
        + load_data_offset_width(8, 8)
        + 8
}

pub fn runtime_text_indexed_stored_place_buffer_address_offset(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 8
}

pub fn runtime_text_frame_base_indexed_stored_place_buffer_address_offset(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 8
}

pub fn runtime_text_indexed_stored_place_source_address_offset(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 24
}

pub fn runtime_text_frame_base_indexed_stored_place_source_address_offset(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 24
}

pub fn runtime_text_indexed_buffer_materialize_buffer_address_offset(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 12
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    40 + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(field_byte_offset)
        + runtime_text_descriptor_load_pair_width(0)
        + runtime_text_descriptor_store_pair_width(0)
}

pub fn runtime_machine_integer_write_width(byte_offset: usize, byte_size: usize) -> usize {
    8 + add_constant_width(byte_offset) + runtime_store_data_width(byte_size)
}

pub(in crate::aarch64) fn bit_fragment_container_bytes(
    fragment: &omega_target_operations::RuntimeBitFieldFragment,
) -> Result<usize, Diagnostic> {
    match fragment.container_width_bits {
        8 => Ok(1),
        16 => Ok(2),
        32 => Ok(4),
        64 => Ok(8),
        width => Err(Diagnostic::error(format!(
            "AArch64 bit-field container width `{width}` is not 8, 16, 32, or 64"
        ))),
    }
}

pub fn runtime_storage_bit_field_write_width(
    base_byte_offset: usize,
    fragments: &[omega_target_operations::RuntimeBitFieldFragment],
) -> Result<usize, Diagnostic> {
    if fragments.is_empty() {
        return Err(Diagnostic::error(
            "AArch64 bit-field write requires at least one fragment",
        ));
    }
    let mut width = 8;
    for fragment in fragments {
        let container_bytes = bit_fragment_container_bytes(fragment)?;
        let offset = base_byte_offset
            .checked_add(fragment.container_byte_offset)
            .ok_or_else(|| Diagnostic::error("AArch64 bit-field offset overflows"))?;
        width += load_data_offset_width(offset, container_bytes)
            + 16
            + 4
            + 16
            + 4
            + store_data_offset_width(offset, container_bytes);
    }
    Ok(width)
}

fn runtime_bit_field_operand_width(
    base_byte_offset: usize,
    fragments: &[omega_target_operations::RuntimeBitFieldFragment],
) -> usize {
    if fragments.is_empty() {
        return 0;
    }
    let mut width = 12; // relocated base pair + zero destination
    for fragment in fragments {
        let Ok(container_bytes) = bit_fragment_container_bytes(fragment) else {
            return 0;
        };
        let Some(offset) = base_byte_offset.checked_add(fragment.container_byte_offset) else {
            return 0;
        };
        width += load_data_offset_width(offset, container_bytes)
            + usize::from(fragment.destination_lsb != 0) * 4
            + 16
            + 4
            + usize::from(fragment.source_lsb != 0) * 4
            + 4;
    }
    width
}

pub fn runtime_pointee_integer_write_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    // adrp+add (8) + pointer load (4) + value materialization (padded W = 8,
    // padded X = 16) + sized store (4).
    let width = match byte_size {
        1 | 2 | 4 => 24,
        8 => 32,
        _ => 0,
    };

    width + add_constant_width(pointer_byte_offset) + add_constant_width(field_byte_offset)
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_convert_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
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
    // `adrp x16 + add x16` (8) — target base, held across source evaluation —
    // then load the source into x17, convert it in place, and store the result.
    8 + runtime_value_operand_width(runtime_value_operands, source)
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
        + runtime_result_write_width(target_offset, target_byte_size)
}

/// Width of the in-register conversion sequence (see
/// `runtime_storage::conversion::append_runtime_convert_operation`). The source bits start in
/// x17 and the converted result is left in x17.
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
        // int -> float: optional narrow signed extension + SCVTF/UCVTF +
        // FMOV result back to GPR.
        (false, true) => {
            8 + if source_signed && matches!(source_byte_size, 1 | 2) {
                4
            } else {
                0
            }
        }
        // float -> int: FMOV bits into FP bank (4) + [F4 Trapping value
        // guard] + FCVTZS (4).
        (true, false) => {
            8 + if trapping {
                super::runtime_storage::FLOAT_TO_INT_TRAP_GUARD_WIDTH
            } else {
                0
            } + if saturating {
                super::runtime_storage::float_to_narrow_int_saturating_width(
                    target_byte_size,
                    target_signed,
                )
            } else {
                0
            }
        }
        (true, true) => {
            if source_byte_size == target_byte_size {
                0 // same precision: bits already in x17.
            } else {
                // FMOV into FP bank (4) + FCVT precision change (4) + FMOV back (4).
                12
            }
        }
        (false, false) => {
            // Every narrow source extends when widening (one SXT/UXT); a
            // 4-byte source extends only when signed.
            if target_byte_size > source_byte_size
                && (matches!(source_byte_size, 1 | 2) || (source_byte_size == 4 && source_signed))
            {
                4
            } else {
                0
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> usize {
    use psi_numerics::arithmetic::ArithmeticDomain;
    let indexed_operand_restore_width = if runtime_value_operands.frame_indexed(left).is_some()
        || runtime_value_operands.frame_indexed(right).is_some()
        || runtime_value_operands.frame_base_indexed(left).is_some()
        || runtime_value_operands.frame_base_indexed(right).is_some()
    {
        4
    } else {
        0
    };

    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );

    let saturating_signed_divide_modulo = domain == ArithmeticDomain::Saturating
        && target_signed
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        );
    let operation_width = if is_float {
        runtime_float_binary_operation_width_with_domain(
            operator,
            super::runtime_storage::runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
            domain,
        )
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add
                | StateGuardOperator::Subtract
                | StateGuardOperator::Multiply
                | StateGuardOperator::ShiftLeft
        )
    {
        saturating_trapping_arithmetic_width(
            domain,
            operator,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )
    } else if saturating_signed_divide_modulo {
        saturating_signed_divide_modulo_width(
            byte_size,
            matches!(operator, StateGuardOperator::Modulo),
        )
    } else {
        runtime_binary_operation_width_with_domain(
            operator,
            super::runtime_storage::runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
            domain,
        )
    };

    8 + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + operation_width
        + indexed_operand_restore_width
        + runtime_result_write_width(target_offset, byte_size)
}

/// Byte count of [`super::runtime_storage::append_saturating_trapping_arithmetic`]
/// — the wide op + two range-checked clamp/trap blocks. MUST stay in lockstep.
fn saturating_trapping_arithmetic_width(
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> usize {
    use psi_numerics::arithmetic::ArithmeticDomain;
    if byte_size == 8 {
        // 64-bit shl: the recovery witness -- mov save (4) [+ movz/movk MIN
        // (8) for saturating-signed] + lslv (4) + asrv/lsrv (4) + cmp (4)
        // + b.ne (4) + cmp #64 (4) + b.lo (4) + cmp #0 (4) + b.eq (4)
        // + fixup (sat-signed cmp+csinv 8; sat-unsigned padded MAX 16;
        // trapping brk 4).
        if operator == StateGuardOperator::ShiftLeft {
            // F8c: Trapping prepends the count trap guard (cmp + b.lo + brk
            // = 12) before the recovery witness.
            return match (domain, target_signed) {
                (ArithmeticDomain::Saturating, true) => 36 + 8 + 8,
                (ArithmeticDomain::Saturating, false) => 36 + 16,
                _ => 12 + 36 + 4,
            };
        }
        // 64-bit multiply: the MULH high-half witness.
        if matches!(operator, StateGuardOperator::Multiply) {
            return match (domain, target_signed) {
                // smulh + eor + movz/movk MIN (8) + mul + cmp-asr + b.eq +
                // cmp + csinv.
                (psi_numerics::arithmetic::ArithmeticDomain::Saturating, true) => 36,
                // umulh + mul + cmp + csinv.
                (psi_numerics::arithmetic::ArithmeticDomain::Saturating, false) => 16,
                // (s/u)mulh + mul + cmp(+asr) + b.eq + brk.
                _ => 20,
            };
        }
        return match (domain, target_signed) {
            // movz+movk MIN (8) + adds/subs (4) + b.vc (4) + csinv (4).
            (ArithmeticDomain::Saturating, true) => 20,
            // adds/subs (4) + csinv/csel (4).
            (ArithmeticDomain::Saturating, false) => 8,
            // adds/subs (4) + b.cond (4) + brk (4).
            _ => 12,
        };
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        // Unsupported widths error during emission; the wide op (4) is a
        // harmless placeholder for the pre-error `Vec::with_capacity`.
        return 4;
    }
    // Narrow shl: [F8c Trapping count trap guard (12)] + [SXT dest (4,
    // signed)] + count cap (padded w 16 + cmp 4 + csel 4) + lslv (4) + the
    // bound checks (28 each: padded bound 16 + cmp 4 + b.cond 4 + mov/brk 4)
    // -- both bounds for signed, the single unsigned upper bound otherwise.
    if operator == StateGuardOperator::ShiftLeft {
        let count_guard = if domain == ArithmeticDomain::Trapping {
            12
        } else {
            0
        };
        let value_extend = if target_signed && !left_is_wide_immediate {
            4
        } else {
            0
        };
        return count_guard
            + if target_signed {
                value_extend + 28 + 56
            } else {
                28 + 28
            };
    }
    // One SXTB/SXTH/SXTW sign-extend (4 bytes) per SIGNED NON-IMMEDIATE
    // operand -- immediates are already their true wide value and skipping
    // keeps them uncorrupted (MUST mirror the emission's per-side skip).
    let sign_extend = if target_signed {
        (if left_is_wide_immediate { 0 } else { 4 }) + (if right_is_wide_immediate { 0 } else { 4 })
    } else {
        0
    };
    // Wide op: ADD/SUB/MUL Xd,Xn,Xm.
    let wide_op = 4;
    // Each bound check: MOVZ+MOVK*3 bound (16) + CMP (4) + b.cond (4)
    // + (MOV clamp OR BRK trap) (4) = 28 bytes. Signed targets check both
    // bounds; unsigned overflow in ONE direction per operator (subtract
    // down, add/mul up), so they take a single check.
    let clamp_or_trap = if target_signed { 2 * 28 } else { 28 };
    sign_extend + wide_op + clamp_or_trap
}

/// Byte count of [`super::runtime_storage::append_saturating_signed_divide_modulo`]
/// — optionally sign-extends narrow inputs, then emits `x9 = -1` (16) + CMP
/// (4) + b.ne (4) + the special block + the unconditional branch (4) + the
/// normal block. MUST stay in lockstep.
fn saturating_signed_divide_modulo_width(byte_size: usize, want_remainder: bool) -> usize {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return 4;
    }
    // Prologue: narrow operands have two sign-extends (8); i64 operands are
    // already full-width. Then MOVZ+MOVK*3 for -1 (16) + CMP (4) + b.ne (4).
    let prologue = if byte_size == 8 { 0 } else { 8 } + 16 + 4 + 4;
    // Special block (divisor == -1): modulo = MOVZ 0 (4); divide = MUL (4) + MOVZ+
    // MOVK*3 MAX (16) + CMP (4) + b.le (4) + MOV clamp (4) = 32 for narrow
    // values. i64 divide instead uses NEG (4) + the two-instruction MIN
    // materialization (8) + CMP (4) + CSINV (4) = 20.
    let special = if want_remainder {
        4
    } else if byte_size == 8 {
        20
    } else {
        32
    };
    // Unconditional branch past the normal block.
    let branch_over = 4;
    // Normal block: divide = SDIV (4); modulo = SDIV (4) + MSUB (4) = 8.
    let normal = if want_remainder { 8 } else { 4 };
    prologue + special + branch_over + normal
}

/// Width of the float binary-operation sequence: two `FMOV` from GPR (4 bytes
/// each), then per operator -- the scalar FP op (4) + `FMOV` back (4); min/max
/// FCMP+FCSEL (8) + `FMOV` back (4); COMPARISONS FCMP + MOVZ + B.cond + MOVZ
/// (16) with NO trailing FMOV (the 0/1 result is already in the GPR). MUST
/// stay in lockstep with `append_runtime_float_binary_operation`.
/// Width twin of `append_runtime_binary_operation_with_domain`: the plain op
/// width plus the domain shift fix -- F8b WRAPPING masks the COUNT (the
/// sub-word AND, 4; widths 4/8 ride the W/X forms' native masking, 0; the
/// Wrapping `<<` W/X emission is one instruction, the same 4 as the plain
/// arm), while Saturating/Trapping `>>`/`>>>` keep the floor-semantics count
/// fixes (CMP #width + CSINV/CSEL = 8) until F8c. MUST stay in lockstep.
pub(in crate::aarch64) fn runtime_binary_operation_width_with_domain(
    operator: StateGuardOperator,
    byte_size: usize,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) -> usize {
    let wrapping = domain == psi_numerics::arithmetic::ArithmeticDomain::Wrapping;
    let trapping = domain == psi_numerics::arithmetic::ArithmeticDomain::Trapping;
    let non_exact = domain != psi_numerics::arithmetic::ArithmeticDomain::Exact;
    runtime_binary_operation_width(operator, byte_size)
        + if wrapping
            && matches!(
                operator,
                StateGuardOperator::ShiftLeft
                    | StateGuardOperator::ShiftRight
                    | StateGuardOperator::ShiftRightLogical
            )
        {
            if matches!(byte_size, 1 | 2) { 4 } else { 0 }
        } else if trapping
            && matches!(
                operator,
                StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical
            )
        {
            // F8c: the count trap guard (cmp + b.lo + brk).
            super::runtime_storage::SHIFT_COUNT_TRAP_GUARD_WIDTH
        } else if non_exact
            && matches!(
                operator,
                StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical
            )
        {
            8
        } else {
            0
        }
}

/// F5 twin: the plain float op width plus the policy guard's bytes. The
/// guard length comes from the EMITTER itself (fixed-register call +
/// `.len()` -- the place-copy rung-2a one-source-of-truth discipline).
pub(in crate::aarch64) fn runtime_float_binary_operation_width_with_domain(
    operator: StateGuardOperator,
    byte_size: usize,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) -> usize {
    if operator == StateGuardOperator::FloatPair {
        return 8;
    }
    if operator == StateGuardOperator::MultiplyThenAdd {
        // fmov a + fmov b + fmul + fmov c + fadd + fmov result.
        return 24 + super::runtime_storage::float_policy_guard_width(operator, byte_size, domain);
    }
    if matches!(
        operator,
        StateGuardOperator::FusedMultiplyAdd
            | StateGuardOperator::FusedMultiplyAddTowardZero
            | StateGuardOperator::FusedMultiplyAddTowardPositive
            | StateGuardOperator::FusedMultiplyAddTowardNegative
    ) {
        // fmov a + fmov b + fmov c + fmadd + fmov result.
        let directed_control = if operator == StateGuardOperator::FusedMultiplyAdd {
            0
        } else {
            20
        };
        return 20
            + directed_control
            + super::runtime_storage::float_policy_guard_width(operator, byte_size, domain);
    }
    if matches!(
        operator,
        StateGuardOperator::IsFinite
            | StateGuardOperator::IsInfinite
            | StateGuardOperator::IsNormal
            | StateGuardOperator::IsSubnormal
    ) {
        return super::runtime_storage::float_classification_predicate_width(operator, byte_size);
    }
    if operator == StateGuardOperator::FloatClassify {
        return super::runtime_storage::float_classify_width(byte_size);
    }
    let guard = super::runtime_storage::float_policy_guard_width(operator, byte_size, domain);
    guard + runtime_float_binary_operation_width_base(operator)
}

fn runtime_float_binary_operation_width_base(operator: StateGuardOperator) -> usize {
    8 + match operator {
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
        | StateGuardOperator::SqrtTowardNegative => 20 + 4 + 4,
        StateGuardOperator::Max | StateGuardOperator::Min => 8 + 4,
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::IsNan
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => 16,
        _ => 4 + 4,
    }
}

pub fn runtime_pointee_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    12 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(
            operator,
            super::runtime_storage::runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )
        + runtime_result_write_width(0, byte_size)
}

pub fn runtime_pointee_operand_start_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    8 + add_constant_width(pointer_byte_offset)
        + runtime_load_data_width(8)
        + add_constant_width(field_byte_offset)
}

pub fn runtime_frame_indexed_integer_write_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + runtime_store_data_width(byte_size)
}

pub fn runtime_frame_indexed_operand_start_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            8
        } else {
            0
        }
}

pub fn runtime_frame_base_indexed_integer_write_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    runtime_frame_base_indexed_integer_write_with_index_region_width(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )
}

pub fn runtime_frame_base_indexed_integer_write_with_index_region_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    runtime_frame_base_index_setup_width_with_index_width(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine) * 8
        + runtime_store_data_width(byte_size)
}

fn runtime_frame_base_index_setup_width_with_index_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    16 + add_constant_width(base_byte_offset)
        + load_data_offset_width(index_offset, index_byte_size)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
}

pub fn runtime_frame_base_indexed_operand_start_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_index_setup_width_with_index_width(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage_width(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_base_index_setup_width_with_index_width(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        8
    } else {
        0
    } + if source_region == omega_target_operations::RuntimeStorageRegion::Machine
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        8
    } else {
        0
    } + load_data_offset_width(source_offset, byte_count)
        + 4
}

pub fn runtime_frame_base_indexed_machine_index_base_offset(base_byte_offset: usize) -> usize {
    12 + add_constant_width(base_byte_offset)
}

pub fn runtime_frame_base_indexed_operand_start_width_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_operand_start_width(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        8
    } else {
        0
    }
}

pub fn runtime_machine_indexed_integer_write_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    // Fixed 16 (adrp+add 8, mov x20 4, add x16,x16,x26 4) + region/offset-aware index
    // load + add-constant(base) + scale + add-constant(field) + the store. Small
    // offsets collapse to the historical 20 (Machine) / 28 (RuntimeFrame).
    16 + machine_index_load_width(index_region, index_offset, index_byte_size)
        + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + runtime_store_data_width(byte_size)
}

pub fn runtime_machine_indexed_integer_runtime_frame_address_offset(
    base_byte_offset: usize,
) -> usize {
    12 + add_constant_width(base_byte_offset)
}

pub fn runtime_frame_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(
            operator,
            super::runtime_storage::runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )
        + runtime_result_write_width(0, byte_size)
}

pub fn runtime_frame_base_indexed_binary_write_width(
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
) -> usize {
    runtime_frame_base_indexed_binary_write_with_index_region_width(
        runtime_value_operands,
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_frame_base_indexed_binary_write_with_index_region_width(
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
) -> usize {
    16 + add_constant_width(base_byte_offset)
        + load_data_offset_width(index_offset, index_byte_size)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine) * 8
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(
            operator,
            super::runtime_storage::runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )
        + runtime_result_write_width(0, byte_size)
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_machine_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    _index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    // The machine-index address helper differs from the frame-base one only
    // by the optional frame-index page pair (see
    // append_runtime_machine_index_target_address).
    (if _index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        8
    } else {
        0
    }) + runtime_frame_base_indexed_binary_write_width(
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
    )
}

pub fn runtime_machine_string_write_width(byte_length: usize) -> usize {
    24 + unsigned_immediate_width(byte_length as u64)
}

/// Width of the owned `[u8; N]` byte-carrier write (see
/// `runtime_storage::encode_runtime_machine_bounded_buffer_write`): `adrp`+`add`
/// for the base (8), the length immediate + its `str` (len word), then per
/// content byte a `movz` immediate + `strb`. Every element is a 4-byte AArch64
/// instruction, so the total is inherently 4-aligned -- unlike the x86_64 width
/// (variable-length instructions with inline immediate bytes) it previously
/// borrowed, which produced non-instruction-aligned branch distances.
pub fn runtime_machine_bounded_buffer_write_width(byte_offset: usize, literal: &[u8]) -> usize {
    8 + add_constant_width(byte_offset)
        + unsigned_immediate_width(literal.len() as u64)
        + 4
        + bounded_buffer_literal_bytes_width(literal)
}

/// Per-content-byte cost shared by the carrier write/append encoders: a `movz`
/// (or `movz`+`movk`s) materializing the byte plus one 4-byte store.
fn bounded_buffer_literal_bytes_width(literal: &[u8]) -> usize {
    literal
        .iter()
        .map(|byte| unsigned_immediate_width(u64::from(*byte)) + 4)
        .sum::<usize>()
}

fn bounded_buffer_literal_tail_width(literal: &[u8]) -> usize {
    unsigned_immediate_width(literal.len() as u64) + 4 + bounded_buffer_literal_bytes_width(literal)
}

pub fn runtime_frame_indexed_bounded_buffer_write_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_indexed_string_data_address_offset_with_index_region(
        index_region,
        element_byte_size,
        field_byte_offset,
    ) + bounded_buffer_literal_tail_width(literal)
}

pub fn runtime_frame_base_indexed_bounded_buffer_write_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_base_indexed_bounded_buffer_write_with_index_region_width(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        literal,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_frame_base_indexed_bounded_buffer_write_with_index_region_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + bounded_buffer_literal_tail_width(literal)
}

pub fn runtime_machine_indexed_bounded_buffer_write_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_machine_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + bounded_buffer_literal_tail_width(literal)
}

pub fn runtime_machine_double_indexed_bounded_buffer_write_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    literal: &[u8],
) -> usize {
    runtime_machine_double_indexed_string_data_address_offset(
        outer_index_region,
        inner_index_region,
    ) + bounded_buffer_literal_tail_width(literal)
}

pub fn runtime_frame_base_double_indexed_bounded_buffer_write_width(literal: &[u8]) -> usize {
    8 + 36 + bounded_buffer_literal_tail_width(literal)
}

fn bounded_buffer_literal_append_tail_width(literal: &[u8]) -> usize {
    20 + bounded_buffer_literal_bytes_width(literal)
}

pub fn runtime_frame_base_double_indexed_bounded_buffer_literal_append_width(
    literal: &[u8],
) -> usize {
    8 + 36 + bounded_buffer_literal_append_tail_width(literal)
}

pub fn runtime_frame_indexed_bounded_buffer_literal_append_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_indexed_string_data_address_offset_with_index_region(
        index_region,
        element_byte_size,
        field_byte_offset,
    ) + bounded_buffer_literal_append_tail_width(literal)
}

pub fn runtime_frame_base_indexed_bounded_buffer_literal_append_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region_width(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        literal,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + bounded_buffer_literal_append_tail_width(literal)
}

pub fn runtime_machine_indexed_bounded_buffer_literal_append_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    runtime_machine_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + bounded_buffer_literal_append_tail_width(literal)
}

pub fn runtime_machine_double_indexed_bounded_buffer_literal_append_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    literal: &[u8],
) -> usize {
    runtime_machine_double_indexed_string_data_address_offset(
        outer_index_region,
        inner_index_region,
    ) + bounded_buffer_literal_append_tail_width(literal)
}

/// Width of the owned-carrier write through a stored pointer (see
/// `runtime_storage::encode_runtime_pointee_bounded_buffer_write`): frame-base
/// `adrp`+`add` (8), the pointer load with its optional offset add, the optional
/// field-offset add, the length immediate + its store, then the per-byte stores.
pub fn runtime_pointee_bounded_buffer_write_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    8 + add_constant_width(pointer_byte_offset)
        + 4
        + add_constant_width(field_byte_offset)
        + unsigned_immediate_width(literal.len() as u64)
        + 4
        + bounded_buffer_literal_bytes_width(literal)
}

/// Width of the owned-carrier literal append (see
/// `runtime_storage::encode_runtime_machine_bounded_buffer_literal_append`):
/// machine-base `adrp`+`add` (8), the running-length load (4), the cursor adds
/// (`add x14, x16, #target+8` then `add x14, x14, x15`), the per-byte
/// post-increment stores, and the new-length add + store (8).
pub fn runtime_machine_bounded_buffer_literal_append_width(
    target_byte_offset: usize,
    literal: &[u8],
) -> usize {
    8 + 4
        + add_constant_width(target_byte_offset + 8)
        + 4
        + bounded_buffer_literal_bytes_width(literal)
        + 8
}

/// Width of the owned-carrier source append (see
/// `runtime_storage::encode_runtime_machine_bounded_buffer_source_append`):
/// machine-base `adrp`+`add` (8), an optional frame-base pair for a frame-local
/// source (8), the two length loads (8), the two cursor adds, the dst-cursor
/// register add + new-length add + store (12), and the fixed 20-byte copy loop.
pub fn runtime_machine_bounded_buffer_source_append_width(
    target_byte_offset: usize,
    source_byte_offset: usize,
    source_in_frame: bool,
) -> usize {
    8 + if source_in_frame { 8 } else { 0 }
        + 8
        + add_constant_width(source_byte_offset + 8)
        + add_constant_width(target_byte_offset + 8)
        + 12
        + 20
}

pub fn runtime_frame_string_write_width(byte_length: usize) -> usize {
    runtime_machine_string_write_width(byte_length)
}

pub fn runtime_pointee_string_write_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    28 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_frame_indexed_string_write_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    runtime_frame_indexed_string_write_width_with_index_region(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

pub fn runtime_frame_indexed_string_write_width_with_index_region(
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    runtime_frame_indexed_string_data_address_offset_with_index_region(
        index_region,
        element_byte_size,
        field_byte_offset,
    ) + 8
        + 4
        + unsigned_immediate_width(byte_length as u64)
        + 4
}

pub fn runtime_frame_indexed_string_data_address_offset(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_indexed_string_data_address_offset_with_index_region(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        element_byte_size,
        field_byte_offset,
    )
}

pub fn runtime_frame_indexed_string_data_address_offset_with_index_region(
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            8
        } else {
            0
        }
}

pub fn runtime_frame_base_indexed_string_write_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    runtime_frame_base_indexed_string_write_with_index_region_width(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_frame_base_indexed_string_write_with_index_region_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 16
        + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_frame_base_indexed_string_data_address_offset(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

pub fn runtime_frame_base_indexed_string_data_address_offset_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_base_indexed_operand_start_width_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

pub fn runtime_machine_indexed_string_write_width(
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    runtime_machine_indexed_string_write_width_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        0,
        8,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

pub fn runtime_machine_indexed_string_write_width_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    runtime_machine_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 8
        + 4
        + unsigned_immediate_width(byte_length as u64)
        + 4
}

pub fn runtime_machine_indexed_string_data_address_offset_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    16 + machine_index_load_width(index_region, index_offset, index_byte_size)
        + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
}

pub fn runtime_machine_indexed_string_runtime_frame_address_offset(
    base_byte_offset: usize,
) -> usize {
    // The machine-index address setup is adrp+add (8) + mov x20,x16 (4) +
    // add-constant(base), and THEN the frame-index page pair -- the same
    // layout the copy-from-machine-indexed offset below uses. This was 20,
    // which patched the index LOAD eight bytes past the frame adrp: the page
    // bits corrupted the load and the unrelocated adrp read its index from a
    // garbage page, so machine-indexed string writes landed nowhere (masked
    // for as long as String guards silently passed; see the slice-indexed
    // String guard canaries).
    12 + add_constant_width(base_byte_offset)
}

pub fn runtime_machine_indexed_string_data_address_offset(
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_machine_indexed_string_data_address_offset_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        0,
        8,
        element_byte_size,
        field_byte_offset,
    )
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
    base_byte_offset: usize,
) -> usize {
    12 + add_constant_width(base_byte_offset)
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    // Target adrp lands after the index-address setup (12) + `add x16,#base` + the
    // index LOAD (region- and offset-aware; a large index materializes) + scale +
    // `add x16,x16,x26` (4) + `add x16,#field`. MUST match the encoder / `..._width`.
    16 + machine_index_load_width(index_region, index_offset, index_byte_size)
        + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
}

pub fn runtime_storage_address_to_runtime_frame_write_width(
    source_offset: usize,
    target_offset: usize,
) -> usize {
    16 + add_constant_width(source_offset) + store_x_offset_width(target_offset)
}

pub fn runtime_storage_address_to_runtime_frame_target_frame_offset(source_offset: usize) -> usize {
    8 + add_constant_width(source_offset)
}

pub fn runtime_pointee_address_to_runtime_frame_write_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    16 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + store_x_offset_width(target_offset)
}

pub fn runtime_frame_indexed_address_to_runtime_frame_write_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    let machine_index_pair =
        if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            8
        } else {
            0
        };
    machine_index_pair +
    // Same `append_runtime_frame_index_target_address` prologue as the indexed
    // reads/writes (fixed-width descriptor + index loads), then a store of the
    // computed address. The old hand-summed `20 + …` predates the fixed-width
    // load helpers and under-planned the encoder by 40 bytes.
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + store_x_offset_width(target_offset)
}

pub fn runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    runtime_frame_fixed_index_setup_width(
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
    ) + store_x_offset_width(target_offset)
}

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_width(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region_width(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    runtime_frame_base_indexed_operand_start_width_with_index_region(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + store_x_offset_width(target_offset)
}

/// One shared frame base supplies the inline 2D array, both runtime indices,
/// and the destination reference slot: page pair + base copy + fixed double-
/// index address math + the final pointer store.
pub fn runtime_frame_base_double_indexed_address_to_runtime_frame_write_width(
    target_offset: usize,
) -> usize {
    8 + 4 + 36 + store_x_offset_width(target_offset)
}

/// A machine-rooted 2D element address always needs one machine page pair and
/// one frame page pair for the destination reference slot. The frame pair is
/// shared with frame-held indices when present and otherwise follows the fixed
/// double-index address program.
pub fn runtime_machine_double_indexed_address_to_runtime_frame_write_width(
    target_offset: usize,
) -> usize {
    8 + 8 + 36 + store_x_offset_width(target_offset)
}

pub fn runtime_machine_double_indexed_address_frame_base_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    if outer_index_region == frame || inner_index_region == frame {
        8
    } else {
        8 + 36
    }
}

/// Extra bytes the line-read's result-descriptor store spends when the String field's
/// offset is too large for the STR scaled immediate: the two stores (ptr@target_offset,
/// len@target_offset+8) go DIRECT when both fit (offset in the immediate = free);
/// otherwise the base is materialized ONCE (`append_add_x_constant`). MUST match the
/// conditional in `encode_runtime_text_line_read`. The x16 target adrp precedes this, so
/// its relocation offset is unchanged.
pub(in crate::aarch64) fn line_read_descriptor_store_extra(target_offset: usize) -> usize {
    if data_offset_encodable(target_offset + 8, 8) {
        0
    } else {
        add_constant_width(target_offset)
    }
}

/// Entry prologue argument store: the frame-base `adrp`+`add` pair (8) plus one
/// `str` (4). The store's scaled-immediate constraint is enforced by the
/// encoder (loud error), so the width is a constant.
pub fn entry_argument_register_write_width() -> usize {
    12
}

/// Frame-base `adrp`+`add`, one source load, and one destination store.
pub fn entry_stack_argument_write_width() -> usize {
    16
}

pub fn entry_indirect_argument_write_width(
    pointer: IndirectPointerLocation,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    let pointer_load = match pointer {
        IndirectPointerLocation::Register(_) => 0,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => load_data_offset_width(stack_byte_offset as usize + super::FUNCTION_FRAME_BYTES, 8),
    };
    let mut width = pointer_load + 8;
    let mut copied = 0usize;
    while copied < byte_size {
        let fragment = [8, 4, 2, 1]
            .into_iter()
            .find(|fragment| byte_size - copied >= *fragment)
            .expect("indirect entry copy has bytes remaining");
        width += load_data_offset_width(copied, fragment)
            + store_data_offset_width(byte_offset + copied, fragment);
        copied += fragment;
    }
    width
}

/// Byte offset of the runtime-frame `adrp` within an indirect entry copy.
/// A stack-passed pointer must first be loaded from the caller's argument area.
pub fn entry_indirect_argument_frame_base_offset(pointer: IndirectPointerLocation) -> usize {
    match pointer {
        IndirectPointerLocation::Register(_) => 0,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => load_data_offset_width(stack_byte_offset as usize + super::FUNCTION_FRAME_BYTES, 8),
    }
}

fn double_indexed_any_frame(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> bool {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    outer_index_region == frame || inner_index_region == frame
}

/// Base-pair widths of the double-indexed ops: the machine pair (8) plus the
/// shared frame pair (8) when any index is frame-resident.
fn double_indexed_base_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    8 + if double_indexed_any_frame(outer_index_region, inner_index_region) {
        8
    } else {
        0
    }
}

/// Width of the double-indexed read `grid[i][j] -> slot`: bases + the 36-byte
/// fixed address math + the element load + the relocated target pair + store.
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    double_indexed_base_width(outer_index_region, inner_index_region) + 36 + 4 + 8 + 4
}

/// Width of the double-indexed literal write `grid[i][j] = value`: bases +
/// address math + the value immediate (variable, AFTER all relocations) +
/// store.
pub fn runtime_machine_double_indexed_integer_write_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    value: i64,
) -> usize {
    double_indexed_base_width(outer_index_region, inner_index_region)
        + 36
        + unsigned_immediate_width(value as u64)
        + 4
}

/// The frame-rooted twin uses one shared frame pair for its collection and
/// both runtime index slots, followed by the fixed address program.
pub fn runtime_frame_base_double_indexed_integer_write_width(value: i64) -> usize {
    8 + 36 + unsigned_immediate_width(value as u64) + 4
}

/// Width of the double-indexed storage write `grid[i][j] = slot`: bases (the
/// shared frame pair also serves a frame-resident source) + the source load +
/// the 36-byte address math + the element store.
pub fn runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let any_frame =
        source_region == frame || outer_index_region == frame || inner_index_region == frame;
    8 + if any_frame { 8 } else { 0 } + 4 + 36 + 4
}

/// Width of the frame-base single-indexed copy: frame pair + the x24 stash +
/// the same-region fixed-shape element address + one load/store pair per
/// chunk. The chunk split mirrors `for_each_runtime_copy_chunk` with the
/// TARGET offset as its base (chunk sizing accounts for target alignment).
pub fn runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame_width(
    target_offset: usize,
    byte_count: usize,
) -> usize {
    let mut chunk_pairs = 0usize;
    let mut remaining = byte_count;
    let mut offset = 0usize;
    while remaining > 0 {
        let target_chunk = target_offset + offset;
        let chunk = if remaining >= 8 && offset.is_multiple_of(8) && target_chunk.is_multiple_of(8)
        {
            8
        } else if remaining >= 4 && offset.is_multiple_of(4) && target_chunk.is_multiple_of(4) {
            4
        } else if remaining >= 2 && offset.is_multiple_of(2) && target_chunk.is_multiple_of(2) {
            2
        } else {
            1
        };
        chunk_pairs += 1;
        offset += chunk;
        remaining -= chunk;
    }
    8 + 4
        + fixed_shape_index_element_address_width(
            omega_target_operations::RuntimeStorageRegion::Machine,
        )
        + 8 * chunk_pairs
}

/// Width of the frame-resident 2D read: frame pair, one optional shared machine
/// index pair, 36-byte math, target pair, and an exact representation copy.
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    8 + if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        8
    } else {
        0
    } + 36
        + 8
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

/// Width of the frame-inline double-indexed write. The target and frame-held
/// indices use one frame pair; x20 preserves a frame source, while one shared
/// machine pair supplies a machine source and/or machine-held indices.
pub fn runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage_width(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    byte_count: usize,
) -> usize {
    8 + 4
        + if source_region == omega_target_operations::RuntimeStorageRegion::Machine
            || outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
            || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        {
            8
        } else {
            0
        }
        + 36
        + runtime_storage_copy_data_width(source_offset, 0, byte_count)
}

/// A distinct machine source is materialized after the leading frame pair and
/// the frame-base preservation move.
pub fn runtime_storage_copy_to_runtime_frame_base_double_indexed_source_base_offset() -> usize {
    12
}

/// The frame-2D read's relocated target pair follows the frame pair and math.
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    44 + if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        8
    } else {
        0
    }
}

pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    8 + 4
        + if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
            || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        {
            8
        } else {
            0
        }
        + 36
        + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(target_field_byte_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    8 + 4
        + if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            8
        } else {
            0
        }
        + 20
        + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(target_field_byte_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_width(
        index_region,
        pointer_byte_offset,
        source_field_byte_offset,
        byte_count,
    )
}

pub fn runtime_storage_copy_machine_double_indexed_to_runtime_pointee_width(
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    8 + 8
        + 36
        + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(target_field_byte_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_machine_indexed_to_runtime_pointee_width(
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    8 + 8
        + 20
        + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(target_field_byte_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_runtime_pointee_to_machine_indexed_width(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_storage_copy_machine_indexed_to_runtime_pointee_width(
        pointer_byte_offset,
        source_field_byte_offset,
        byte_count,
    )
}

pub fn runtime_storage_copy_runtime_pointee_to_machine_double_indexed_width(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_storage_copy_machine_double_indexed_to_runtime_pointee_width(
        pointer_byte_offset,
        source_field_byte_offset,
        byte_count,
    )
}

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_width(
        outer_index_region,
        inner_index_region,
        pointer_byte_offset,
        source_field_byte_offset,
        byte_count,
    )
}

/// Width of the double-indexed RMW binary write: bases + 36-byte math +
/// operands + the operation + the result store.
pub fn runtime_machine_double_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    double_indexed_base_width(outer_index_region, inner_index_region)
        + 36
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(
            operator,
            super::runtime_storage::runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )
        + runtime_result_write_width(0, byte_size)
}

/// The double-indexed RMW's left operand starts after bases + math.
pub fn runtime_machine_double_indexed_binary_left_operand_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    double_indexed_base_width(outer_index_region, inner_index_region) + 36
}

/// An all-frame double-indexed RMW needs one shared frame base pair followed
/// by the same fixed 36-byte address program as the machine-rooted form.
pub fn runtime_frame_base_double_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    8 + 36
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(
            operator,
            super::runtime_storage::runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )
        + runtime_result_write_width(0, byte_size)
}

pub fn runtime_frame_base_double_indexed_binary_left_operand_offset() -> usize {
    44
}

pub fn runtime_frame_base_double_indexed_convert_operand_offset() -> usize {
    44
}

pub fn runtime_machine_double_indexed_string_data_address_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    double_indexed_base_width(outer_index_region, inner_index_region) + 36
}

pub fn runtime_frame_base_double_indexed_string_data_address_offset() -> usize {
    8 + 36
}

pub fn runtime_frame_base_double_indexed_string_write_width(byte_length: usize) -> usize {
    runtime_frame_base_double_indexed_string_data_address_offset()
        + 16
        + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_machine_double_indexed_string_write_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_length: usize,
) -> usize {
    runtime_machine_double_indexed_string_data_address_offset(
        outer_index_region,
        inner_index_region,
    ) + 16
        + unsigned_immediate_width(byte_length as u64)
}

/// The shared frame pair sits directly after the machine pair in every
/// double-indexed op.
pub fn runtime_machine_double_indexed_frame_base_offset() -> usize {
    8
}

/// The double-indexed read's relocated TARGET pair follows bases + math +
/// the element load.
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    double_indexed_base_width(outer_index_region, inner_index_region) + 36 + 4
}

/// Fixed-shape indexed-element address width (see
/// `append_fixed_shape_index_element_address`): mov + [frame adrp pair] +
/// index ldr + movz + mul + add-register + add-immediate.
fn fixed_shape_index_element_address_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    24 + if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        8
    } else {
        0
    }
}

/// Chunk pairs the dual-indexed copy emits (one load + one store each),
/// mirroring `for_each_runtime_copy_chunk`'s split rule from offset 0.
fn runtime_copy_chunk_pair_count(byte_count: usize) -> usize {
    let eights = byte_count / 8;
    let remainder = byte_count % 8;
    eights
        + usize::from(remainder >= 4)
        + usize::from(remainder % 4 >= 2)
        + usize::from(remainder % 2 == 1)
}

/// Width of the dual runtime-indexed copy `arr[i] = arr[j]`: two relocated
/// machine-base pairs, two fixed-shape element addresses, the x24 stash, and
/// one load/store pair per copy chunk.
pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_width(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_count: usize,
) -> usize {
    8 + fixed_shape_index_element_address_width(source_index_region)
        + 4
        + 8
        + fixed_shape_index_element_address_width(target_index_region)
        + 8 * runtime_copy_chunk_pair_count(byte_count)
}

/// Width of a frame-inline indexed pair copy: one shared frame pair, one
/// optional shared machine-index pair, two fixed-shape element addresses, one
/// source-address stash, one target-base reset, and one load/store pair per
/// copy chunk.
pub fn runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_width(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_count: usize,
) -> usize {
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    8 + if source_index_region == machine || target_index_region == machine {
        8
    } else {
        0
    } + fixed_shape_index_element_address_width(
        omega_target_operations::RuntimeStorageRegion::Machine,
    ) + 4
        + 4
        + fixed_shape_index_element_address_width(
            omega_target_operations::RuntimeStorageRegion::Machine,
        )
        + 8 * runtime_copy_chunk_pair_count(byte_count)
}

/// Width of a single-index pair copy across one machine-inline and one
/// frame-inline array: two storage roots, one preserved source root, two fixed
/// address walks, one source-address stash, one target-root move, and the exact
/// representation copy.
pub fn runtime_storage_copy_cross_region_indexed_pair_width(byte_count: usize) -> usize {
    8 + 8 + 4 + 20 + 4 + 4 + 20 + 8 * runtime_copy_chunk_pair_count(byte_count)
}

/// Width of a double-index pair copy across one machine-inline and one
/// frame-inline array, with two storage roots and two fixed 2D address walks.
pub fn runtime_storage_copy_cross_region_double_indexed_pair_width(byte_count: usize) -> usize {
    8 + 8 + 4 + 36 + 4 + 4 + 36 + 8 * runtime_copy_chunk_pair_count(byte_count)
}

/// Width of a frame-inline double-indexed pair copy: one shared frame pair,
/// one optional shared machine-index pair, one preserved root, two fixed 2D
/// address walks, a source-address stash, a target-root reset, and the exact
/// representation copy.
pub fn runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_width(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
    target_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    target_inner_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_count: usize,
) -> usize {
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    8 + 4
        + if source_outer_index_region == machine
            || source_inner_index_region == machine
            || target_outer_index_region == machine
            || target_inner_index_region == machine
        {
            8
        } else {
            0
        }
        + 36
        + 4
        + 4
        + 36
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

/// Width of a machine-rooted double-indexed pair copy. Each side owns its
/// machine pair and fixed 2D walk; a side with either frame-held index also
/// owns one frame pair. The source address is stashed between the walks.
pub fn runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_width(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
    target_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    target_inner_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_count: usize,
) -> usize {
    double_indexed_base_width(source_outer_index_region, source_inner_index_region)
        + 36
        + 4
        + double_indexed_base_width(target_outer_index_region, target_inner_index_region)
        + 36
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_machine_double_indexed_pair_second_base_offset(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    double_indexed_base_width(source_outer_index_region, source_inner_index_region) + 36 + 4
}

pub fn runtime_storage_copy_machine_double_indexed_pair_target_frame_base_offset(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    runtime_storage_copy_machine_double_indexed_pair_second_base_offset(
        source_outer_index_region,
        source_inner_index_region,
    ) + 8
}

/// Offset of the SECOND relocated machine-base `adrp` inside the dual-indexed
/// copy (the target half): the first base pair (8) + the source element
/// address + the x24 stash (4).
pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    8 + fixed_shape_index_element_address_width(source_index_region) + 4
}

/// Offset of a FRAME-resident index's relocated `adrp` pair inside the
/// dual-indexed copy: each side's pair sits after its machine-base pair plus
/// the leading `mov x20`.
pub fn runtime_storage_copy_machine_indexed_frame_index_offset(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    is_target_side: bool,
) -> usize {
    if is_target_side {
        runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
            source_index_region,
        ) + 8
            + 4
    } else {
        8 + 4
    }
}

/// Entry `args: &[u8]` descriptor write: the frame pair (8) + the spill
/// address add (4) + ptr store (4) + the padded length immediate (8) + len
/// store (4).
pub fn entry_arguments_slice_descriptor_write_width() -> usize {
    28
}

pub fn runtime_text_line_read_import_width(_byte_capacity: usize, target_offset: usize) -> usize {
    116 + line_read_descriptor_store_extra(target_offset)
}

pub fn runtime_text_line_read_syscall_width(
    _byte_capacity: usize,
    syscall_number: u32,
    target_offset: usize,
) -> usize {
    116 + unsigned_immediate_width(u64::from(syscall_number))
        + line_read_descriptor_store_extra(target_offset)
}

/// Width of the owned `[u8; N]` carrier line read (import binding): the
/// relocated region `adrp`+`add` (8) + the fixed bytes-base add (4) + cursor
/// setup (8) + the 80-byte read loop (byte-identical to the descriptor flavor)
/// + the `subs` + len-word store epilogue (8). Every element is fixed width,
/// so the total is a constant -- which keeps the import-call relocation offset
/// constant too (the planner has no target_offset to hand).
pub fn runtime_text_line_read_carrier_import_width(target_offset: usize) -> usize {
    104 + add_constant_width(target_offset + 8)
}

/// The syscall flavor swaps the 4-byte `bl` for the syscall-number immediate
/// plus `svc` (4), so it grows by exactly the immediate's width.
pub fn runtime_text_line_read_carrier_syscall_width(
    syscall_number: u32,
    target_offset: usize,
) -> usize {
    100 + add_constant_width(target_offset + 8)
        + unsigned_immediate_width(u64::from(syscall_number))
        + 4
}

/// Raw `[u8; N]` scratch has the carrier's direct-target prologue but no
/// descriptor/length epilogue.
pub fn runtime_text_line_read_fixed_array_import_width(target_offset: usize) -> usize {
    96 + add_constant_width(target_offset)
}

pub fn runtime_text_line_read_fixed_array_syscall_width(
    syscall_number: u32,
    target_offset: usize,
) -> usize {
    92 + add_constant_width(target_offset) + unsigned_immediate_width(u64::from(syscall_number)) + 4
}

/// Offset of the import `bl` inside the carrier line read: the 12-byte
/// prologue (adrp + add + bytes-base add) + cursor setup (8) + the three
/// syscall-argument moves (12).
pub fn runtime_text_line_read_carrier_import_call_offset(target_offset: usize) -> usize {
    28 + add_constant_width(target_offset + 8)
}

pub fn runtime_text_line_read_fixed_array_import_call_offset(target_offset: usize) -> usize {
    28 + add_constant_width(target_offset)
}

/// Width of the ByteRead stdin read (import binding): the relocated region
/// `adrp`+`add` (8) + tag/payload zero stores (8) + the three call-argument
/// moves (12) + `bl` (4) + the cbz/movz/tag-store epilogue (12). Every
/// element is fixed width, so the import-call relocation offset is the
/// constant 28.
pub fn runtime_byte_read_import_width() -> usize {
    44
}

/// The syscall flavor swaps the 4-byte `bl` for the syscall-number
/// immediate plus `svc` (4).
pub fn runtime_byte_read_syscall_width(syscall_number: u32) -> usize {
    40 + unsigned_immediate_width(u64::from(syscall_number)) + 4
}

/// Offset of the import `bl` inside the ByteRead read: adrp + add (8) +
/// the two zero stores (8) + the three call-argument moves (12).
pub fn runtime_byte_read_import_call_offset() -> usize {
    28
}

/// Width of the stdout byte write (import binding): the relocated source
/// `adrp`+`add` (8) + the three call-argument moves (12) + `bl` (4).
pub fn runtime_byte_write_import_width(source_offset: usize) -> usize {
    runtime_byte_write_import_call_offset(source_offset) + 4
}

/// The syscall flavor swaps the 4-byte `bl` for the syscall-number
/// immediate plus `svc` (4).
pub fn runtime_byte_write_syscall_width(syscall_number: u32, source_offset: usize) -> usize {
    16 + add_constant_width(source_offset).max(4)
        + unsigned_immediate_width(u64::from(syscall_number))
        + 4
}

/// Offset of the import `bl` inside the byte write: adrp + add (8) + the
/// three call-argument moves (12).
pub fn runtime_byte_write_import_call_offset(source_offset: usize) -> usize {
    16 + add_constant_width(source_offset).max(4)
}

pub fn runtime_text_line_read_import_target_address_offset() -> usize {
    100
}

pub fn runtime_text_line_read_syscall_target_address_offset(syscall_number: u32) -> usize {
    100 + unsigned_immediate_width(u64::from(syscall_number))
}

pub fn runtime_text_line_read_import_call_offset() -> usize {
    28
}

pub fn runtime_storage_copy_width(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    16 + add_constant_width(source_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_to_runtime_frame_indexed_width(
    source_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + add_constant_width(source_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 8
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(field_byte_offset);
    12 + add_constant_width(source_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(field_byte_offset);
    20 + add_constant_width(source_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(source_field_byte_offset);
    16 + add_constant_width(source_offset)
        + add_constant_width(target_field_byte_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
    element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    // index setup (x16 = element source-field addr) + load x20 = pointer (4)
    // + add target field to x20 + data copy.
    runtime_frame_index_setup_width(element_byte_size, source_field_byte_offset)
        + 4
        + add_constant_width(target_field_byte_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    // Fixed part = index-address setup (adrp+add+mov = 12) + the index LOAD +
    // `add x16,x16,x26` (4) + the target-region adrp+add (8). The index load is
    // region- and offset-aware (a large index materializes); MUST match the encoder.
    24 + machine_index_load_width(index_region, index_offset, index_byte_size)
        + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

/// Write-side mirror of `..._from_runtime_machine_indexed_to_runtime_storage_width`:
/// same fixed part (index-address setup + region-dependent index load + the store
/// base's adrp/add), with `source_offset`'s `add x20,#source` in place of the
/// read's `add x20,#target`. MUST match the encoder.
pub fn runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width(
    source_offset: usize,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    24 + machine_index_load_width(index_region, index_offset, index_byte_size)
        + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + add_constant_width(source_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

/// The byte offset of the SOURCE adrp (`adrp x20`) within the store — same
/// position as the read's target adrp, region-aware. Used by the relocation
/// planner to relocate the source page-pair to the machine symbol.
/// Width of the machine-indexed ADDRESS write (`&self.buf[k] as &Wide` -- the
/// element ADDRESS into a frame slot): the machine-indexed address computation
/// (identical layout to the copy family's prefix, so its relocation positions
/// reuse those offset fns) + the target frame page pair (8) + the 8-byte
/// address store (materializing a large target offset). MUST stay in lockstep
/// with `encode_runtime_machine_indexed_address_to_runtime_frame_write`.
pub fn runtime_machine_indexed_address_to_runtime_frame_write_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) + 8
        + store_data_offset_width(target_offset, 8)
}

pub fn runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    16 + machine_index_load_width(index_region, index_offset, index_byte_size)
        + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
}

pub fn runtime_storage_copy_to_runtime_pointee_width(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    20 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + add_constant_width(source_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    16 + add_constant_width(pointer_byte_offset)
        + runtime_load_data_width(8)
        + add_constant_width(field_byte_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn operand_width(operand: &Aarch64CallOperand) -> usize {
    match operand {
        DataAddress { .. } => 8,
        // adrp + add (8) + a sized load (arg) / store (result) that materializes a
        // large field offset (a scalar declared after a big array, offset > the LDR/STR
        // scaled-immediate range): `load_data_offset_width` == `store_data_offset_width`
        // (4 when encodable; 8 + add-constant otherwise), so this width tracks BOTH the
        // arg-load and the result-store emission in lockstep — and the relocation
        // planner (data_addresses.rs) sums these to place each operand's adrp exactly.
        RuntimeScalarInteger {
            byte_offset,
            byte_count,
        } => 8 + load_data_offset_width(*byte_offset, *byte_count),
        // adrp + add (8) + `add field_offset` that materializes a large offset
        // (`append_add_x_constant`; `add_constant_width` == 0 for offset 0, 4 when
        // <=4095, else movz/movk + add-register). Matches the emission in lockstep.
        RuntimeStorageAddress { byte_offset } => 8 + add_constant_width(*byte_offset),
        RuntimeStringPointer {
            byte_offset,
            is_bounded_buffer: true,
        } => 8 + add_constant_width(*byte_offset + 8),
        RuntimeStringPointer { .. } | RuntimeStringLength { .. } => 12,
        RuntimePointeeStringPointer { .. } | RuntimePointeeStringLength { .. } => 16,
        // adrp + add + load + fmov (into a v-register) = 16.
        RuntimeScalarFloat { .. } => 16,
        // One relocated base pair, then a sized load plus FMOV for every
        // normalized member fragment. Large source offsets use the same
        // load-width accounting as emission.
        RuntimeHomogeneousFloatAggregate {
            byte_offset,
            member_byte_count,
            members,
        } => {
            8 + (0..usize::from(*members))
                .map(|member| {
                    load_data_offset_width(
                        byte_offset + member * member_byte_count,
                        *member_byte_count,
                    ) + 4
                })
                .sum::<usize>()
        }
        RuntimeSmallAggregate {
            byte_offset,
            byte_count,
            ..
        } => {
            8 + (0..byte_count.div_ceil(8))
                .map(|fragment| {
                    let fragment_offset = fragment * 8;
                    load_data_offset_width(
                        byte_offset + fragment_offset,
                        (byte_count - fragment_offset).min(8),
                    )
                })
                .sum::<usize>()
        }
        RuntimeLargeAggregate {
            byte_offset,
            byte_count,
            ..
        } => {
            let mut width = 8;
            let mut copied = 0;
            while copied < *byte_count {
                let fragment = [8, 4, 2, 1]
                    .into_iter()
                    .find(|fragment| byte_count - copied >= *fragment)
                    .expect("large aggregate copy has bytes remaining");
                width += load_data_offset_width(byte_offset + copied, fragment);
                copied += fragment;
            }
            width
        }
        ImmediateInteger(value) => immediate_width(*value),
        ByteLength(value) => unsigned_immediate_width(*value as u64),
    }
}

fn immediate_width(value: i64) -> usize {
    // Negative values materialize as their full 64-bit two's-complement bit
    // pattern (see `append_unsigned_immediate`), so size that pattern.
    unsigned_immediate_width(value as u64)
}

pub(in crate::aarch64) fn unsigned_immediate_width(value: u64) -> usize {
    let high_nonzero_halfwords = (1..4)
        .filter(|halfword_shift| halfword(value, *halfword_shift) != 0)
        .count();

    4 + high_nonzero_halfwords * 4
}

fn runtime_storage_copy_data_width(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
) -> usize {
    let mut remaining = byte_count;
    let mut offset = 0usize;
    let mut width = 0usize;

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

        width += load_data_offset_width(source_offset, chunk_size)
            + store_data_offset_width(target_offset, chunk_size);
        offset += chunk_size;
        remaining -= chunk_size;
    }

    width
}

pub(in crate::aarch64) fn load_data_offset_width(byte_offset: usize, byte_size: usize) -> usize {
    if data_offset_encodable(byte_offset, byte_size) {
        4
    } else {
        4 + add_constant_width(byte_offset) + 4
    }
}

/// Width of `encode_host_call_sequence_constant_result_from_operands`:
/// padded imm64 (16) + adrp/add (8) + the result store. MUST stay in
/// lockstep with that encoder and the data-address relocation offset (16).
pub fn constant_result_sequence_width(byte_offset: usize, byte_size: usize) -> usize {
    16 + 8 + store_data_offset_width(byte_offset, byte_size)
}

pub(in crate::aarch64) fn store_data_offset_width(byte_offset: usize, byte_size: usize) -> usize {
    if data_offset_encodable(byte_offset, byte_size) {
        4
    } else {
        4 + add_constant_width(byte_offset) + 4
    }
}

/// Bytes `append_runtime_machine_index_target_address` spends loading the
/// declared-width index, region-aware and offset-aware. Both regions load via
/// `append_load_data_from_x_offset` (`load_data_offset_width`), which materializes a
/// large `index_offset` (a loop counter declared after a big array); the
/// `RuntimeFrame` case first re-derives the frame base with an `adrp`+`add` page
/// pair (8). MUST stay in lockstep with that encoder. Small offsets collapse to the
/// historical fixed widths (Machine 4, RuntimeFrame 12) so existing layouts are
/// unchanged.
pub(in crate::aarch64) fn machine_index_load_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
) -> usize {
    match index_region {
        omega_target_operations::RuntimeStorageRegion::Machine => {
            load_data_offset_width(index_offset, index_byte_size)
        }
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            8 + load_data_offset_width(index_offset, index_byte_size)
        }
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

fn runtime_text_descriptor_load_pair_width(byte_offset: usize) -> usize {
    load_data_offset_width(byte_offset, 8) + load_data_offset_width(byte_offset + 8, 8)
}

fn runtime_text_descriptor_store_pair_width(byte_offset: usize) -> usize {
    store_data_offset_width(byte_offset, 8) + store_data_offset_width(byte_offset + 8, 8)
}

/// Fixed width of a value-position text-equals operand (the `TextEquals` arm
/// of `append_runtime_value_operand`): two relocated descriptor-page
/// materializations (adrp+add, 8 bytes each), four fixed-width 24-byte
/// descriptor word loads (padded immediate offset + add + ldr, so the width is
/// offset-independent), and eleven 4-byte compare/loop instructions. MUST stay
/// in lockstep with that encoder (it ends with a `debug_assert_eq!` against
/// this function) and with `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET` below.
pub fn runtime_text_equals_operand_width() -> usize {
    8 + 24 + 24 + 8 + 24 + 24 + 11 * 4
}

/// Width of a guard-position text-vs-literal content compare operand (the
/// `TextEqualsLiteral` arm of `append_runtime_value_operand`): the place's
/// descriptor-address setup (relocated storage base, pointee pointer deref,
/// or one of the indexed element address sequences, ending in x16), two
/// descriptor word loads (8), a fixed 28-byte head (result zero, padded
/// literal-length immediate, length compare, mismatch branch), one unrolled
/// 12-byte load/compare/branch block per literal byte, and the final 4-byte
/// result set. MUST stay in lockstep with that encoder (it ends with a
/// `debug_assert_eq!` against this function).
pub fn runtime_text_equals_literal_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    place: RuntimeValueOperandHandle,
    literal: &[u8],
) -> usize {
    let place_setup_width = if let Some((_, byte_offset, _)) = runtime_value_operands.storage(place)
    {
        8 + add_constant_width(byte_offset)
    } else if let Some((pointer_byte_offset, field_byte_offset, _)) =
        runtime_value_operands.pointee(place)
    {
        // Page pair (8) + the 8-byte pointer load (4) with its optional
        // offset add, + the optional field-offset add.
        12 + add_constant_width(pointer_byte_offset) + add_constant_width(field_byte_offset)
    } else if let Some((_, index_region, _, _, element_byte_size, field_byte_offset, _)) =
        runtime_value_operands.frame_indexed(place)
    {
        runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
            + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine)
                * 8
    } else if let Some((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_base_indexed(place)
    {
        runtime_frame_base_index_setup_width_with_index_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        )
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_fixed_indexed(place)
    {
        runtime_frame_fixed_index_setup_width(
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
        )
    } else {
        // Selection only builds this operand over storage/pointee/indexed
        // text places; the encoder rejects anything else with a hard
        // diagnostic before this width could be compared against emitted
        // bytes.
        0
    };
    place_setup_width + 8 + 28 + 12 * literal.len() + 4
}

/// Byte offset of the RIGHT descriptor's adrp inside a text-equals operand
/// (left page + two fixed-width left descriptor loads precede it). The
/// relocation planner targets the right region's symbol here.
pub const RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET: usize = 8 + 24 + 24;
/// Relative address-materialization sites inside recursive value operands.
pub const MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET: usize = 8;
pub const FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET: usize = 32;

pub fn runtime_value_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    if let Some(value) = runtime_value_operands.immediate_integer(operand) {
        immediate_width(value)
    } else if let Some((_, byte_offset, byte_size)) = runtime_value_operands.storage(operand) {
        8 + add_constant_width(byte_offset) + runtime_load_data_width(byte_size)
    } else if let Some((_, base_byte_offset, _, fragments)) =
        runtime_value_operands.bit_field(operand)
    {
        runtime_bit_field_operand_width(base_byte_offset, &fragments)
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        runtime_value_operands.pointee(operand)
    {
        12 + add_constant_width(pointer_byte_offset)
            + add_constant_width(field_byte_offset)
            + runtime_load_data_width(byte_size)
    } else if let Some((_, index_region, _, _, element_byte_size, field_byte_offset, byte_size)) =
        runtime_value_operands.frame_indexed(operand)
    {
        runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
            + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine)
                * 8
            + runtime_load_data_width(byte_size)
    } else if let Some((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        runtime_frame_base_index_setup_width_with_index_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ) + runtime_load_data_width(byte_size)
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_fixed_indexed(operand)
    {
        runtime_frame_fixed_index_setup_width(
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
        ) + runtime_load_data_width(byte_size)
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
        // MUST mirror the machine-indexed operand arm exactly: machine pair
        // (8) + conditional frame pair (8) + 4-byte index load + scale +
        // address add (4) + combined base+field constant + element load (4).
        let frame_pair =
            if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                8
            } else {
                0
            };
        let _ = byte_size;
        8 + frame_pair
            + load_data_offset_width(index_offset, index_byte_size)
            + scale_index_width(element_byte_size)
            + 4
            + add_constant_width(base_byte_offset + field_byte_offset)
            + 4
    } else if runtime_value_operands.text_equals(operand).is_some() {
        runtime_text_equals_operand_width()
    } else if let Some((place, literal, _place_is_bounded_buffer)) =
        runtime_value_operands.text_equals_literal(operand)
    {
        runtime_text_equals_literal_operand_width(runtime_value_operands, place, &literal)
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let operand_domain = runtime_value_operands
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
            });
        let saturating_signed_div_mod = runtime_value_operands
            .binary_arithmetic_domain(operand)
            .is_some_and(|(domain, operands_signed)| {
                domain == psi_numerics::arithmetic::ArithmeticDomain::Saturating
                    && operands_signed
                    && matches!(
                        operator,
                        StateGuardOperator::Divide | StateGuardOperator::Modulo
                    )
            });
        let operation_width = if runtime_value_operands.binary_is_float(operand) {
            runtime_float_binary_operation_width_with_domain(
                operator,
                runtime_value_operands
                    .binary_byte_width(operand)
                    .or_else(|| {
                        super::runtime_storage::runtime_value_operand_value_byte_size(
                            runtime_value_operands,
                            left,
                        )
                    })
                    .or_else(|| {
                        super::runtime_storage::runtime_value_operand_value_byte_size(
                            runtime_value_operands,
                            right,
                        )
                    })
                    .unwrap_or(8),
                runtime_value_operands
                    .binary_arithmetic_domain(operand)
                    .map(|(domain, _)| domain)
                    .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact),
            )
        } else if let Some((domain, operands_signed)) = operand_domain {
            // Saturating/Trapping operand-position arithmetic: MUST mirror the
            // operand evaluator's clamp/trap dispatch or offsets drift.
            saturating_trapping_arithmetic_width(
                domain,
                operator,
                runtime_value_operands
                    .binary_byte_width(operand)
                    .unwrap_or(8),
                operands_signed,
                runtime_value_operands.immediate_integer(left).is_some(),
                runtime_value_operands.immediate_integer(right).is_some(),
            )
        } else if saturating_signed_div_mod {
            // Signed Saturating div/mod operand arm: the TYPE_MIN/-1 fixup.
            saturating_signed_divide_modulo_width(
                runtime_value_operands
                    .binary_byte_width(operand)
                    .unwrap_or(8),
                operator == StateGuardOperator::Modulo,
            )
        } else {
            // Plain-op arm; a nested WRAPPING node < 8 bytes appends one
            // truncation instruction (append_wrapping_operand_truncation) so
            // the parent reads the width-wrapped value. MUST stay in
            // lockstep with the operand evaluator.
            let wrapping_truncation = if matches!(
                runtime_value_operands.binary_arithmetic_domain(operand),
                Some((psi_numerics::arithmetic::ArithmeticDomain::Wrapping, _))
            ) && runtime_value_operands
                .binary_byte_width(operand)
                .is_some_and(|width| width < 8)
            {
                4
            } else {
                0
            };
            runtime_binary_operation_width_with_domain(
                operator,
                super::runtime_storage::runtime_binary_operation_byte_size(
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
            ) + wrapping_truncation
        };
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + operation_width
    } else if let Some((
        source,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
    )) = runtime_value_operands.convert(operand)
    {
        runtime_value_operand_width(runtime_value_operands, source)
            + runtime_convert_operation_width(
                source_byte_size,
                target_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                runtime_value_operands.convert_target_signed(operand),
                runtime_value_operands.convert_trapping(operand),
                runtime_value_operands.convert_saturating(operand),
            )
    } else {
        0
    }
}

fn runtime_binary_operation_width(operator: StateGuardOperator, byte_size: usize) -> usize {
    // Narrow SIGNED divide/modulo sign-extend BOTH operands first (+8); a
    // narrow signed shift-right extends the shifted value (+4). See
    // append_narrow_signed_division_operand_extension / the ShiftRight arm.
    let narrow_signed_extension = match operator {
        StateGuardOperator::Divide | StateGuardOperator::Modulo if matches!(byte_size, 1 | 2) => 8,
        StateGuardOperator::ShiftRight if matches!(byte_size, 1 | 2) => 4,
        // A narrow logical `>>` zero-extends the shifted value the same way
        // (see the ShiftRightLogical arm's uxtb/uxth); width 4 uses the W form
        // with no extension.
        StateGuardOperator::ShiftRightLogical if matches!(byte_size, 1 | 2) => 4,
        _ => 0,
    };
    narrow_signed_extension + runtime_binary_operation_width_base(operator)
}

fn runtime_binary_operation_width_base(operator: StateGuardOperator) -> usize {
    // Every operation emits the same instruction count for the 32-bit and
    // 64-bit register forms, so this width is operand-width independent.
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::And
        | StateGuardOperator::Or
        | StateGuardOperator::BitwiseAnd
        | StateGuardOperator::BitwiseOr
        | StateGuardOperator::BitwiseXor
        | StateGuardOperator::Subtract
        | StateGuardOperator::Multiply
        | StateGuardOperator::Divide
        | StateGuardOperator::DivideUnsigned
        | StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => 4,
        StateGuardOperator::Modulo | StateGuardOperator::ModuloUnsigned => 8,
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => 12,
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => 16,
        _ => 0,
    }
}

fn runtime_store_data_width(byte_size: usize) -> usize {
    // Narrow stores materialize the value as a fixed-width MOVZ+MOVK pair (8)
    // + the sized store (4); 8-byte stores use the padded 4-instruction
    // materialization (16) + STR (4).
    match byte_size {
        1 | 2 | 4 => 12,
        8 => 20,
        _ => 0,
    }
}

fn runtime_load_data_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 2 | 4 | 8 => 4,
        _ => 0,
    }
}

fn runtime_result_write_width(byte_offset: usize, byte_size: usize) -> usize {
    match byte_size {
        1 | 2 | 4 | 8 => store_data_offset_width(byte_offset, byte_size),
        _ => 0,
    }
}

pub(in crate::aarch64) fn runtime_frame_index_setup_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    60 + scale_index_width(element_byte_size) + add_constant_width(field_byte_offset)
}

fn runtime_frame_fixed_index_setup_width(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(field_byte_offset);

    8 + load_data_offset_width(descriptor_offset, 8) + add_constant_width(source_offset)
}

pub(in crate::aarch64) fn scale_index_width(element_byte_size: usize) -> usize {
    if element_byte_size == 0 {
        return 0;
    }

    let highest_bit = usize::BITS - element_byte_size.leading_zeros();
    let doubles = highest_bit.saturating_sub(1) as usize;
    let additions = element_byte_size.count_ones() as usize;
    8 + (doubles + additions) * 4
}

pub(in crate::aarch64) fn add_constant_width(value: usize) -> usize {
    if value == 0 {
        0
    } else if value <= 4095 {
        4
    } else {
        unsigned_immediate_width(value as u64) + 4
    }
}

fn store_x_offset_width(byte_offset: usize) -> usize {
    if byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095 {
        4
    } else {
        add_constant_width(byte_offset) + 4
    }
}

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}

/// Shared wire-append prologue (`wire_encode.rs`): out page pair + out-offset
/// add + written page pair + cursor load + pointer add.
fn wire_append_prologue_width(out_offset: usize, written_offset: usize) -> usize {
    8 + add_constant_width(out_offset) + 8 + load_data_offset_width(written_offset, 8) + 4
}

/// The fixed nine-instruction LEB128 emit loop in
/// `encode_append_wire_scalar_varint`.
pub fn wire_varint_emit_loop_width() -> usize {
    36
}

/// Sign-mask + shift + xor (12), plus the `sxtw` a 4-byte signed source needs
/// before zigzagging at 64 bits.
pub(in crate::aarch64) fn wire_zigzag_width(byte_size: usize) -> usize {
    if byte_size == 4 { 16 } else { 12 }
}

pub fn append_wire_literal_byte_width(out_offset: usize, written_offset: usize) -> usize {
    // Prologue + movz + post-increment strb + cursor add + cursor store.
    wire_append_prologue_width(out_offset, written_offset)
        + 12
        + store_data_offset_width(written_offset, 8)
}

pub fn append_wire_scalar_varint_width(
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    wire_append_prologue_width(out_offset, written_offset)
        + 8
        + load_data_offset_width(source_offset, byte_size)
        + if zigzag {
            wire_zigzag_width(byte_size)
        } else {
            0
        }
        + wire_varint_emit_loop_width()
        + store_data_offset_width(written_offset, 8)
}

/// The fixed eight-instruction bounds-checked byte-copy loop in
/// `encode_append_wire_text_bytes`.
pub fn wire_text_copy_loop_width() -> usize {
    32
}

pub fn append_wire_text_bytes_width(
    source_offset: usize,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> usize {
    // Prologue + descriptor page pair + ptr load + len load + count move +
    // length-varint emit loop + capacity materialization + bounded copy loop
    // + cursor store.
    wire_append_prologue_width(out_offset, written_offset)
        + 8
        + load_data_offset_width(source_offset, 8)
        + load_data_offset_width(source_offset + 8, 8)
        + 4
        + wire_varint_emit_loop_width()
        + unsigned_immediate_width(out_length as u64)
        + wire_text_copy_loop_width()
        + store_data_offset_width(written_offset, 8)
}

pub fn append_wire_scalar_slice_width(
    source_offset: usize,
    element_byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> usize {
    let zigzag_instructions = if zigzag {
        if element_byte_size == 4 { 4 } else { 3 }
    } else {
        0
    };
    // Dynamic block: 25 control/accounting instructions, two nine-word
    // varint emit loops, and two scalar load/advance/zigzag blocks.
    let dynamic_instructions = 25 + 18 + 2 * (2 + zigzag_instructions);
    wire_append_prologue_width(out_offset, written_offset)
        + 8
        + load_data_offset_width(source_offset, 8)
        + load_data_offset_width(source_offset + 8, 8)
        + 12
        + unsigned_immediate_width(out_length as u64)
        + dynamic_instructions * 4
        + store_data_offset_width(written_offset, 8)
}

/// Byte offset of the WRITTEN page adrp pair inside both wire appends (the
/// relocation planner points it at the written slot's region symbol).
pub fn wire_append_written_page_offset(out_offset: usize) -> usize {
    8 + add_constant_width(out_offset)
}

/// Byte offset of the SOURCE page adrp pair inside the varint append AND the
/// text-bytes append (both materialize the source page right after the shared
/// prologue).
pub fn wire_append_varint_source_page_offset(out_offset: usize, written_offset: usize) -> usize {
    wire_append_prologue_width(out_offset, written_offset)
}

/// Shared wire-decode prologue (`wire_decode.rs`): buffer page pair +
/// buffer-offset add + read page pair + cursor load + pointer add + ok page
/// pair.
fn wire_decode_prologue_width(buffer_offset: usize, read_offset: usize) -> usize {
    8 + add_constant_width(buffer_offset) + 8 + load_data_offset_width(read_offset, 8) + 4 + 8
}

/// Shared wire-decode epilogue: sticky ok-flag merge (load + and + store) +
/// cursor write-back.
fn wire_decode_tail_width(read_offset: usize, ok_offset: usize) -> usize {
    load_data_offset_width(ok_offset, 1)
        + 4
        + store_data_offset_width(ok_offset, 1)
        + store_data_offset_width(read_offset, 8)
}

/// The fixed canonical LEB128 read loop in
/// `encode_read_wire_scalar_varint`.
pub fn wire_varint_read_loop_width() -> usize {
    84
}

/// movz #63 + lslv + asr + lsr + eor (`(n >> 1) ^ -(n & 1)`).
pub fn wire_unzigzag_width() -> usize {
    20
}

pub fn read_wire_expected_byte_width(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
) -> usize {
    // Prologue + length materialization + success-bit movz + the fixed
    // seven-instruction check block + epilogue.
    wire_decode_prologue_width(buffer_offset, read_offset)
        + unsigned_immediate_width(buffer_length as u64)
        + 4
        + 28
        + wire_decode_tail_width(read_offset, ok_offset)
}

pub fn read_wire_scalar_varint_width(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    // Prologue + length materialization + success/value/shift movz triple +
    // read loop + optional unzigzag + target page pair + truncating store +
    // epilogue.
    wire_decode_prologue_width(buffer_offset, read_offset)
        + unsigned_immediate_width(buffer_length as u64)
        + 12
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 8
        + range.map_or(0, |range| {
            unsigned_immediate_width(range.minimum as u64)
                + unsigned_immediate_width(range.maximum as u64)
                + 24
        })
        + store_data_offset_width(target_offset, byte_size)
        + wire_decode_tail_width(read_offset, ok_offset)
}

/// Byte offset of the READ (cursor) page adrp pair inside both wire decodes.
pub fn wire_decode_read_page_offset(buffer_offset: usize) -> usize {
    8 + add_constant_width(buffer_offset)
}

/// Byte offset of the OK (sticky flag) page adrp pair inside both wire
/// decodes.
pub fn wire_decode_ok_page_offset(buffer_offset: usize, read_offset: usize) -> usize {
    wire_decode_read_page_offset(buffer_offset) + 8 + load_data_offset_width(read_offset, 8) + 4
}

/// Byte offset of the TARGET page adrp pair inside the varint decode.
pub fn wire_decode_varint_target_page_offset(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    zigzag: bool,
) -> usize {
    wire_decode_prologue_width(buffer_offset, read_offset)
        + unsigned_immediate_width(buffer_length as u64)
        + 12
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
}

/// Bytes of the decode-boundary byte-predicate validation blocks (one per
/// mask bit, `ByteSequencePredicate::ALL` order). MUST mirror
/// `append_wire_byte_predicate_checks`: utf8 is the 77-instruction
/// compare/branch walk; no_nul 7; ascii_only 8; non_empty 2.
pub fn wire_byte_predicate_checks_width(predicate_mask: u8) -> usize {
    use psi_language_semantics::byte_predicates::ByteSequencePredicate;
    ByteSequencePredicate::in_mask(predicate_mask)
        .map(|predicate| match predicate {
            ByteSequencePredicate::ValidUtf8 => 308,
            ByteSequencePredicate::NoNul => 28,
            ByteSequencePredicate::AsciiOnly => 32,
            ByteSequencePredicate::NonEmpty => 8,
        })
        .sum()
}

pub fn read_wire_byte_slice_width(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_offset: usize,
    predicate_mask: u8,
) -> usize {
    // Prologue + buffer-length materialization + success/value/shift movz triple
    // + read loop + bounds&advance (6 instrs, 24) + the byte-predicate
    // validation blocks + target page pair (8) + ptr store + len store +
    // epilogue.
    wire_decode_prologue_width(buffer_offset, read_offset)
        + unsigned_immediate_width(buffer_length as u64)
        + 12
        + wire_varint_read_loop_width()
        + 24
        + wire_byte_predicate_checks_width(predicate_mask)
        + 8
        + store_data_offset_width(target_offset, 8)
        + store_data_offset_width(target_offset + 8, 8)
        + wire_decode_tail_width(read_offset, ok_offset)
}

/// Byte offset of the TARGET page adrp pair inside the byte-slice decode (after
/// the prologue, length init, read loop, and the bounds&advance block).
pub fn wire_decode_byte_slice_target_page_offset(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    predicate_mask: u8,
) -> usize {
    wire_decode_prologue_width(buffer_offset, read_offset)
        + wire_byte_predicate_checks_width(predicate_mask)
        + unsigned_immediate_width(buffer_length as u64)
        + 12
        + wire_varint_read_loop_width()
        + 24
}

pub fn read_wire_nested_open_width(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> usize {
    // Prologue + end page pair + length load + buffer-length materialization
    // + the fixed eight-instruction length/end check block + end store +
    // epilogue.
    wire_decode_prologue_width(buffer_offset, read_offset)
        + 8
        + load_data_offset_width(end_offset, 8)
        + unsigned_immediate_width(buffer_length as u64)
        + 32
        + store_data_offset_width(end_offset, 8)
        + wire_decode_tail_width(read_offset, ok_offset)
}

pub fn read_wire_nested_close_width(
    buffer_offset: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> usize {
    // Prologue + end page pair + end load + the success movz/cmp/branch/fail
    // movz quad + epilogue.
    wire_decode_prologue_width(buffer_offset, read_offset)
        + 8
        + load_data_offset_width(end_offset, 8)
        + 16
        + wire_decode_tail_width(read_offset, ok_offset)
}

/// Byte offset of the END-slot page adrp pair inside both nested decodes
/// (materialized right after the shared prologue). The repeated-element read
/// materializes its end page at the same position.
pub fn wire_decode_nested_end_page_offset(buffer_offset: usize, read_offset: usize) -> usize {
    wire_decode_prologue_width(buffer_offset, read_offset)
}

pub fn append_wire_repeated_scalar_varint_width(
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    index: u64,
    count_offset: usize,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    // Prologue + count page pair + count load + index materialization +
    // cmp/b.hs guard + source page pair + scalar load + optional zigzag +
    // emit loop + cursor store.
    wire_append_prologue_width(out_offset, written_offset)
        + 8
        + load_data_offset_width(count_offset, 8)
        + unsigned_immediate_width(index)
        + 8
        + 8
        + load_data_offset_width(source_offset, byte_size)
        + if zigzag {
            wire_zigzag_width(byte_size)
        } else {
            0
        }
        + wire_varint_emit_loop_width()
        + store_data_offset_width(written_offset, 8)
}

/// Byte offset of the COUNT page adrp pair inside the repeated append (right
/// after the shared prologue).
pub fn wire_append_repeated_count_page_offset(out_offset: usize, written_offset: usize) -> usize {
    wire_append_prologue_width(out_offset, written_offset)
}

/// Byte offset of the SOURCE page adrp pair inside the repeated append
/// (after the count guard).
pub fn wire_append_repeated_source_page_offset(
    out_offset: usize,
    written_offset: usize,
    count_offset: usize,
    index: u64,
) -> usize {
    wire_append_repeated_count_page_offset(out_offset, written_offset)
        + 8
        + load_data_offset_width(count_offset, 8)
        + unsigned_immediate_width(index)
        + 8
}

#[allow(clippy::too_many_arguments)]
pub fn read_wire_repeated_scalar_varint_width(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
    count_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    // Prologue + end page pair + end load + cmp/b.hs guard + length
    // materialization + success/value/shift movz triple + read loop +
    // optional unzigzag + target page pair + truncating store + count page
    // pair + count load + add + count store + epilogue.
    wire_decode_prologue_width(buffer_offset, read_offset)
        + 8
        + load_data_offset_width(end_offset, 8)
        + 8
        + unsigned_immediate_width(buffer_length as u64)
        + 12
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 8
        + range.map_or(0, |range| {
            unsigned_immediate_width(range.minimum as u64)
                + unsigned_immediate_width(range.maximum as u64)
                + 24
        })
        + store_data_offset_width(target_offset, byte_size)
        + 8
        + load_data_offset_width(count_offset, 8)
        + 4
        + store_data_offset_width(count_offset, 8)
        + wire_decode_tail_width(read_offset, ok_offset)
}

/// Byte offset of the TARGET page adrp pair inside the repeated read (after
/// the guard and the read loop).
pub fn wire_decode_repeated_target_page_offset(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    zigzag: bool,
) -> usize {
    wire_decode_prologue_width(buffer_offset, read_offset)
        + 8
        + load_data_offset_width(end_offset, 8)
        + 8
        + unsigned_immediate_width(buffer_length as u64)
        + 12
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
}

/// Byte offset of the COUNT page adrp pair inside the repeated read (after
/// the target store).
#[allow(clippy::too_many_arguments)]
pub fn wire_decode_repeated_count_page_offset(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    wire_decode_repeated_target_page_offset(
        buffer_offset,
        buffer_length,
        read_offset,
        end_offset,
        zigzag,
    ) + 8
        + range.map_or(0, |range| {
            unsigned_immediate_width(range.minimum as u64)
                + unsigned_immediate_width(range.maximum as u64)
                + 24
        })
        + store_data_offset_width(target_offset, byte_size)
}
