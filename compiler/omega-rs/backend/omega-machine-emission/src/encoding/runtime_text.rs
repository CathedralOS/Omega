use crate::MachineEmissionContext;
use crate::branch_distances::{
    byte_distance_to_next_runtime_write_end, byte_distances_to_next_runtime_machine_write_end,
};
use crate::layout::LaidOutMachineInstruction;
use omega_assigned_target_operations::{RuntimeTextReadSource, RuntimeTextReadTarget};
use omega_instruction_selection as architecture;
use psi_diagnostics::Diagnostic;

use crate::host_bindings::runtime_text_call_plans;

pub(super) fn encode_runtime_text_literal_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_compare(
        input.target.architecture,
        literal,
        byte_distances_to_next_runtime_machine_write_end(
            input.target.architecture,
            input,
            machine_instructions,
            machine_instruction_index,
            literal,
        )?,
        byte_distance_to_next_runtime_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
        )?,
    )
}

pub(super) fn encode_runtime_text_literal_write(
    input: MachineEmissionContext<'_>,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_write(input.target.architecture, literal)
}

pub(super) fn encode_runtime_text_literal_segment_write(
    input: MachineEmissionContext<'_>,
    byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_segment_write(
        input.target.architecture,
        byte_offset,
        literal,
    )
}

pub(super) fn encode_runtime_text_stored_suffix_append(
    input: MachineEmissionContext<'_>,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_stored_suffix_append(
        input.target.architecture,
        buffer_offset,
        source_offset,
        target_offset,
        length_delta,
    )
}

pub(super) fn encode_runtime_text_line_read(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_capacity: usize,
    source: &RuntimeTextReadSource,
    target: RuntimeTextReadTarget,
) -> Result<Vec<u8>, Diagnostic> {
    let RuntimeTextReadSource::HostOperation { operation_key } = source;
    let Some(binding) = input
        .assigned_target_operations
        .host_binding(*operation_key)
    else {
        return Err(Diagnostic::error(format!(
            "missing host binding for runtime text read operation {}.{}",
            operation_key.capability_name(),
            operation_key.operation_name()
        )));
    };
    architecture::encode_runtime_text_line_read_with_plans(
        input.target.architecture,
        target_offset,
        byte_capacity,
        &binding.mechanism,
        target,
        runtime_text_call_plans(input, *operation_key, binding)?,
    )
}

pub(super) fn encode_runtime_byte_read(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    payload_offset: usize,
    source: &RuntimeTextReadSource,
) -> Result<Vec<u8>, Diagnostic> {
    let RuntimeTextReadSource::HostOperation { operation_key } = source;
    let Some(binding) = input
        .assigned_target_operations
        .host_binding(*operation_key)
    else {
        return Err(Diagnostic::error(format!(
            "missing host binding for runtime byte read operation {}.{}",
            operation_key.capability_name(),
            operation_key.operation_name()
        )));
    };
    architecture::encode_runtime_byte_read_with_plans(
        input.target.architecture,
        target_offset,
        payload_offset,
        &binding.mechanism,
        runtime_text_call_plans(input, *operation_key, binding)?,
    )
}

pub(super) fn encode_runtime_byte_write(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    source: &RuntimeTextReadSource,
) -> Result<Vec<u8>, Diagnostic> {
    let RuntimeTextReadSource::HostOperation { operation_key } = source;
    let Some(binding) = input
        .assigned_target_operations
        .host_binding(*operation_key)
    else {
        return Err(Diagnostic::error(format!(
            "missing host binding for runtime byte write operation {}.{}",
            operation_key.capability_name(),
            operation_key.operation_name()
        )));
    };
    architecture::encode_runtime_byte_write_with_plans(
        input.target.architecture,
        source_offset,
        &binding.mechanism,
        runtime_text_call_plans(input, *operation_key, binding)?,
    )
}
