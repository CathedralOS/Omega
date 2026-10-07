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
    physical: &register_model::ValidatedPhysicalRegisterModel,
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    completion_edge: semantic_vocabulary::EdgeId,
    instruction: &SelectedInstruction,
    machine: &physical_instructions::PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    block_end: u64,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), Error> {
    if instruction.kind == SelectedInstructionKind::Crash {
        // The explicit completion owns an admitted foreign invocation, not a
        // source crash. If that provider returns, only this uncharged terminal
        // trap may execute; no result, normal return or restoration is admitted.
        let mut rows = function.normalized_foreign_calls.iter().filter(|row| {
            row.invocation_site
                == semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(completion_edge)
        });
        let row = rows
            .next()
            .ok_or(Error::NonReturnControlEffect(instruction.id))?;
        let call = block
            .instructions
            .last()
            .ok_or(Error::NonReturnControlEffect(instruction.id))?;
        let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = call.kind else {
            return Err(Error::NonReturnControlEffect(instruction.id));
        };
        if rows.next().is_some()
            || row.instruction != call.id
            || row.call.boundary != boundary
            || function.normalized_foreign_calls.get(ordinal as usize) != Some(row)
            || selected_instructions::calls::normalized_foreign::return_transport::resolve(
                function, block, row,
            )
            .is_none()
        {
            return Err(Error::NonReturnControlEffect(instruction.id));
        }
        return super::trap::validate_trap(
            physical,
            instruction,
            machine,
            encoding,
            layout,
            block_end,
            encoded,
        );
    }
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
