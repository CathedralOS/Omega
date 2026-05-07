use crate::instructions::{InstructionOperand, InstructionOperandKind};
use omega_core::diagnostics::Diagnostic;

pub fn host_call_sequence_width(operands: &[InstructionOperand]) -> usize {
    operands.iter().map(operand_width).sum::<usize>() + 4
}

pub fn syscall_sequence_width(operands: &[InstructionOperand], syscall_number: u32) -> usize {
    operands.iter().map(operand_width).sum::<usize>()
        + unsigned_immediate_width(u64::from(syscall_number))
        + 4
}

pub fn return_width() -> usize {
    4
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

pub fn dispatch_guard_compare_static_width() -> usize {
    20
}

pub fn runtime_text_literal_compare_width(literal: &str) -> usize {
    8 + literal.len() * 12 + runtime_text_input_delimiter_check_width()
}

pub fn runtime_text_storage_compare_width() -> usize {
    84
}

pub fn runtime_storage_compare_width() -> usize {
    32
}

pub fn runtime_storage_value_compare_width() -> usize {
    20
}

fn runtime_text_input_delimiter_check_width() -> usize {
    32
}

pub fn runtime_text_literal_write_width(literal: &str) -> usize {
    8 + literal.len() * 8
}

pub fn runtime_text_literal_segment_write_width(literal: &str) -> usize {
    runtime_text_literal_write_width(literal)
}

pub fn runtime_text_stored_suffix_append_width() -> usize {
    72
}

pub fn runtime_text_stored_place_append_width() -> usize {
    80
}

pub fn runtime_text_literal_append_width(literal: &str) -> usize {
    40 + literal.len() * 8
}

pub fn runtime_text_buffer_materialize_width() -> usize {
    60
}

pub fn runtime_machine_integer_write_width() -> usize {
    16
}

pub fn runtime_machine_string_write_width(byte_length: usize) -> usize {
    24 + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_text_line_read_width(_byte_capacity: usize, _syscall_number: u32) -> usize {
    104
}

pub fn runtime_storage_copy_width(byte_count: usize) -> usize {
    16 + runtime_storage_copy_data_width(byte_count)
}

pub fn operand_width(operand: &InstructionOperand) -> usize {
    match &operand.kind {
        InstructionOperandKind::DataAddress { .. } => 8,
        InstructionOperandKind::RuntimeMachineStringPointer { .. }
        | InstructionOperandKind::RuntimeMachineStringLength { .. } => 12,
        InstructionOperandKind::ImmediateInteger(value) => immediate_width(*value),
        InstructionOperandKind::ByteLength(value) => unsigned_immediate_width(*value as u64),
    }
}

pub fn encode_host_call_sequence(operands: &[InstructionOperand]) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    let mut next_register = 0u8;

    for operand in operands {
        match &operand.kind {
            InstructionOperandKind::ImmediateInteger(value) => {
                bytes.extend(encode_immediate(next_register, *value)?);
                next_register += 1;
            }
            InstructionOperandKind::DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                next_register += 1;
            }
            InstructionOperandKind::RuntimeMachineStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                next_register += 1;
            }
            InstructionOperandKind::RuntimeMachineStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    byte_offset + 8,
                )?);
                next_register += 1;
            }
            InstructionOperandKind::ByteLength(value) => {
                bytes.extend(encode_unsigned_immediate(next_register, *value as u64));
                next_register += 1;
            }
        }
    }

    bytes.extend(encode_branch_link_placeholder());
    Ok(bytes)
}

pub fn encode_syscall_sequence(
    operands: &[InstructionOperand],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    let mut next_register = 0u8;

    for operand in operands {
        match &operand.kind {
            InstructionOperandKind::ImmediateInteger(value) => {
                bytes.extend(encode_immediate(next_register, *value)?);
                next_register += 1;
            }
            InstructionOperandKind::DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                next_register += 1;
            }
            InstructionOperandKind::RuntimeMachineStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                next_register += 1;
            }
            InstructionOperandKind::RuntimeMachineStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    byte_offset + 8,
                )?);
                next_register += 1;
            }
            InstructionOperandKind::ByteLength(value) => {
                bytes.extend(encode_unsigned_immediate(next_register, *value as u64));
                next_register += 1;
            }
        }
    }

    bytes.extend(encode_unsigned_immediate(8, u64::from(syscall_number)));
    bytes.extend(encode_svc());
    Ok(bytes)
}

pub fn encode_return() -> Vec<u8> {
    encode_instruction(0xD65F03C0)
}

pub fn encode_dispatch_loop_enter(entry_dispatch_index: u32) -> Result<Vec<u8>, Diagnostic> {
    let immediate = u16::try_from(entry_dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{entry_dispatch_index}` yet"
        ))
    })?;
    Ok(encode_movz_w(19, immediate))
}

pub fn encode_dispatch_case_enter(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_compare_w19_immediate(dispatch_index)?;
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
    let mut bytes = encode_movz_w(19, immediate);
    bytes.extend(encode_unconditional_branch(case_leave_byte_distance)?);
    Ok(bytes)
}

pub fn encode_dispatch_case_leave(loop_byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    encode_unconditional_branch(loop_byte_distance)
}

pub fn encode_dispatch_guard_compare_static(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_value = u32::try_from(expected_value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare negative guard value `{expected_value}` yet"
        ))
    })?;

    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_w17_from_x16(byte_offset, byte_size)?);
    bytes.extend(encode_compare_w17_immediate(expected_value)?);
    bytes.extend(if branch_when_equal {
        encode_conditional_branch_equal(skip_byte_distance)?
    } else {
        encode_conditional_branch_not_equal(skip_byte_distance)?
    });
    Ok(bytes)
}

pub fn encode_runtime_text_literal_compare(
    literal: &str,
    failure_branch_distances: Vec<isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    if literal.len() != failure_branch_distances.len() {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime text guard expected {} branch distance(s), got {}",
            literal.len(),
            failure_branch_distances.len()
        )));
    }

    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (byte_index, expected_byte) in literal.as_bytes().iter().enumerate() {
        bytes.extend(encode_load_byte_w17_from_x16(byte_index)?);
        bytes.extend(encode_compare_w17_immediate(u32::from(*expected_byte))?);
        bytes.extend(encode_conditional_branch_not_equal(
            failure_branch_distances[byte_index],
        )?);
    }

    bytes.extend(encode_runtime_text_input_delimiter_check(
        literal.len(),
        delimiter_failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_text_storage_compare(
    source_offset: usize,
    compare_failure_branch_distance: isize,
    delimiter_failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(18, 17, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 17, source_offset + 8)?);

    bytes.extend(encode_cbz_x(19, 28)?);
    bytes.extend(encode_load_byte_w_post_increment(20, 18, 1)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 16, 1)?);
    bytes.extend(encode_compare_w_register(20, 21));
    bytes.extend(if branch_when_equal {
        encode_conditional_branch_equal(compare_failure_branch_distance)?
    } else {
        encode_conditional_branch_not_equal(compare_failure_branch_distance)?
    });
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-20)?);
    bytes.extend(encode_runtime_text_input_delimiter_check(
        0,
        delimiter_failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_storage_compare(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_w_from_x(18, 16, left_offset, byte_size)?);
    bytes.extend(encode_load_w_from_x(19, 17, right_offset, byte_size)?);
    bytes.extend(encode_compare_w_register(18, 19));
    bytes.extend(if branch_when_equal {
        encode_conditional_branch_equal(failure_branch_distance)?
    } else {
        encode_conditional_branch_not_equal(failure_branch_distance)?
    });
    Ok(bytes)
}

pub fn encode_runtime_storage_value_compare(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_value = u32::try_from(expected_value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare negative runtime guard value `{expected_value}` yet"
        ))
    })?;

    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_w_from_x(17, 16, byte_offset, byte_size)?);
    bytes.extend(encode_compare_w17_immediate(expected_value)?);
    bytes.extend(if branch_when_equal {
        encode_conditional_branch_equal(failure_branch_distance)?
    } else {
        encode_conditional_branch_not_equal(failure_branch_distance)?
    });
    Ok(bytes)
}

pub fn encode_runtime_text_literal_write(literal: &str) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_literal_segment_write(0, literal)
}

pub fn encode_runtime_text_literal_segment_write(
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        bytes.extend(encode_movz_w(17, u16::from(*byte)));
        bytes.extend(encode_store_byte_w17_to_x16(byte_offset + byte_index)?);
    }

    Ok(bytes)
}

pub fn encode_runtime_text_stored_suffix_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(18, 17, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 17, source_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_add_x_immediate(22, 16, buffer_offset)?);

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(16, 17, target_offset)?);
    bytes.extend(encode_add_x_immediate(23, 23, length_delta)?);
    bytes.extend(encode_store_x_to_x(23, 17, target_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_text_stored_place_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(22, 17, target_offset + 8)?);
    bytes.extend(encode_move_x_register(24, 22));
    bytes.extend(encode_add_x_register(22, 16, 22));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(18, 20, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 20, source_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_add_x_register(24, 24, 23));
    bytes.extend(encode_store_x_to_x(16, 17, target_offset)?);
    bytes.extend(encode_store_x_to_x(24, 17, target_offset + 8)?);
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_literal_append(
    buffer_offset: usize,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(22, 17, target_offset + 8)?);
    bytes.extend(encode_move_x_register(20, 16));
    bytes.extend(encode_add_x_register(16, 16, 22));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        bytes.extend(encode_movz_w(18, u16::from(*byte)));
        bytes.extend(encode_store_byte_w_to_x(18, 16, byte_index)?);
    }

    bytes.extend(encode_store_x_to_x(20, 17, target_offset)?);
    bytes.extend(encode_add_x_immediate(22, 22, literal.len())?);
    bytes.extend(encode_store_x_to_x(22, 17, target_offset + 8)?);
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_buffer_materialize(target_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(18, 17, target_offset)?);
    bytes.extend(encode_load_x_from_x(19, 17, target_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_move_x_register(22, 16));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_store_x_to_x(16, 17, target_offset)?);
    bytes.extend(encode_store_x_to_x(23, 17, target_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_machine_integer_write(
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let value = u16::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store runtime integer value `{value}` yet"
        ))
    })?;

    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_movz_w(17, value));
    bytes.extend(encode_store_w17_to_x16(byte_offset, byte_size)?);
    Ok(bytes)
}

pub fn encode_runtime_machine_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(17);
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x17_to_x16(byte_offset)?);
    bytes.extend(encode_unsigned_immediate(17, byte_length as u64));
    bytes.extend(encode_store_x17_to_x16(byte_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_text_line_read(
    target_offset: usize,
    byte_capacity: usize,
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let max_payload_bytes = byte_capacity.saturating_sub(1);
    let capacity = u32::try_from(max_payload_bytes).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 runtime line read cannot encode capacity `{byte_capacity}` yet"
        ))
    })?;
    if capacity > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime line read cannot compare capacity `{byte_capacity}` yet"
        )));
    }
    if syscall_number > u32::from(u16::MAX) {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime line read cannot encode syscall `{syscall_number}` yet"
        )));
    }

    let mut bytes = Vec::new();
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(21, 20));
    bytes.extend(encode_movz(22, 0));

    bytes.extend(encode_movz(0, 0));
    bytes.extend(encode_move_x_register(1, 21));
    bytes.extend(encode_movz(2, 1));
    bytes.extend(encode_movz(8, syscall_number as u16));
    bytes.extend(encode_svc());
    bytes.extend(encode_cbz_x(0, 48)?);
    bytes.extend(encode_load_byte_w_from_x(24, 21, 0)?);
    bytes.extend(encode_compare_w_immediate(24, 10)?);
    bytes.extend(encode_conditional_branch_equal(36)?);
    bytes.extend(encode_compare_w_immediate(24, 13)?);
    bytes.extend(encode_conditional_branch_equal(28)?);
    bytes.extend(encode_compare_w_immediate(24, 0)?);
    bytes.extend(encode_conditional_branch_equal(20)?);
    bytes.extend(encode_add_x_immediate(21, 21, 1)?);
    bytes.extend(encode_add_x_immediate(22, 22, 1)?);
    bytes.extend(encode_compare_w_immediate(22, capacity)?);
    bytes.extend(encode_conditional_branch_not_equal(-64)?);

    bytes.extend(encode_store_byte_w_to_x(31, 21, 0)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x_to_x(20, 16, target_offset)?);
    bytes.extend(encode_store_x_to_x(22, 16, target_offset + 8)?);

    debug_assert_eq!(
        bytes.len(),
        runtime_text_line_read_width(byte_capacity, syscall_number)
    );
    Ok(bytes)
}

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));

    match byte_count {
        1 | 4 => {
            bytes.extend(encode_load_w_from_x(18, 16, source_offset, byte_count)?);
            bytes.extend(encode_store_w_to_x(18, 17, target_offset, byte_count)?);
        }
        _ if byte_count.is_multiple_of(8) => {
            for offset in (0..byte_count).step_by(8) {
                bytes.extend(encode_load_x_from_x(18, 16, source_offset + offset)?);
                bytes.extend(encode_store_x_to_x(18, 17, target_offset + offset)?);
            }
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot copy `{byte_count}` byte(s) of runtime storage yet"
            )));
        }
    }

    Ok(bytes)
}

fn encode_movz(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
}

fn encode_movz_w(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0x52800000 | (u32::from(immediate) << 5) | u32::from(register))
}

fn encode_movk(register: u8, immediate: u16, halfword_shift: u8) -> Vec<u8> {
    encode_instruction(
        0xF2800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

fn encode_move_x_register(destination_register: u8, source_register: u8) -> Vec<u8> {
    encode_instruction(
        0xAA0003E0 | (u32::from(source_register) << 16) | u32::from(destination_register),
    )
}

fn encode_adrp_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x90000000 | u32::from(register))
}

fn encode_add_page_offset_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x91000000 | (u32::from(register) << 5) | u32::from(register))
}

fn encode_branch_link_placeholder() -> Vec<u8> {
    encode_instruction(0x94000000)
}

fn encode_svc() -> Vec<u8> {
    encode_instruction(0xD4000001)
}

fn encode_compare_w19_immediate(value: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_compare_w_immediate(19, value)
}

fn encode_compare_w17_immediate(value: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_compare_w_immediate(17, value)
}

fn encode_compare_w_immediate(register: u8, value: u32) -> Result<Vec<u8>, Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare guard value `{value}` yet"
        )));
    }

    Ok(encode_instruction(
        0x7100001F | (value << 10) | (u32::from(register) << 5),
    ))
}

fn encode_compare_w_register(left_register: u8, right_register: u8) -> Vec<u8> {
    encode_instruction(
        0x6B00001F | (u32::from(right_register) << 16) | (u32::from(left_register) << 5),
    )
}

fn encode_load_w17_from_x16(byte_offset: usize, byte_size: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_load_w_from_x(17, 16, byte_offset, byte_size)
}

fn encode_load_w_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match byte_size {
        1 => encode_load_byte_w_from_x(destination_register, base_register, byte_offset),
        4 => {
            if !byte_offset.is_multiple_of(4) || byte_offset / 4 > 4095 {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load u32 guard at offset `{byte_offset}` yet"
                )));
            }
            Ok(encode_instruction(
                0xB9400000
                    | (((byte_offset / 4) as u32) << 10)
                    | (u32::from(base_register) << 5)
                    | u32::from(destination_register),
            ))
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot load {byte_size}-byte guard operands yet"
        ))),
    }
}

fn encode_load_byte_w17_from_x16(byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_load_byte_w_from_x(17, 16, byte_offset)
}

fn encode_runtime_text_input_delimiter_check(
    byte_offset: usize,
    failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_load_byte_w17_from_x16(byte_offset)?;
    bytes.extend(encode_compare_w17_immediate(10)?);
    bytes.extend(encode_conditional_branch_equal(24)?);
    bytes.extend(encode_compare_w17_immediate(13)?);
    bytes.extend(encode_conditional_branch_equal(16)?);
    bytes.extend(encode_compare_w17_immediate(0)?);
    bytes.extend(encode_conditional_branch_equal(8)?);
    bytes.extend(encode_unconditional_branch(failure_branch_distance)?);
    Ok(bytes)
}

fn encode_load_byte_w_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if byte_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot load byte at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0x39400000
            | ((byte_offset as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(destination_register),
    ))
}

fn encode_store_w17_to_x16(byte_offset: usize, byte_size: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_store_w_to_x(17, 16, byte_offset, byte_size)
}

fn encode_store_w_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match byte_size {
        1 => encode_store_byte_w_to_x(source_register, base_register, byte_offset),
        4 => {
            if !byte_offset.is_multiple_of(4) || byte_offset / 4 > 4095 {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot store u32 at offset `{byte_offset}` yet"
                )));
            }
            Ok(encode_instruction(
                0xB9000000
                    | (((byte_offset / 4) as u32) << 10)
                    | (u32::from(base_register) << 5)
                    | u32::from(source_register),
            ))
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
        ))),
    }
}

fn encode_store_x17_to_x16(byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_store_x_to_x(17, 16, byte_offset)
}

fn encode_store_x_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store u64 at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0xF9000000
            | (((byte_offset / 8) as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

fn encode_load_x_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot load u64 at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0xF9400000
            | (((byte_offset / 8) as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(destination_register),
    ))
}

fn encode_store_byte_w17_to_x16(byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_store_byte_w_to_x(17, 16, byte_offset)
}

fn encode_store_byte_w_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if byte_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store byte at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0x39000000
            | ((byte_offset as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

fn encode_load_byte_w_post_increment(
    destination_register: u8,
    base_register: u8,
    byte_increment: i16,
) -> Result<Vec<u8>, Diagnostic> {
    let immediate = signed_memory_immediate_9(byte_increment, "post-increment byte load")?;
    Ok(encode_instruction(
        0x38400400
            | (immediate << 12)
            | (u32::from(base_register) << 5)
            | u32::from(destination_register),
    ))
}

fn encode_store_byte_w_post_increment(
    source_register: u8,
    base_register: u8,
    byte_increment: i16,
) -> Result<Vec<u8>, Diagnostic> {
    let immediate = signed_memory_immediate_9(byte_increment, "post-increment byte store")?;
    Ok(encode_instruction(
        0x38000400
            | (immediate << 12)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

fn encode_add_x_immediate(
    destination_register: u8,
    source_register: u8,
    value: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot add immediate `{value}` yet"
        )));
    }
    Ok(encode_instruction(
        0x91000000
            | ((value as u32) << 10)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    ))
}

fn encode_add_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Vec<u8> {
    encode_instruction(
        0x8B000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

fn encode_subs_x_immediate(
    destination_register: u8,
    source_register: u8,
    value: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot subtract immediate `{value}` yet"
        )));
    }
    Ok(encode_instruction(
        0xF1000000
            | ((value as u32) << 10)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    ))
}

fn encode_conditional_branch_not_equal(byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.ne")?;
    Ok(encode_instruction(
        0x54000001 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

fn encode_conditional_branch_equal(byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.eq")?;
    Ok(encode_instruction(
        0x54000000 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

fn encode_cbz_x(register: u8, byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "cbz")?;
    Ok(encode_instruction(
        0xB4000000 | ((instruction_distance as u32 & 0x7ffff) << 5) | u32::from(register),
    ))
}

fn encode_unconditional_branch(byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 26, "b")?;
    Ok(encode_instruction(
        0x14000000 | (instruction_distance as u32 & 0x03ff_ffff),
    ))
}

fn signed_memory_immediate_9(value: i16, instruction_name: &str) -> Result<u32, Diagnostic> {
    if !(-256..=255).contains(&value) {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} immediate is out of range: {value}"
        )));
    }
    Ok((i32::from(value) as u32) & 0x1ff)
}

fn checked_instruction_distance(
    byte_distance: isize,
    immediate_bits: u8,
    instruction_name: &str,
) -> Result<isize, Diagnostic> {
    if byte_distance % 4 != 0 {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is not instruction aligned: {byte_distance} byte(s)"
        )));
    }

    let instruction_distance = byte_distance / 4;
    let min = -(1isize << (immediate_bits - 1));
    let max = (1isize << (immediate_bits - 1)) - 1;
    if instruction_distance < min || instruction_distance > max {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is out of range: {instruction_distance} instruction(s)"
        )));
    }

    Ok(instruction_distance)
}

fn encode_instruction(instruction: u32) -> Vec<u8> {
    instruction.to_le_bytes().to_vec()
}

fn encode_immediate(register: u8, value: i64) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode negative immediate `{value}` yet"
        ))
    })?;

    Ok(encode_unsigned_immediate(register, value))
}

fn encode_unsigned_immediate(register: u8, value: u64) -> Vec<u8> {
    let mut bytes = encode_movz(register, halfword(value, 0));

    for halfword_shift in 1..4 {
        let immediate = halfword(value, halfword_shift);
        if immediate != 0 {
            bytes.extend(encode_movk(register, immediate, halfword_shift));
        }
    }

    bytes
}

fn immediate_width(value: i64) -> usize {
    match u64::try_from(value) {
        Ok(value) => unsigned_immediate_width(value),
        Err(_) => 4,
    }
}

fn unsigned_immediate_width(value: u64) -> usize {
    let high_nonzero_halfwords = (1..4)
        .filter(|halfword_shift| halfword(value, *halfword_shift) != 0)
        .count();

    4 + high_nonzero_halfwords * 4
}

fn runtime_storage_copy_data_width(byte_count: usize) -> usize {
    match byte_count {
        1 | 4 => 8,
        _ if byte_count.is_multiple_of(8) => (byte_count / 8) * 8,
        _ => 0,
    }
}

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
