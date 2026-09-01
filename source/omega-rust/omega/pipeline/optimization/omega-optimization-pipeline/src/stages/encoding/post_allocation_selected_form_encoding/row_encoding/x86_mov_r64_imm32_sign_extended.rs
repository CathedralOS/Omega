//! Exact row encoder for x86-64 `MOV r64, imm32` sign-extended materialization.

use omega_isa_x86_64::encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization;
use omega_machine_optimizer::{
    PostAllocationMachineInstruction, X86MovR64Imm32SignExtendedInstructionDisposition,
    X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstruction, SelectedInstructionKind};
use omega_target::Architecture;

use super::{encode_scalar, integer_bits, validate_operand_footprint};
use crate::{
    OptimizedSelectedFormEncodingError, SelectedFormDecodedFootprint, SelectedFormEncodingState,
};

pub(super) fn encode(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &X86MovR64Imm32SignExtendedInstructionDisposition,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let X86MovR64Imm32SignExtendedInstructionDisposition::MovR64Imm32SignExtendedMaterializationV1 {
        literal_bits,
        destination,
        baseline_byte_count,
        selected_byte_count,
    } = disposition
    else {
        return encode_scalar(
            architecture,
            selected.id,
            kind,
            machine.alternative.key,
            machine,
            physical,
        );
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    };
    let operand = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1);
    let destination_matches = operand.is_some_and(|operand| {
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
        || *baseline_byte_count != X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT
    {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    let encoded = encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
        physical,
        destination.destination_view,
        value,
    )
    .map_err(OptimizedSelectedFormEncodingError::X86_64MovR64Imm32SignExtended)?;
    let footprint = encoded.footprint();
    validate_operand_footprint(
        selected.id,
        machine,
        &footprint.encoded,
        &footprint.register_reads,
        &footprint.register_writes,
    )?;
    if encoded.bytes().len() != usize::from(*selected_byte_count)
        || encoded.destination() != destination.destination_view
        || encoded.encoded_write_view() != destination.encoded_view
        || encoded.value_bits() != *literal_bits
        || footprint.writes_rflags
        || footprint.encoded_write_view != destination.encoded_view
        || footprint.encoded_write_view_units != destination.encoded_storage_units
        || footprint.encoded_write_units != destination.encoded_write_units
        || footprint.encoded_write_semantics != destination.encoded_write_semantics
        || footprint.encoded != machine.alternative.encoded
    {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(selected.id));
    }
    Ok(SelectedFormEncodingState::Encoded {
        bytes: encoded.bytes().to_vec(),
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: footprint.register_reads.clone(),
            register_writes: footprint.register_writes.clone(),
            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
            encoded: footprint.encoded.clone(),
        }),
    })
}
