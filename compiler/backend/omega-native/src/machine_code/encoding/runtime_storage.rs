use crate::architecture;
use crate::machine_code::branch_distances::byte_distance_to_next_runtime_write_end;
use crate::machine_code::model::MachineInstruction;
use crate::plan::NativePlan;
use crate::state_guards::StateGuardOperator;
use omega_core::diagnostics::Diagnostic;

pub(super) fn encode_runtime_storage_compare(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_compare(
        native_plan.target.architecture,
        left_offset,
        right_offset,
        byte_size,
        byte_distance_to_next_runtime_write_end(
            native_plan,
            machine_instructions,
            machine_instruction_index,
        )?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_runtime_storage_value_compare(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_value_compare(
        native_plan.target.architecture,
        byte_offset,
        byte_size,
        expected_value,
        byte_distance_to_next_runtime_write_end(
            native_plan,
            machine_instructions,
            machine_instruction_index,
        )?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_runtime_machine_integer_write(
    native_plan: &NativePlan,
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_integer_write(
        native_plan.target.architecture,
        byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_machine_string_write(
    native_plan: &NativePlan,
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_string_write(
        native_plan.target.architecture,
        byte_offset,
        byte_length,
    )
}

pub(super) fn encode_runtime_storage_copy(
    native_plan: &NativePlan,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy(
        native_plan.target.architecture,
        source_offset,
        target_offset,
        byte_count,
    )
}
