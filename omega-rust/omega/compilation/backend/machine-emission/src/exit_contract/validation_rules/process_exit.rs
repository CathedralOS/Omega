//! Terminal process-exit effects are distinct from returning to a caller.
use super::super::WholeFunctionExitContractError as Error;
use machine_code::{
    ResolvedSelectedFormRow, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};
use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstruction, SelectedInstructionKind,
};

pub(in crate::exit_contract) fn validate_process_exit(
    instruction: &SelectedInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    block_end: u64,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    if !matches!(encoding.state, SelectedFormEncodingState::Encoded) {
        return Err(Error::InstructionRosterMismatch(instruction.id));
    }
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(Error::InstructionRosterMismatch(instruction.id));
    };
    if instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes.is_empty()
        || bytes != layout.bytes
        || layout.branch.is_some()
        || encoding.address.is_some()
        || layout.offset.checked_add(bytes.len() as u64) != Some(block_end)
        || footprint.encoded.control != MachineEncodedControlEffect::HostedExitOrTrapV1
        || footprint.encoded.trap != MachineEncodedTrapBehavior::HostedExitReturnedV1
        || footprint.encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || footprint.encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || footprint.encoded.external_operand_reads != [0]
        || !footprint.encoded.external_operand_writes.is_empty()
        || instruction.operands.len() != 1
    {
        return Err(Error::NonReturnControlEffect(instruction.id));
    }
    Ok(())
}
