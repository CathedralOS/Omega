//! The stage's rows: the correspondence both directions walk, and the
//! accounting a pool of produced rows has.
//!
//! The producer (`selected_form_encoding::compute`) and the retained-artifact
//! replay (`validation::ordinary`) walk one correspondence between the
//! selected plan and the post-allocation machine plan: one machine instruction
//! per selected instruction, with the block terminator standing in for the
//! machine instruction that carries the block out. `for_each_row` states that
//! correspondence and its roster diagnostics once; the two directions differ
//! only in what they do with each paired row, encode it or admit it.
//!
//! `counts_of` states the row-state to counter classification once. The replay
//! recomputes an artifact's counts through it and compares them with the
//! stored ones, exactly as both directions recompute the encoding identity
//! through `recomputed_identity`, so no row state can be accounted differently
//! on the two sides.

use physical_instructions::{
    PostAllocationMachineFunction, PostAllocationMachineInstruction, PostAllocationMachinePlan,
};
use selected_instructions::{SelectedFunction, SelectedInstruction, terminator_instruction};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::OptimizedSelectedFormEncodingError as Error;
use machine_code::{
    SelectedFormEncodingCounts, SelectedFormEncodingRow, SelectedFormEncodingState,
};

/// Walk the selected to post-allocation machine row correspondence in
/// lockstep, rejecting a roster whose functions, blocks or instructions do not
/// pair up, and hand each paired row to `visit`.
pub(super) fn for_each_row<S, V>(
    selected: &S,
    machine: &PostAllocationMachinePlan,
    mut visit: V,
) -> Result<(), Error>
where
    S: ValidatedSelectedAnalysis,
    V: FnMut(
        &SelectedFunction,
        &SelectedInstruction,
        &PostAllocationMachineFunction,
        &PostAllocationMachineInstruction,
    ) -> Result<(), Error>,
{
    let selected_plan = selected.selected_plan();
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(Error::FunctionRosterMismatch);
    }
    for (selected_function, machine_function) in
        selected_plan.functions.iter().zip(&machine.functions)
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(Error::FunctionRosterMismatch);
        }
        for (selected_block, machine_block) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(Error::BlockRosterMismatch);
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let selected_instruction = if index < selected_block.instructions.len() {
                    &selected_block.instructions[index]
                } else {
                    terminator_instruction(&selected_block.terminator)
                };
                if selected_instruction.id != machine_instruction.instruction {
                    return Err(Error::InstructionRosterMismatch);
                }
                visit(
                    selected_function,
                    selected_instruction,
                    machine_function,
                    machine_instruction,
                )?;
            }
        }
    }
    Ok(())
}

/// Classify every produced row into the encoding counts.
pub(super) fn counts_of(
    rows: &[SelectedFormEncodingRow],
) -> Result<SelectedFormEncodingCounts, Error> {
    let mut counts = SelectedFormEncodingCounts::default();
    for row in rows {
        let count = match row.state {
            SelectedFormEncodingState::Encoded => &mut counts.ordinary_encoded,
            SelectedFormEncodingState::DeferredControl { .. } => {
                &mut counts.ordinary_deferred_control
            }
            SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(Error::CountOverflow)?;
                counts.ordinary_deferred_internal_control = counts
                    .ordinary_deferred_internal_control
                    .checked_add(1)
                    .ok_or(Error::CountOverflow)?;
                &mut counts.ordinary_internal_fixups
            }
            SelectedFormEncodingState::UnresolvedNormalizedForeignCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(Error::CountOverflow)?;
                &mut counts.ordinary_normalized_foreign_import_fixups
            }
            SelectedFormEncodingState::UnresolvedImageAddress { .. } => {
                &mut counts.ordinary_image_address_fixups
            }
        };
        *count = count.checked_add(1).ok_or(Error::CountOverflow)?;
    }
    Ok(counts)
}
