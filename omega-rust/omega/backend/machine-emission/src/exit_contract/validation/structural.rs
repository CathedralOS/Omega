//! Direct admission of the bounded structural Unit exit and call roster.

use super::super::{
    WholeFunctionExitContractError,
    validation_rules::{reject_preservation_writes, validate_structural_call_layout},
};
use super::{Inputs, context::Context, require, returned};
use isa_x86_64::X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT;
use selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use std::collections::BTreeSet;

pub(super) fn check(
    inputs: &Inputs<'_>,
    context: &Context,
) -> Result<(), WholeFunctionExitContractError> {
    let Inputs {
        selected,
        machine,
        encoding,
        layout,
        contract,
        ..
    } = inputs;
    let count = selected.structural_unit_functions.len();
    if !matches!(count, 1 | 2)
        || machine.structural_unit_functions.len() != count
        || encoding.structural_unit_functions().len() != count
        || layout.structural_unit_functions().len() != count
    {
        return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
    }
    require(contract.functions.is_empty() && contract.structural_unit_functions.len() == count)?;
    let mut seen = BTreeSet::new();
    let mut caller = None;
    let mut leaf = None;
    for (claimed, source) in contract
        .structural_unit_functions
        .iter()
        .zip(&selected.structural_unit_functions)
    {
        require(
            claimed.machine == source.machine
                && seen.insert(source.machine)
                && claimed.entry_block == source.entry_block
                && claimed.body_stack_delta == 0
                && claimed.modified_callee_saved_units.is_empty(),
        )?;
        let machine = machine
            .structural_unit_functions
            .iter()
            .find(|row| row.machine == source.machine)
            .ok_or(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(source.machine),
            )?;
        let encoded = encoding
            .structural_unit_functions()
            .iter()
            .find(|row| row.machine == source.machine)
            .ok_or(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(source.machine),
            )?;
        let resolved = layout
            .structural_unit_functions()
            .iter()
            .find(|row| row.machine == source.machine)
            .ok_or(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(source.machine),
            )?;
        let terminal = &source.terminator.instruction;
        if source.entry_block != machine.block
            || source.entry_block != encoded.block
            || source.entry_block != resolved.block
            || resolved.offset != 0
            || terminal.id != SelectedInstructionId(u32::from(source.call.is_some()))
            || terminal.kind != SelectedInstructionKind::ReturnUnit
            || terminal.id != machine.return_instruction.instruction
            || terminal.id != encoded.return_instruction.instruction
            || terminal.id != resolved.return_instruction.instruction
            || terminal.provenance != machine.return_provenance
            || source.terminator.effect != machine.return_effect
            || source.terminator.ownership != machine.return_ownership
            || machine.return_instruction.alternative.key != encoded.return_instruction.alternative
            || machine.return_instruction.alternative.key != resolved.return_instruction.alternative
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(source.machine),
            );
        }
        reject_preservation_writes(
            &machine.return_instruction,
            &context.callee_saved,
            &context.link_units,
            terminal.id,
        )?;
        match (
            &claimed.call,
            &source.call,
            &machine.call,
            &encoded.call,
            &resolved.call,
        ) {
            (None, None, None, None, None) => {
                require(
                    leaf.replace(source.machine).is_none()
                        && resolved.byte_count == 1
                        && resolved.return_instruction.offset == 0,
                )?;
            }
            (
                Some(claim),
                Some(source_call),
                Some(actual),
                Some(encoded_call),
                Some(resolved_call),
            ) => {
                require(
                    caller
                        .replace((source.machine, source_call.callee))
                        .is_none()
                        && source.machine == selected.entry,
                )?;
                if source_call.id != SelectedInstructionId(0)
                    || source_call.id != actual.instruction
                    || source_call.id != encoded_call.instruction
                    || source_call.id != resolved_call.instruction
                    || source_call.operation != actual.operation
                    || source_call.operation != encoded_call.operation
                    || source_call.operation != resolved_call.operation
                    || source_call.callee != actual.callee
                    || source_call.callee != encoded_call.callee
                    || source_call.callee != resolved_call.callee
                    || source_call.constraint != actual.constraint
                    || source_call.implicit_uses != actual.unit_uses
                    || source_call.implicit_defs != actual.unit_defs
                    || source_call.clobbers != actual.unit_clobbers
                    || source_call.layout != actual.layout
                    || source_call.effect != actual.effect
                    || source_call.ownership != actual.ownership
                    || source_call.claim_transfers != actual.claim_transfers
                    || source_call.provenance != actual.provenance
                    || encoded_call.bytes != resolved_call.bytes
                    || encoded_call.footprint != resolved_call.footprint
                    || encoded_call.fixup != resolved_call.fixup
                {
                    return Err(
                        WholeFunctionExitContractError::StructuralCallRosterMismatch(
                            source_call.id,
                        ),
                    );
                }
                validate_structural_call_layout(
                    source_call.id,
                    source_call.callee,
                    actual,
                    resolved_call,
                    &context.callee_saved,
                )?;
                let call_size = u64::try_from(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT)
                    .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?;
                require(
                    resolved.byte_count == call_size + 1
                        && resolved.return_instruction.offset == call_size,
                )?;
                require(
                    claim.block == source.entry_block
                        && claim.instruction == source_call.id
                        && claim.operation == source_call.operation
                        && claim.callee == source_call.callee
                        && claim.offset == resolved_call.offset
                        && claim.bytes == resolved_call.bytes
                        && claim.fixup == resolved_call.fixup
                        && claim.unit_uses == actual.unit_uses
                        && claim.unit_defs == actual.unit_defs
                        && claim.unit_clobbers == actual.unit_clobbers
                        && claim.frame_byte_count == resolved_call.footprint.frame_byte_count
                        && claim.shadow_byte_count == resolved_call.footprint.shadow_byte_count
                        && claim.pre_call_stack_alignment
                            == resolved_call.footprint.pre_call_stack_alignment
                        && claim.frame_is_balanced == resolved_call.footprint.frame_is_balanced,
                )?;
            }
            _ => return Err(WholeFunctionExitContractError::ArtifactMismatch),
        }
        let end = resolved
            .offset
            .checked_add(resolved.byte_count)
            .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
        returned::check(
            context,
            inputs.machine.target.architecture,
            source.entry_block,
            source.terminator.psi_return_edge,
            terminal,
            &machine.return_instruction,
            &encoded.return_instruction,
            &resolved.return_instruction,
            end,
            &claimed.returned,
        )?;
    }
    if count == 1 {
        require(caller.is_none() && leaf == Some(selected.entry))
    } else {
        require(matches!((caller, leaf), (Some((owner, target)), Some(leaf))
            if owner == selected.entry && owner != leaf && target == leaf))
    }
}
