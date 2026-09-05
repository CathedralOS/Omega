//! Exact row encoder for x86-64 XOR-zero materialization.

use omega_isa_x86_64::{encode_x86_64_selected_form, encode_x86_64_xor_zero_i64_materialization};
use omega_machine_optimizer::{
    X86_MOVABS_I64_BYTE_COUNT, X86_XOR_R64_SELF_BYTE_COUNT, X86XorZeroInstructionDisposition,
};
use omega_physical_instructions::PostAllocationMachineInstruction;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstruction, SelectedInstructionKind};
use omega_target::Architecture;

use super::{integer_bits, validate_operand_footprint, validate_size};
use crate::{
    OptimizedSelectedFormEncodingError, SelectedFormDecodedFootprint, SelectedFormEncodingState,
};

pub(super) fn encode(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &X86XorZeroInstructionDisposition,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let baseline = encode_x86_64_selected_form(
        physical,
        kind,
        machine.alternative.key,
        &machine
            .operands
            .iter()
            .map(|operand| operand.view)
            .collect::<Vec<_>>(),
    )
    .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
    validate_operand_footprint(
        selected.id,
        machine,
        &baseline.footprint().encoded,
        &baseline.footprint().register_reads,
        &baseline.footprint().register_writes,
    )?;
    if architecture != Architecture::X86_64
        || baseline.bytes().len() != usize::from(X86_MOVABS_I64_BYTE_COUNT)
        || baseline.footprint().encoded != machine.alternative.encoded
    {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(selected.id));
    }
    validate_size(
        selected.id,
        machine.alternative.size,
        baseline.bytes().len(),
    )?;

    let X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
        destination,
        rflags_units,
        baseline_byte_count,
        selected_byte_count,
    } = disposition
    else {
        return Ok(SelectedFormEncodingState::Encoded {
            bytes: baseline.bytes().to_vec(),
            footprint: Box::new(SelectedFormDecodedFootprint {
                register_reads: baseline.footprint().register_reads.clone(),
                register_writes: baseline.footprint().register_writes.clone(),
                implicit_defs: baseline.footprint().encoded.implicit_unit_defs.clone(),
                implicit_clobbers: baseline.footprint().encoded.implicit_unit_clobbers.clone(),
                encoded: baseline.footprint().encoded.clone(),
            }),
        });
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
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
    if !destination_matches
        || integer_bits(value) != Some(0)
        || *baseline_byte_count != X86_MOVABS_I64_BYTE_COUNT
        || *selected_byte_count != X86_XOR_R64_SELF_BYTE_COUNT
    {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    let encoded = encode_x86_64_xor_zero_i64_materialization(physical, destination.view)
        .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
    let footprint = encoded.footprint();
    validate_operand_footprint(
        selected.id,
        machine,
        &footprint.encoded,
        &footprint.register_reads,
        &footprint.register_writes,
    )?;
    if encoded.bytes().len() != usize::from(X86_XOR_R64_SELF_BYTE_COUNT)
        || encoded.value_bits() != 0
        || encoded.destination() != destination.view
        || !footprint.writes_rflags
        || !footprint.register_reads.is_empty()
        || footprint.register_writes.as_slice() != [destination.view]
        || !footprint.encoded.implicit_unit_uses.is_empty()
        || !footprint.encoded.implicit_unit_defs.is_empty()
        || footprint.encoded.implicit_unit_clobbers != *rflags_units
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
