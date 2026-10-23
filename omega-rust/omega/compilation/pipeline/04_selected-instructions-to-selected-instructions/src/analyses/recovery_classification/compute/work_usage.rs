//! Deterministic work accounting for recovery-classification proposals.
//!
//! The proposal pass projects the exact work its inputs demand before it
//! classifies anything; the shared accumulator owns the checked totals and the
//! one budget admission, so the seal of this pass's own output does not repeat
//! it.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::work_accounting::Work;
use crate::{
    RecoveryClassificationError, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillChoices,
};

/// The exact work the classification pass will charge, admitted against its
/// budget before the pass starts.
pub(super) fn required(
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    spill_choices: &ValidatedSpillChoices,
    budget: OptimizationWorkBudget,
) -> Result<OptimizationWorkUsage, RecoveryClassificationError> {
    let mut work = Work::new(OptimizationWorkUsage {
        iterations: 1,
        ..Default::default()
    });
    for ((selected, ranges), choices) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .zip(&spill_choices.plan().functions)
    {
        work.rule_evaluations(1)?;
        work.validation_steps(selected.virtual_registers.len() as u64)?;
        let instruction_count = selected
            .blocks
            .iter()
            .map(|block| block.instructions.len() as u64 + 1)
            .sum::<u64>();
        work.validation_steps(instruction_count)?;
        work.validation_steps(ranges.virtual_registers.len() as u64)?;
        if choices.choice.is_some() {
            work.candidates(1)?;
            work.commits(1)?;
        }
    }
    Ok(work.finish(budget)?)
}
