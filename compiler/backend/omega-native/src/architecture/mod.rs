pub mod aarch64;

use crate::instructions::InstructionOperand;
use crate::target::Architecture;
use omega_core::diagnostics::Diagnostic;

pub fn host_call_sequence_width(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::host_call_sequence_width(operands),
        Architecture::X86_64 => operands.len() * 8 + 5,
    }
}

pub fn syscall_sequence_width(
    architecture: Architecture,
    operands: &[InstructionOperand],
    syscall_number: u32,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::syscall_sequence_width(operands, syscall_number),
        Architecture::X86_64 => operands.len() * 8 + 7,
    }
}

pub fn return_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::return_width(),
        Architecture::X86_64 => 1,
    }
}

pub fn dispatch_loop_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_loop_enter_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn dispatch_case_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_case_enter_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn dispatch_state_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_state_write_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn dispatch_case_leave_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_case_leave_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn dispatch_guard_compare_static_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_guard_compare_static_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_compare_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_compare_width(literal),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_storage_compare_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_storage_compare_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_compare_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_compare_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_value_compare_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_value_compare_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_write_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_write_width(literal),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_segment_write_width(
    architecture: Architecture,
    literal: &str,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_segment_write_width(literal),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_stored_suffix_append_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_stored_suffix_append_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_stored_place_append_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_stored_place_append_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_append_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_width(literal),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_buffer_materialize_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_buffer_materialize_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_machine_integer_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_integer_write_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_machine_string_write_width(architecture: Architecture, byte_length: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_string_write_width(byte_length),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_line_read_width(
    architecture: Architecture,
    byte_capacity: usize,
    syscall_number: u32,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_line_read_width(byte_capacity, syscall_number)
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_width(architecture: Architecture, byte_count: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_width(byte_count),
        Architecture::X86_64 => 0,
    }
}

pub fn encode_host_call_sequence(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_host_call_sequence(operands),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_syscall_sequence(
    architecture: Architecture,
    operands: &[InstructionOperand],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_syscall_sequence(operands, syscall_number),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_return(architecture: Architecture) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Ok(aarch64::encode_return()),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_dispatch_loop_enter(
    architecture: Architecture,
    entry_dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_loop_enter(entry_dispatch_index),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_dispatch_case_enter(
    architecture: Architecture,
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_dispatch_case_enter(dispatch_index, skip_byte_distance)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_dispatch_state_write(
    architecture: Architecture,
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_dispatch_state_write(dispatch_index, case_leave_byte_distance)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_dispatch_case_leave(
    architecture: Architecture,
    loop_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_case_leave(loop_byte_distance),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_dispatch_guard_compare_static(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_guard_compare_static(
            byte_offset,
            byte_size,
            expected_value,
            skip_byte_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_literal_compare(
    architecture: Architecture,
    literal: &str,
    failure_branch_distances: Vec<isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_compare(
            literal,
            failure_branch_distances,
            delimiter_failure_branch_distance,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_storage_compare(
    architecture: Architecture,
    source_offset: usize,
    compare_failure_branch_distance: isize,
    delimiter_failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_storage_compare(
            source_offset,
            compare_failure_branch_distance,
            delimiter_failure_branch_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_storage_compare(
    architecture: Architecture,
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_compare(
            left_offset,
            right_offset,
            byte_size,
            failure_branch_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_storage_value_compare(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_value_compare(
            byte_offset,
            byte_size,
            expected_value,
            failure_branch_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_literal_write(
    architecture: Architecture,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_write(literal),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_literal_segment_write(
    architecture: Architecture,
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_segment_write(byte_offset, literal)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_stored_suffix_append(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_stored_suffix_append(
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_stored_place_append(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_stored_place_append(
            buffer_offset,
            source_offset,
            target_offset,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_literal_append(
    architecture: Architecture,
    buffer_offset: usize,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_append(buffer_offset, target_offset, literal)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_buffer_materialize(
    architecture: Architecture,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_buffer_materialize(target_offset),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_machine_integer_write(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_machine_integer_write(byte_offset, byte_size, value)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_machine_string_write(
    architecture: Architecture,
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_machine_string_write(byte_offset, byte_length)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_text_line_read(
    architecture: Architecture,
    target_offset: usize,
    byte_capacity: usize,
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_line_read(target_offset, byte_capacity, syscall_number)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_storage_copy(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy(source_offset, target_offset, byte_count)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn operand_width(architecture: Architecture, operand: &InstructionOperand) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::operand_width(operand),
        Architecture::X86_64 => x86_64_operand_width(operand),
    }
}

fn x86_64_operand_width(_operand: &InstructionOperand) -> usize {
    8
}
