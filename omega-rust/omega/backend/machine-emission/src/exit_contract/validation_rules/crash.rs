//! A crash is the final instruction, not a call, cleanup, or returning exit.
//! ISA decoding establishes the trap bytes; this check joins their effects to
//! the selected no-successor instruction and rejects hidden machine actions.

use super::super::WholeFunctionExitContractError as Error;
use machine_code::{
    ResolvedSelectedFormRow, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};
use physical_instructions::PostAllocationMachineInstruction;
use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstruction, SelectedInstructionKind,
};

pub(in crate::exit_contract) fn validate_crash(
    physical: &register_model::ValidatedPhysicalRegisterModel,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    block_end: u64,
) -> Result<(), Error> {
    let SelectedFormEncodingState::Encoded { bytes, footprint } = &encoding.state else {
        return Err(Error::InstructionRosterMismatch(instruction.id));
    };
    let effects = &footprint.encoded;
    let counter_name = match physical.model().architecture {
        target::Architecture::X86_64 => "rip",
        target::Architecture::Aarch64 => "pc",
    };
    let counter = physical
        .model()
        .view_named(counter_name)
        .ok_or(Error::NonReturnControlEffect(instruction.id))?;
    if instruction.kind != SelectedInstructionKind::Crash
        || !instruction.operands.is_empty()
        || instruction.implicit_uses != counter.units
        || instruction.implicit_defs != counter.units
        || !instruction.clobbers.is_empty()
        || !machine.operands.is_empty()
        || machine.address.is_some()
        || machine.unit_uses != counter.units
        || machine.unit_defs != counter.units
        || !machine.unit_clobbers.is_empty()
        || machine.implicit_unit_uses != counter.units
        || machine.implicit_unit_defs != counter.units
        || !machine.implicit_unit_clobbers.is_empty()
        || encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes.is_empty()
        || bytes != &layout.bytes
        || layout.branch.is_some()
        || encoding.address.is_some()
        || layout.offset.checked_add(bytes.len() as u64) != Some(block_end)
        || effects.control != MachineEncodedControlEffect::CrashV1
        || effects.trap != MachineEncodedTrapBehavior::ExplicitCrashV1
        || effects.stack != MachineEncodedStackEffect::UnchangedV1
        || effects.memory != MachineEncodedMemoryEffect::NoneV1
        || !effects.external_operand_reads.is_empty()
        || !effects.external_operand_writes.is_empty()
        || effects.implicit_unit_uses != counter.units
        || effects.implicit_unit_defs != counter.units
        || !effects.implicit_unit_clobbers.is_empty()
    {
        return Err(Error::NonReturnControlEffect(instruction.id));
    }
    Ok(())
}
