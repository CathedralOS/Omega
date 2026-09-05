//! Independent validator for x86-64 zero-extending `MOV r32, imm32` rows.

use isa_x86_64::validate_x86_64_mov_r32_imm32_i64_materialization;
use physical_instructions::PostAllocationMachineInstruction;
use post_allocation_machine_to_post_allocation_machine::{
    X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT, X86MovR32Imm32InstructionDisposition,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstruction, SelectedInstructionKind};
use target::Architecture;

use super::{decoded_footprint, integer_bits, validate_baseline, validate_external_operands};
use crate::{OptimizedSelectedFormEncodingError, SelectedFormEncodingState};

pub(super) fn validate(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &X86MovR32Imm32InstructionDisposition,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let X86MovR32Imm32InstructionDisposition::MovR32Imm32MaterializationV1 {
        literal_bits,
        destination,
        baseline_byte_count,
        selected_byte_count,
        ..
    } = disposition
    else {
        return validate_baseline(architecture, selected.id, kind, machine, physical, state);
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let destination_matches = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1)
        .is_some_and(|operand| {
            architecture == Architecture::X86_64
                && destination.instruction == selected.id
                && destination.operand == operand.operand
                && destination.virtual_register == operand.virtual_register
                && destination.class == operand.class
                && destination.destination_view == operand.view
                && destination.destination_storage_units == operand.storage_units
                && destination.destination_write_units == operand.write_units
                && Some(destination.destination_write_semantics) == operand.write_semantics
        });
    if !destination_matches
        || integer_bits(value) != Some(*literal_bits)
        || *baseline_byte_count != X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let SelectedFormEncodingState::Encoded { bytes, footprint } = state else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let decoded = validate_x86_64_mov_r32_imm32_i64_materialization(
        physical,
        destination.destination_view,
        value,
        bytes,
    )
    .map_err(OptimizedSelectedFormEncodingError::X86_64MovR32Imm32)?;
    let decoded_footprint = decoded_footprint(
        &decoded.footprint().register_reads,
        &decoded.footprint().register_writes,
        &decoded.footprint().encoded,
    );
    validate_external_operands(selected.id, machine, &decoded_footprint)?;
    if bytes.len() != usize::from(*selected_byte_count)
        || decoded.value_bits() != *literal_bits
        || decoded.destination() != destination.destination_view
        || decoded.encoded_write_view() != destination.encoded_view
        || decoded.footprint().writes_rflags
        || decoded.footprint().encoded_write_view != destination.encoded_view
        || decoded.footprint().encoded_write_view_units != destination.encoded_storage_units
        || decoded.footprint().encoded_write_units != destination.encoded_write_units
        || decoded.footprint().encoded_write_semantics != destination.encoded_write_semantics
        || decoded.footprint().encoded != machine.alternative.encoded
        || footprint.as_ref() != &decoded_footprint
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}
