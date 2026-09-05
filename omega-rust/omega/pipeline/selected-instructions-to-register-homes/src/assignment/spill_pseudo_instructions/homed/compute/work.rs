//! Producer-local exact five-axis work accounting.

use optimization_core::OptimizationWorkUsage;

use crate::{
    FunctionHomedSpillPseudoInstructions, HomedSpillPseudoInstruction,
    HomedSpillPseudoInstructionError,
};

pub(super) fn usage(
    functions: &[FunctionHomedSpillPseudoInstructions],
) -> Result<OptimizationWorkUsage, HomedSpillPseudoInstructionError> {
    let function_count = count(functions.len())?;
    let storage_count = sum(functions.iter().map(|row| row.storage.len()))?;
    let instruction_count = sum(functions.iter().map(|row| row.instructions.len()))?;
    let reload_count = sum(functions.iter().map(|row| {
        row.instructions
            .iter()
            .filter(|instruction| matches!(instruction, HomedSpillPseudoInstruction::Reload { .. }))
            .count()
    }))?;
    let rewrite_count = sum(functions.iter().map(|row| row.rewrites.len()))?;
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

fn sum(mut values: impl Iterator<Item = usize>) -> Result<u64, HomedSpillPseudoInstructionError> {
    values.try_fold(0_u64, |total, value| {
        total
            .checked_add(count(value)?)
            .ok_or(HomedSpillPseudoInstructionError::WorkOverflow)
    })
}

fn count(value: usize) -> Result<u64, HomedSpillPseudoInstructionError> {
    u64::try_from(value).map_err(|_| HomedSpillPseudoInstructionError::WorkOverflow)
}
