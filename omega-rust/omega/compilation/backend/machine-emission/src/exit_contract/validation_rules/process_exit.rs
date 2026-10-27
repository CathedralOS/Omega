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
        // The explicit completion owns an exact invocation, not a source
        // crash. If its checked or opaque provider returns, only this
        // uncharged trap may execute: no result or normal continuation.
        let mut rows = function.normalized_foreign_calls.iter().filter(|row| {
            row.invocation_site
                == semantic_vocabulary::InvocationSite::ExternalCompletion(completion_edge)
        });
        let foreign = rows.next();
        let mut checked = function.calls.iter().filter(|row| {
            row.owner == target_operations::CallSiteOwner::ExternalCompletion(completion_edge)
        });
        let checked_call = checked.next();
        let call = block
            .instructions
            .last()
            .ok_or(Error::NonReturnControlEffect(instruction.id))?;
        if rows.next().is_some()
            || checked.next().is_some()
            || match (foreign, checked_call, &call.kind) {
                (
                    Some(row),
                    None,
                    SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal },
                ) => row.instruction != call.id
                    || row.call.boundary != *boundary
                    || function.normalized_foreign_calls.get(*ordinal as usize) != Some(row)
                    || selected_instructions::calls::normalized_foreign::return_transport::resolve(
                        function, block, row,
                    )
                    .is_none(),
                (None, Some(row), SelectedInstructionKind::CallUnit { target }) => {
                    row.instruction != call.id
                        || *target
                            != selected_instructions::SelectedCallTarget::from_native(
                                row.call.target,
                            )
                        || row.call.result_placement.is_some()
                        || row.call.structural_result.is_some()
                        || !row.call.crash_continuations.is_empty()
                        || !matches!(
                            row.call.source,
                            target_operations::NativeCallOrigin::InstalledProvider { .. }
                                | target_operations::NativeCallOrigin::TargetFamily { .. }
                        )
                        || row.call.validate_source(&[]).is_err()
                        || call.provenance.edges != [completion_edge]
                }
                _ => true,
            }
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
