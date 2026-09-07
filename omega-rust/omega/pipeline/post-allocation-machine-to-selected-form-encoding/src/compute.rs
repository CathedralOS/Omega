use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstruction, SelectedTerminator};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::{
    OptimizedSelectedFormEncodingError, SelectedFormEncoding, SelectedFormEncodingCounts,
    SelectedFormEncodingIdentity, SelectedFormEncodingRow, SelectedFormEncodingState,
    row_encoding::encode_row,
};

pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
) -> Result<SelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let machine = staged.machine().plan();
    crate::frame_address::validate_frame_root(machine, frame)?;
    if machine.selected != selected.selected_identity() {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let selected_plan = selected.selected_plan();
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    let mut rows = Vec::new();
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
                rows.push(encode_row(
                    selected_plan.target,
                    selected_instruction,
                    machine_instruction,
                    physical,
                    crate::frame_address::resolve(machine_function, frame, machine_instruction)?,
                )?);
            }
        }
    }
    let counts = encoding_counts(&rows)?;
    let selected_root = selected.selected_identity();
    let machine_root = staged.machine().receipt().identity();
    let mut program = SelectedFormEncoding {
        selected: selected_root,
        machine: machine_root,
        post_allocation_machine_optimization: None,
        identity: SelectedFormEncodingIdentity::from_bytes([0; 32]),
        rows,
        frame: frame.cloned(),
        counts,
    };
    program.identity = program.recomputed_identity();
    Ok(program)
}

fn encoding_counts(
    rows: &[SelectedFormEncodingRow],
) -> Result<SelectedFormEncodingCounts, OptimizedSelectedFormEncodingError> {
    let mut counts = SelectedFormEncodingCounts::default();
    for row in rows {
        let count = match row.state {
            SelectedFormEncodingState::Encoded { .. } => &mut counts.ordinary_encoded,
            SelectedFormEncodingState::DeferredControl { .. } => {
                &mut counts.ordinary_deferred_control
            }
            SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                counts.ordinary_deferred_internal_control = counts
                    .ordinary_deferred_internal_control
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                &mut counts.ordinary_internal_fixups
            }
        };
        *count = count
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
    }
    Ok(counts)
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
