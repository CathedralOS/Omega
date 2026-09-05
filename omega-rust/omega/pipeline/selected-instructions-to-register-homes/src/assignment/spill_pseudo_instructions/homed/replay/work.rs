//! Replay-local exact five-axis work reconstruction.

use optimization_core::OptimizationWorkUsage;

use crate::{
    FunctionHomedSpillPseudoInstructions, HomedSpillPseudoInstruction,
    HomedSpillPseudoInstructionError,
};

pub(super) fn reconstruct(
    functions: &[FunctionHomedSpillPseudoInstructions],
) -> Result<OptimizationWorkUsage, HomedSpillPseudoInstructionError> {
    let function_count = cast(functions.len())?;
    let mut storage_count = 0_u64;
    let mut instruction_count = 0_u64;
    let mut reload_count = 0_u64;
    let mut rewrite_count = 0_u64;
    for function in functions {
        storage_count = add(storage_count, function.storage.len())?;
        instruction_count = add(instruction_count, function.instructions.len())?;
        reload_count = add(
            reload_count,
            function
                .instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction, HomedSpillPseudoInstruction::Reload { .. })
                })
                .count(),
        )?;
        rewrite_count = add(rewrite_count, function.rewrites.len())?;
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count
            .checked_add(reload_count)
            .ok_or(HomedSpillPseudoInstructionError::WorkOverflow)?,
        candidates: instruction_count,
        validation_steps: storage_count
            .checked_add(instruction_count)
            .and_then(|value| value.checked_add(reload_count))
            .and_then(|value| value.checked_add(rewrite_count))
            .ok_or(HomedSpillPseudoInstructionError::WorkOverflow)?,
        commits: instruction_count
            .checked_add(rewrite_count)
            .ok_or(HomedSpillPseudoInstructionError::WorkOverflow)?,
        iterations: function_count
            .checked_add(storage_count)
            .and_then(|value| value.checked_add(instruction_count))
            .ok_or(HomedSpillPseudoInstructionError::WorkOverflow)?,
    })
}

fn add(total: u64, value: usize) -> Result<u64, HomedSpillPseudoInstructionError> {
    total
        .checked_add(cast(value)?)
        .ok_or(HomedSpillPseudoInstructionError::WorkOverflow)
}

fn cast(value: usize) -> Result<u64, HomedSpillPseudoInstructionError> {
    u64::try_from(value).map_err(|_| HomedSpillPseudoInstructionError::WorkOverflow)
}
