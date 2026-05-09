use crate::machine_code::branch_distances::{
    byte_distance_to_next_runtime_write_end,
    byte_distance_to_next_runtime_write_end_from_branch_offset,
    byte_distances_to_next_runtime_machine_write_end,
};
use crate::machine_code::widths::runtime_text_storage_compare_width;
use crate::plan::NativePlan;
use crate::state_guards::StateGuardOperator;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_machine_program::MachineInstruction;

pub(super) fn encode_runtime_text_literal_compare(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_compare(
        native_plan.target.architecture,
        literal,
        byte_distances_to_next_runtime_machine_write_end(
            native_plan,
            machine_instructions,
            machine_instruction_index,
            literal,
        )?,
        byte_distance_to_next_runtime_write_end(
            native_plan,
            machine_instructions,
            machine_instruction_index,
        )?,
    )
}

pub(super) fn encode_runtime_text_storage_compare(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    source_offset: usize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_storage_compare(
        native_plan.target.architecture,
        source_offset,
        byte_distance_to_next_runtime_write_end_from_branch_offset(
            native_plan,
            machine_instructions,
            machine_instruction_index,
            40,
        )?,
        byte_distance_to_next_runtime_write_end_from_branch_offset(
            native_plan,
            machine_instructions,
            machine_instruction_index,
            runtime_text_storage_compare_width(native_plan.target.architecture).saturating_sub(4),
        )?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_runtime_text_literal_write(
    native_plan: &NativePlan,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_write(native_plan.target.architecture, literal)
}

pub(super) fn encode_runtime_text_literal_segment_write(
    native_plan: &NativePlan,
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_segment_write(
        native_plan.target.architecture,
        byte_offset,
        literal,
    )
}

pub(super) fn encode_runtime_text_stored_suffix_append(
    native_plan: &NativePlan,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_stored_suffix_append(
        native_plan.target.architecture,
        buffer_offset,
        source_offset,
        target_offset,
        length_delta,
    )
}

pub(super) fn encode_runtime_text_stored_place_append(
    native_plan: &NativePlan,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_stored_place_append(
        native_plan.target.architecture,
        0,
        source_offset,
        target_offset,
    )
}

pub(super) fn encode_runtime_text_literal_append(
    native_plan: &NativePlan,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_append(
        native_plan.target.architecture,
        0,
        target_offset,
        literal,
    )
}

pub(super) fn encode_runtime_text_buffer_materialize(
    native_plan: &NativePlan,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_buffer_materialize(
        native_plan.target.architecture,
        target_offset,
    )
}

pub(super) fn encode_runtime_text_line_read(
    native_plan: &NativePlan,
    target_offset: usize,
    byte_capacity: usize,
    syscall_number: u32,
    syscall_number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_line_read(
        native_plan.target.architecture,
        target_offset,
        byte_capacity,
        syscall_number,
        syscall_number_register,
        supervisor_call,
    )
}
