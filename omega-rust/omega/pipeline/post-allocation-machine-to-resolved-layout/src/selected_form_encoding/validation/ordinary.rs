use post_allocation_machine_to_post_allocation_machine::{
    Aarch64CbnzInstructionDisposition, Aarch64SameViewCopyInstructionDisposition,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstruction, SelectedTerminator};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::selected_form_encoding::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
};

use super::{
    super::{
        OptimizedSelectedFormEncodingError, SelectedFormEncodingRow,
        SelectedFormMachineDisposition, materialization::MaterializationPlan,
    },
    row,
};

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    rows: &[SelectedFormEncodingRow],
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let selected_plan = selected.selected_plan();
    let machine = staged.machine().plan();
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    validate_optimization_function_count(optimization, selected_plan.functions.len())?;

    let fusion = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion) => Some(fusion),
        _ => None,
    });
    let materialization = MaterializationPlan::from_optimization(optimization);
    let copy_elision = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(elision) => {
            Some(elision)
        }
        _ => None,
    });
    let mut candidate_rows = rows.iter();

    for (function_index, (selected_function, machine_function)) in selected_plan
        .functions
        .iter()
        .zip(&machine.functions)
        .enumerate()
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        let fusion_function =
            fusion.map(|fusion| &fusion.fusion().plan().functions[function_index]);
        let materialization_function = materialization
            .map(|materialization| materialization.function(function_index))
            .transpose()?;
        let copy_elision_function =
            copy_elision.map(|elision| &elision.elision().plan().functions[function_index]);
        if fusion_function.is_some_and(|row| {
            row.machine != selected_function.machine
                || row.blocks.len() != selected_function.blocks.len()
        }) || materialization_function.is_some_and(|row| {
            !row.matches(selected_function.machine, selected_function.blocks.len())
        }) || copy_elision_function.is_some_and(|row| {
            row.machine != selected_function.machine
                || row.blocks.len() != selected_function.blocks.len()
        }) {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }

        for (block_index, (selected_block, machine_block)) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
            .enumerate()
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            let fusion_block = fusion_function.map(|function| &function.blocks[block_index]);
            let materialization_block = materialization_function
                .map(|function| function.block(block_index))
                .transpose()?;
            let copy_elision_block =
                copy_elision_function.map(|function| &function.blocks[block_index]);
            if fusion_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) || materialization_block.is_some_and(|row| {
                !row.matches(selected_block.id, machine_block.instructions.len())
            }) || copy_elision_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) {
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
                let fusion_disposition = fusion_block.map(|block| &block.instructions[index]);
                let materialization_disposition = materialization_block
                    .map(|block| block.disposition(index, selected_instruction.id))
                    .transpose()?;
                let copy_elision_disposition =
                    copy_elision_block.map(|block| &block.instructions[index]);
                if fusion_disposition.is_some_and(|row| row.instruction != selected_instruction.id)
                    || copy_elision_disposition
                        .is_some_and(|row| row.instruction != selected_instruction.id)
                {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                let candidate = candidate_rows
                    .next()
                    .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
                let machine_disposition = match (fusion_disposition, copy_elision_disposition) {
                    (Some(row), None) => match &row.disposition {
                        Aarch64CbnzInstructionDisposition::RetainedV1 => {
                            SelectedFormMachineDisposition::RetainedV1
                        }
                        Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer } => {
                            SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 {
                                consumer: *consumer,
                            }
                        }
                        Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                            compare,
                            source_read,
                        } => SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 {
                            compare: *compare,
                            source_read: source_read.clone(),
                        },
                    },
                    (None, Some(row)) => match &row.disposition {
                        Aarch64SameViewCopyInstructionDisposition::RetainedV1 => {
                            SelectedFormMachineDisposition::RetainedV1
                        }
                        Aarch64SameViewCopyInstructionDisposition::ElidedSameViewCopyI64V1 {
                            consumer,
                        } => SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 {
                            consumer: *consumer,
                        },
                    },
                    (None, None) => SelectedFormMachineDisposition::RetainedV1,
                    (Some(_), Some(_)) => {
                        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                    }
                };
                row::validate(
                    selected_plan.target,
                    selected_instruction,
                    machine_instruction,
                    physical,
                    &machine_disposition,
                    materialization_disposition,
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

fn validate_optimization_function_count(
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    expected: usize,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let actual = match optimization {
        Some(StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion)) => {
            fusion.fusion().plan().functions.len()
        }
        Some(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(
            elision,
        )) => elision.elision().plan().functions.len(),
        Some(optimization) => MaterializationPlan::from_optimization(Some(optimization))
            .map(MaterializationPlan::function_count)
            .unwrap_or(expected),
        None => expected,
    };
    if actual != expected {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    Ok(())
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}
