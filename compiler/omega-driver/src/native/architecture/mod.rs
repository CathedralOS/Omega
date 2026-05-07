pub mod aarch64;

use crate::diagnostics::Diagnostic;
use crate::native::instructions::InstructionOperand;
use crate::native::target::Architecture;

pub fn host_call_sequence_width(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::host_call_sequence_width(operands),
        Architecture::X86_64 => operands.len() * 8 + 5,
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

pub fn runtime_storage_compare_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_compare_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_write_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_write_width(literal),
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
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_compare(literal, failure_branch_distances)
        }
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

pub fn encode_runtime_text_literal_write(
    architecture: Architecture,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_write(literal),
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
