//! Independent validator for x86-64 XOR-zero materialization rows.

use omega_isa_x86_64::validate_x86_64_xor_zero_i64_materialization;
use omega_machine_optimizer::{
    PostAllocationMachineInstruction, X86_MOVABS_I64_BYTE_COUNT, X86_XOR_R64_SELF_BYTE_COUNT,
    X86XorZeroInstructionDisposition,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstruction, SelectedInstructionKind};
use omega_target::Architecture;

use super::{decoded_footprint, integer_bits, validate_baseline, validate_external_operands};
use crate::{OptimizedSelectedFormEncodingError, SelectedFormEncodingState};

pub(super) fn validate(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &X86XorZeroInstructionDisposition,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
        destination,
        rflags_units,
        baseline_byte_count,
        selected_byte_count,
    } = disposition
    else {
        return validate_baseline(architecture, selected.id, kind, machine, physical, state);
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let operand = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1);
    let destination_matches = operand.is_some_and(|operand| {
        destination.instruction == selected.id
            && destination.operand == operand.operand
            && destination.virtual_register == operand.virtual_register
            && destination.class == operand.class
            && destination.view == operand.view
            && destination.storage_units == operand.storage_units
            && destination.write_units == operand.write_units
            && Some(destination.write_semantics) == operand.write_semantics
    });
    if architecture != Architecture::X86_64
        || !destination_matches
        || integer_bits(value) != Some(0)
        || *baseline_byte_count != X86_MOVABS_I64_BYTE_COUNT
        || *selected_byte_count != X86_XOR_R64_SELF_BYTE_COUNT
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let SelectedFormEncodingState::Encoded { bytes, footprint } = state else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let decoded = validate_x86_64_xor_zero_i64_materialization(physical, destination.view, bytes)
        .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
    let decoded_footprint = decoded_footprint(
        &decoded.footprint().register_reads,
        &decoded.footprint().register_writes,
        &decoded.footprint().encoded,
    );
    validate_external_operands(selected.id, machine, &decoded_footprint)?;
    if bytes.len() != usize::from(X86_XOR_R64_SELF_BYTE_COUNT)
        || decoded.value_bits() != 0
        || decoded.destination() != destination.view
        || !decoded.footprint().writes_rflags
        || !decoded.footprint().register_reads.is_empty()
        || decoded.footprint().register_writes.as_slice() != [destination.view]
        || !decoded.footprint().encoded.implicit_unit_uses.is_empty()
        || !decoded.footprint().encoded.implicit_unit_defs.is_empty()
        || decoded.footprint().encoded.implicit_unit_clobbers != *rflags_units
        || footprint.as_ref() != &decoded_footprint
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}
