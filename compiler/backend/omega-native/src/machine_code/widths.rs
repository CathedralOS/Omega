pub(super) fn host_call_sequence_width(
    architecture: omega_target::Architecture,
    operands: &[crate::instructions::InstructionOperand],
) -> usize {
    crate::architecture::host_call_sequence_width(architecture, operands)
}

pub(super) fn return_width(architecture: omega_target::Architecture) -> usize {
    crate::architecture::return_width(architecture)
}

pub(super) fn dispatch_loop_enter_width(architecture: omega_target::Architecture) -> usize {
    crate::architecture::dispatch_loop_enter_width(architecture)
}

pub(super) fn dispatch_case_enter_width(architecture: omega_target::Architecture) -> usize {
    crate::architecture::dispatch_case_enter_width(architecture)
}

pub(super) fn dispatch_state_write_width(architecture: omega_target::Architecture) -> usize {
    crate::architecture::dispatch_state_write_width(architecture)
}

pub(super) fn dispatch_case_leave_width(architecture: omega_target::Architecture) -> usize {
    crate::architecture::dispatch_case_leave_width(architecture)
}

pub(super) fn dispatch_guard_compare_static_width(
    architecture: omega_target::Architecture,
) -> usize {
    crate::architecture::dispatch_guard_compare_static_width(architecture)
}

pub(super) fn runtime_text_literal_compare_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    crate::architecture::runtime_text_literal_compare_width(architecture, literal)
}

pub(super) fn runtime_text_storage_compare_width(
    architecture: omega_target::Architecture,
) -> usize {
    crate::architecture::runtime_text_storage_compare_width(architecture)
}

pub(super) fn runtime_storage_compare_width(architecture: omega_target::Architecture) -> usize {
    crate::architecture::runtime_storage_compare_width(architecture)
}

pub(super) fn runtime_storage_value_compare_width(
    architecture: omega_target::Architecture,
) -> usize {
    crate::architecture::runtime_storage_value_compare_width(architecture)
}

pub(super) fn runtime_text_literal_write_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    crate::architecture::runtime_text_literal_write_width(architecture, literal)
}

pub(super) fn runtime_text_literal_segment_write_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    crate::architecture::runtime_text_literal_segment_write_width(architecture, literal)
}

pub(super) fn runtime_text_stored_suffix_append_width(
    architecture: omega_target::Architecture,
) -> usize {
    crate::architecture::runtime_text_stored_suffix_append_width(architecture)
}

pub(super) fn runtime_text_buffer_materialize_width(
    architecture: omega_target::Architecture,
) -> usize {
    crate::architecture::runtime_text_buffer_materialize_width(architecture)
}

pub(super) fn runtime_text_stored_place_append_width(
    architecture: omega_target::Architecture,
) -> usize {
    crate::architecture::runtime_text_stored_place_append_width(architecture)
}

pub(super) fn runtime_text_literal_append_width(
    architecture: omega_target::Architecture,
    literal: &str,
) -> usize {
    crate::architecture::runtime_text_literal_append_width(architecture, literal)
}

pub(super) fn runtime_machine_integer_write_width(
    architecture: omega_target::Architecture,
) -> usize {
    crate::architecture::runtime_machine_integer_write_width(architecture)
}

pub(super) fn runtime_machine_string_write_width(
    architecture: omega_target::Architecture,
    byte_length: usize,
) -> usize {
    crate::architecture::runtime_machine_string_write_width(architecture, byte_length)
}

pub(super) fn runtime_text_line_read_width(
    architecture: omega_target::Architecture,
    byte_capacity: usize,
    syscall_number: u32,
) -> usize {
    crate::architecture::runtime_text_line_read_width(architecture, byte_capacity, syscall_number)
}

pub(super) fn runtime_storage_copy_width(
    architecture: omega_target::Architecture,
    byte_count: usize,
) -> usize {
    crate::architecture::runtime_storage_copy_width(architecture, byte_count)
}
