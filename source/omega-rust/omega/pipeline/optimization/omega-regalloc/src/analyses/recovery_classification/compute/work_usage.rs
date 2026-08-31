//! Deterministic work accounting for recovery-classification proposals.

use omega_optimization_core::OptimizationWorkUsage;

use crate::{
    RecoveryClassificationError, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillChoices,
};

pub(super) fn required(
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    spill_choices: &ValidatedSpillChoices,
) -> Result<OptimizationWorkUsage, RecoveryClassificationError> {
    let mut usage = OptimizationWorkUsage {
        rule_evaluations: 0,
        candidates: 0,
        validation_steps: 0,
        commits: 0,
        iterations: 1,
    };
    for ((selected, ranges), choices) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .zip(&spill_choices.plan().functions)
    {
        checked_add(&mut usage.rule_evaluations, 1)?;
        checked_add(
            &mut usage.validation_steps,
            selected.virtual_registers.len() as u64,
        )?;
        let instruction_count = selected
            .blocks
            .iter()
            .map(|block| block.instructions.len() as u64 + 1)
            .sum::<u64>();
        checked_add(&mut usage.validation_steps, instruction_count)?;
        checked_add(
            &mut usage.validation_steps,
            ranges.virtual_registers.len() as u64,
        )?;
        if choices.choice.is_some() {
            checked_add(&mut usage.candidates, 1)?;
            checked_add(&mut usage.commits, 1)?;
        }
    }
    Ok(usage)
}

fn checked_add(target: &mut u64, amount: u64) -> Result<(), RecoveryClassificationError> {
    *target = target
        .checked_add(amount)
        .ok_or(RecoveryClassificationError::WorkOverflow)?;
    Ok(())
}
