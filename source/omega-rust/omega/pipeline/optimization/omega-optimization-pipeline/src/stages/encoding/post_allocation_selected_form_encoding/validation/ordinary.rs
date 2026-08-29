use omega_machine_optimizer::Aarch64CbnzInstructionDisposition;
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstruction, SelectedTerminator};

use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
};

use super::{
    super::{OptimizedSelectedFormEncodingError, SelectedFormEncodingRow},
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
    let movn = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) => {
            Some(materialization)
        }
        _ => None,
    });
    let xor_zero = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::X86XorZero(materialization) => {
            Some(materialization)
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
        let movn_function = movn.map(|materialization| {
            &materialization.materialization().plan().functions[function_index]
        });
        let xor_zero_function = xor_zero.map(|materialization| {
            &materialization.materialization().plan().functions[function_index]
        });
        if fusion_function.is_some_and(|row| {
            row.machine != selected_function.machine
                || row.blocks.len() != selected_function.blocks.len()
        }) || movn_function.is_some_and(|row| {
            row.machine != selected_function.machine
                || row.blocks.len() != selected_function.blocks.len()
        }) || xor_zero_function.is_some_and(|row| {
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
            let movn_block = movn_function.map(|function| &function.blocks[block_index]);
            let xor_zero_block = xor_zero_function.map(|function| &function.blocks[block_index]);
            if fusion_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) || movn_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) || xor_zero_block.is_some_and(|row| {
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
                let movn_disposition = movn_block.map(|block| &block.instructions[index]);
                let xor_zero_disposition = xor_zero_block.map(|block| &block.instructions[index]);
                if fusion_disposition.is_some_and(|row| row.instruction != selected_instruction.id)
                    || movn_disposition
                        .is_some_and(|row| row.instruction != selected_instruction.id)
                    || xor_zero_disposition
                        .is_some_and(|row| row.instruction != selected_instruction.id)
                {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                let candidate = candidate_rows
                    .next()
                    .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
                row::validate(
                    selected_plan.target.architecture,
                    selected_instruction,
                    machine_instruction,
                    physical,
                    fusion_disposition
                        .map(|row| &row.disposition)
                        .unwrap_or(&Aarch64CbnzInstructionDisposition::RetainedV1),
                    movn_disposition.map(|row| &row.disposition),
                    xor_zero_disposition.map(|row| &row.disposition),
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
        Some(StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization)) => {
            materialization.materialization().plan().functions.len()
        }
        Some(StagedOptimizedPostAllocationMachineOptimization::X86XorZero(materialization)) => {
            materialization.materialization().plan().functions.len()
        }
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
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}
