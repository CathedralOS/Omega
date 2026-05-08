use crate::instructions::{InstructionOperand, InstructionOperandKind};
use omega_core::diagnostics::Diagnostic;

mod primitives;
mod widths;

use primitives::*;
pub use widths::*;

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
    number_register: u8,
    supervisor_call: u16,
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

    bytes.extend(encode_unsigned_immediate(
        number_register,
        u64::from(syscall_number),
    ));
    bytes.extend(encode_svc(supervisor_call));
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
    syscall_number_register: u8,
    supervisor_call: u16,
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
    let mut bytes = Vec::new();
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(21, 20));
    bytes.extend(encode_movz(22, 0));

    let read_loop_offset = bytes.len();
    bytes.extend(encode_movz(0, 0));
    bytes.extend(encode_move_x_register(1, 21));
    bytes.extend(encode_movz(2, 1));
    bytes.extend(encode_unsigned_immediate(
        syscall_number_register,
        u64::from(syscall_number),
    ));
    bytes.extend(encode_svc(supervisor_call));
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
    let repeat_read_distance = read_loop_offset as isize - bytes.len() as isize;
    bytes.extend(encode_conditional_branch_not_equal(repeat_read_distance)?);

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
