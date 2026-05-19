use crate::aarch64_call_operand;
use omega_core::arena::Arena;
use omega_isa_aarch64::aarch64;
use omega_target::Architecture;
use omega_target_operations::{
    InstructionOperand, RuntimeTextReadSource, RuntimeValueOperand, RuntimeValueOperandHandle,
    StateGuardOperator,
};

pub fn host_call_sequence_width(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::host_call_sequence_width_from_operands(
            operands.iter().map(aarch64_call_operand),
        ),
        Architecture::X86_64 => operands.len() * 8 + 5,
    }
}

pub fn syscall_sequence_width(
    architecture: Architecture,
    operands: &[InstructionOperand],
    syscall_number: u32,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::syscall_sequence_width_from_operands(
            operands.iter().map(aarch64_call_operand),
            syscall_number,
        ),
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

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_append_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_width(literal),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_literal_append_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_buffer_materialize_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_buffer_materialize_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_machine_integer_write_width(architecture: Architecture, byte_size: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_integer_write_width(byte_size),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_pointee_integer_write_width(architecture: Architecture, byte_size: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_integer_write_width(byte_size),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_pointee_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_value_compare_width(
    architecture: Architecture,
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    _byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_value_compare_width(runtime_value_operands, left, right)
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_value_operand_width(
    architecture: Architecture,
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    operand: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_value_operand_width(runtime_value_operands, operand)
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_frame_indexed_integer_write_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_integer_write_width(
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_frame_indexed_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_binary_write_width(
            runtime_value_operands,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_machine_string_write_width(architecture: Architecture, byte_length: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_string_write_width(byte_length),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_pointee_string_write_width(architecture: Architecture, byte_length: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_string_write_width(byte_length),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_frame_indexed_string_write_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_string_write_width(
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_address_to_runtime_frame_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_address_to_runtime_frame_write_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_pointee_address_to_runtime_frame_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_address_to_runtime_frame_write_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_line_read_width(
    architecture: Architecture,
    byte_capacity: usize,
    source: &RuntimeTextReadSource,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match source {
            RuntimeTextReadSource::Import { .. } => {
                aarch64::runtime_text_line_read_import_width(byte_capacity)
            }
            RuntimeTextReadSource::Syscall { number, .. } => {
                aarch64::runtime_text_line_read_syscall_width(byte_capacity, *number)
            }
        },
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    source: &RuntimeTextReadSource,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match source {
            RuntimeTextReadSource::Import { .. } => {
                aarch64::runtime_text_line_read_import_target_address_offset()
            }
            RuntimeTextReadSource::Syscall { number, .. } => {
                aarch64::runtime_text_line_read_syscall_target_address_offset(*number)
            }
        },
        Architecture::X86_64 => 8,
    }
}

pub fn runtime_text_line_read_import_call_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_line_read_import_call_offset(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_width(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_width(source_offset, target_offset, byte_count)
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_to_runtime_frame_indexed_width(
    architecture: Architecture,
    source_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_to_runtime_frame_indexed_width(
            source_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_from_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
    architecture: Architecture,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_to_runtime_pointee_width(
    architecture: Architecture,
    source_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_to_runtime_pointee_width(
            source_offset,
            field_byte_offset,
            byte_count,
        ),
        Architecture::X86_64 => 0,
    }
}
