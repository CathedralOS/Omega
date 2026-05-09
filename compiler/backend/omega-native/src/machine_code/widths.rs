pub(super) fn host_call_sequence_width(
    architecture: omega_target::Architecture,
    operands: &[omega_target_program::InstructionOperand],
) -> usize {
    omega_instruction_selection::host_call_sequence_width(architecture, operands)
}

pub(super) fn return_width(architecture: omega_target::Architecture) -> usize {
    omega_instruction_selection::return_width(architecture)
}

pub(super) fn dispatch_loop_enter_width(architecture: omega_target::Architecture) -> usize {
    omega_instruction_selection::dispatch_loop_enter_width(architecture)
}

pub(super) fn dispatch_case_enter_width(architecture: omega_target::Architecture) -> usize {
    omega_instruction_selection::dispatch_case_enter_width(architecture)
}

pub(super) fn dispatch_state_write_width(architecture: omega_target::Architecture) -> usize {
    omega_instruction_selection::dispatch_state_write_width(architecture)
}

pub(super) fn dispatch_case_leave_width(architecture: omega_target::Architecture) -> usize {
    omega_instruction_selection::dispatch_case_leave_width(architecture)
}

pub(super) fn dispatch_guard_compare_static_width(
    architecture: omega_target::Architecture,
) -> usize {
    omega_instruction_selection::dispatch_guard_compare_static_width(architecture)
}

pub(super) fn runtime_text_literal_compare_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    omega_instruction_selection::runtime_text_literal_compare_width(architecture, literal)
}

pub(super) fn runtime_text_storage_compare_width(
    architecture: omega_target::Architecture,
) -> usize {
    omega_instruction_selection::runtime_text_storage_compare_width(architecture)
}

pub(super) fn runtime_storage_compare_width(architecture: omega_target::Architecture) -> usize {
    omega_instruction_selection::runtime_storage_compare_width(architecture)
}

pub(super) fn runtime_storage_value_compare_width(
    architecture: omega_target::Architecture,
) -> usize {
    omega_instruction_selection::runtime_storage_value_compare_width(architecture)
}

pub(super) fn runtime_text_literal_write_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    omega_instruction_selection::runtime_text_literal_write_width(architecture, literal)
}

pub(super) fn runtime_text_literal_segment_write_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    omega_instruction_selection::runtime_text_literal_segment_write_width(architecture, literal)
}

pub(super) fn runtime_text_stored_suffix_append_width(
    architecture: omega_target::Architecture,
) -> usize {
    omega_instruction_selection::runtime_text_stored_suffix_append_width(architecture)
}

pub(super) fn runtime_text_buffer_materialize_width(
    architecture: omega_target::Architecture,
) -> usize {
    omega_instruction_selection::runtime_text_buffer_materialize_width(architecture)
}

pub(super) fn runtime_text_stored_place_append_width(
    architecture: omega_target::Architecture,
) -> usize {
    omega_instruction_selection::runtime_text_stored_place_append_width(architecture)
}

pub(super) fn runtime_text_literal_append_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    omega_instruction_selection::runtime_text_literal_append_width(architecture, literal)
}

pub(super) fn runtime_machine_integer_write_width(
    architecture: omega_target::Architecture,
) -> usize {
    omega_instruction_selection::runtime_machine_integer_write_width(architecture)
}

pub(super) fn runtime_machine_string_write_width(
    architecture: omega_target::Architecture,
    byte_length: usize,
) -> usize {
    omega_instruction_selection::runtime_machine_string_write_width(architecture, byte_length)
}

pub(super) fn runtime_text_line_read_width(
    architecture: omega_target::Architecture,
    byte_capacity: usize,
    syscall_number: u32,
) -> usize {
    omega_instruction_selection::runtime_text_line_read_width(
        architecture,
        byte_capacity,
        syscall_number,
    )
}

pub(super) fn runtime_storage_copy_width(
    architecture: omega_target::Architecture,
    byte_count: usize,
) -> usize {
    omega_instruction_selection::runtime_storage_copy_width(architecture, byte_count)
}
