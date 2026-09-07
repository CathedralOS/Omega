use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstruction, SelectedTerminator};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::{
    super::{OptimizedSelectedFormEncodingError, SelectedFormEncodingRow},
    row,
};

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    rows: &[SelectedFormEncodingRow],
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let selected_plan = selected.selected_plan();
    let machine = staged.machine().plan();
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    let mut candidate_rows = rows.iter();

    for (selected_function, machine_function) in
        selected_plan.functions.iter().zip(&machine.functions)
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        for (selected_block, machine_block) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let selected_instruction = if index < selected_block.instructions.len() {
                    &selected_block.instructions[index]
                } else {
                    terminator_instruction(&selected_block.terminator)
                };
                if selected_instruction.id != machine_instruction.instruction {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                let candidate = candidate_rows
                    .next()
                    .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
                crate::frame_address::validate_address(
                    machine_function,
                    frame,
                    machine_instruction,
                    candidate.address,
                )?;
                row::validate(
                    selected_plan.target,
                    selected_instruction,
                    machine_instruction,
                    physical,
                    candidate,
                )?;
            }
        }
    }
    if candidate_rows.next().is_some() {
        return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
    }
    Ok(())
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}
