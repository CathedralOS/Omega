//! Ordinary exit roster and effect checks, driven by the claimed contract.

use super::super::{
    WholeFunctionExitContractError, WholeFunctionExitLayoutCustody,
    validation_rules::{
        frame_permissions, transformed_implicit_writes_any, unique_encoding_rows,
        unique_layout_rows, validate_internal_call, validate_non_return,
        validate_preservation_writes,
    },
};
use super::{Inputs, context::Context, require, returned};
use selected_instructions::{
    MachineAlternativeFamily, SelectedInstructionKind, SelectedTerminator,
};
use std::collections::BTreeSet;

pub(super) fn check(
    inputs: &Inputs<'_>,
    context: &Context,
) -> Result<(), WholeFunctionExitContractError> {
    let Inputs {
        selected,
        machine,
        physical,
        encoding,
        layout,
        frame,
        contract,
    } = inputs;
    require(
        contract.structural_unit_functions.is_empty()
            && contract.functions.len() == selected.functions.len(),
    )?;
    if !machine.structural_unit_functions.is_empty()
        || !encoding.structural_unit_functions().is_empty()
        || !layout.structural_unit_functions().is_empty()
        || machine.functions.len() != selected.functions.len()
        || layout.functions().len() != selected.functions.len()
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    if let Some((frame, protocol)) = frame
        && (frame.plan().functions.len() != selected.functions.len()
            || protocol.plan().functions.len() != selected.functions.len())
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let encoding_rows = unique_encoding_rows(selected, encoding)?;
    let layout_rows = unique_layout_rows(layout)?;
    let mut seen = BTreeSet::new();
    for (claimed, function) in contract.functions.iter().zip(&selected.functions) {
        require(
            claimed.machine == function.machine
                && seen.insert(claimed.machine)
                && claimed.entry_block == function.entry_block
                && claimed.body_stack_delta == 0,
        )?;
        let machine_function = machine
            .functions
            .iter()
            .find(|row| row.machine == claimed.machine)
            .ok_or(WholeFunctionExitContractError::FunctionRosterMismatch(
                claimed.machine,
            ))?;
        let layout_function = layout
            .functions()
            .iter()
            .find(|row| row.machine == claimed.machine)
            .ok_or(WholeFunctionExitContractError::FunctionRosterMismatch(
                claimed.machine,
            ))?;
        if machine_function.blocks.len() != function.blocks.len()
            || layout_function.blocks.len() != function.blocks.len()
        {
            return Err(WholeFunctionExitContractError::FunctionRosterMismatch(
                claimed.machine,
            ));
        }
        let function_frame = frame
            .map(|(frame, _)| {
                frame
                    .plan()
                    .functions
                    .iter()
                    .find(|row| row.machine == claimed.machine)
                    .ok_or(WholeFunctionExitContractError::FunctionRosterMismatch(
                        claimed.machine,
                    ))
            })
            .transpose()?;
        if let Some((_, protocol)) = frame
            && !protocol
                .plan()
                .functions
                .iter()
                .any(|row| row.machine == claimed.machine)
        {
            return Err(WholeFunctionExitContractError::FunctionRosterMismatch(
                claimed.machine,
            ));
        }
        let (allowed, link_write) = frame_permissions(physical, function_frame)?;
        let mut modified = BTreeSet::new();
        let mut returns = claimed.returns.iter();
        for block in &function.blocks {
            let machine_block = machine_function
                .blocks
                .iter()
                .find(|row| row.block == block.id)
                .ok_or(WholeFunctionExitContractError::BlockRosterMismatch(
                    block.id,
                ))?;
            if !layout_function
                .blocks
                .iter()
                .any(|row| row.block == block.id)
                || machine_block.instructions.len() != block.instructions.len() + 1
            {
                return Err(WholeFunctionExitContractError::BlockRosterMismatch(
                    block.id,
                ));
            }
            let (terminal, edge) = match &block.terminator {
                SelectedTerminator::Return {
                    instruction,
                    psi_return_edge,
                } => (instruction, Some(*psi_return_edge)),
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                | SelectedTerminator::Jump { instruction, .. } => (instruction, None),
            };
            for (index, (instruction, actual)) in block
                .instructions
                .iter()
                .chain(std::iter::once(terminal))
                .zip(&machine_block.instructions)
                .enumerate()
            {
                if instruction.id != actual.instruction {
                    return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                        instruction.id,
                    ));
                }
                let key = (claimed.machine, instruction.id);
                let encoded = encoding_rows.get(&key).ok_or(
                    WholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                let (resolved_block, resolved) = layout_rows.get(&key).ok_or(
                    WholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                let relaxed_comparison = matches!(
                    contract.layout_custody,
                    WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. }
                ) && matches!(
                    (instruction.kind, actual.alternative.key.family),
                    (
                        SelectedInstructionKind::ConditionalBranchU64LessThan,
                        MachineAlternativeFamily::ConditionalBranchU64LessThan
                    ) | (
                        SelectedInstructionKind::ConditionalBranchI64LessThan,
                        MachineAlternativeFamily::ConditionalBranchI64LessThan
                    )
                ) && actual.alternative.key.variant == 0
                    && resolved.alternative.family == actual.alternative.key.family
                    && resolved.alternative.variant == 1;
                if resolved_block.block != block.id
                    || encoded.alternative != actual.alternative.key
                    || (resolved.alternative != actual.alternative.key && !relaxed_comparison)
                {
                    return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                        instruction.id,
                    ));
                }
                validate_preservation_writes(
                    actual,
                    encoded,
                    &context.callee_saved,
                    &context.link_units,
                    &allowed,
                    link_write,
                    instruction.id,
                    &mut modified,
                )?;
                let terminal = index == block.instructions.len();
                if let Some(edge) = edge.filter(|_| terminal) {
                    let claimed_return = returns
                        .next()
                        .ok_or(WholeFunctionExitContractError::ArtifactMismatch)?;
                    let end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
                    returned::check(
                        context,
                        machine.target.architecture,
                        block.id,
                        edge,
                        instruction,
                        actual,
                        encoded,
                        resolved,
                        end,
                        claimed_return,
                    )?;
                } else if matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. }) {
                    if function_frame.is_none() {
                        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
                            instruction.id,
                        ));
                    }
                    validate_internal_call(
                        machine.target,
                        context.stack_pointer,
                        instruction.id,
                        encoded,
                        resolved,
                    )?;
                } else {
                    if actual
                        .unit_defs
                        .iter()
                        .chain(&actual.unit_clobbers)
                        .any(|unit| context.stack_units.contains(unit))
                        || transformed_implicit_writes_any(encoded, &context.stack_units)
                    {
                        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
                            instruction.id,
                        ));
                    }
                    validate_non_return(instruction.id, instruction.kind, encoded, resolved)?;
                }
            }
        }
        require(returns.next().is_none() && !claimed.returns.is_empty())?;
        if function_frame.is_some() && modified != allowed {
            return Err(WholeFunctionExitContractError::FramePreservationMismatch(
                claimed.machine,
            ));
        }
        require(
            claimed
                .modified_callee_saved_units
                .iter()
                .copied()
                .eq(modified),
        )?;
    }
    Ok(())
}
