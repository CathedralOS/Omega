//! Producer-local work accounting.

use omega_optimization_core::OptimizationWorkUsage;

use crate::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError, FunctionAbstractSpillMemoryEffects,
    ValidatedHomedSpillPseudoInstructions,
};

pub(super) fn usage(
    source: &ValidatedHomedSpillPseudoInstructions,
    functions: &[FunctionAbstractSpillMemoryEffects],
) -> Result<OptimizationWorkUsage, AbstractSpillMemoryEffectError> {
    let function_count = count(functions.len())?;
    let storage_count = sum(source.plan().functions.iter().map(|row| row.storage.len()))?;
    let effect_count = sum(functions.iter().map(|row| row.effects.len()))?;
    let read_count = sum(functions.iter().map(|row| {
        row.effects
            .iter()
            .filter(|effect| matches!(effect, AbstractSpillMemoryEffect::Read { .. }))
            .count()
    }))?;
    let write_count = effect_count
        .checked_sub(read_count)
        .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count
            .checked_add(effect_count)
            .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)?,
        candidates: storage_count
            .checked_add(effect_count)
            .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)?,
        validation_steps: storage_count
            .checked_add(effect_count)
            .and_then(|value| value.checked_add(read_count))
            .and_then(|value| value.checked_add(write_count))
            .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)?,
        commits: effect_count,
        iterations: function_count
            .checked_add(storage_count)
            .and_then(|value| value.checked_add(effect_count))
            .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)?,
    })
}

fn sum(mut values: impl Iterator<Item = usize>) -> Result<u64, AbstractSpillMemoryEffectError> {
    values.try_fold(0_u64, |total, value| {
        total
            .checked_add(count(value)?)
            .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)
    })
}

fn count(value: usize) -> Result<u64, AbstractSpillMemoryEffectError> {
    u64::try_from(value).map_err(|_| AbstractSpillMemoryEffectError::WorkOverflow)
}
