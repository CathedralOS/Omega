//! Replay-local work reconstruction.

use omega_optimization_core::OptimizationWorkUsage;

use crate::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError, FunctionAbstractSpillMemoryEffects,
    ValidatedHomedSpillPseudoInstructions,
};

pub(super) fn reconstruct(
    source: &ValidatedHomedSpillPseudoInstructions,
    functions: &[FunctionAbstractSpillMemoryEffects],
) -> Result<OptimizationWorkUsage, AbstractSpillMemoryEffectError> {
    let function_count = cast(functions.len())?;
    let mut storage_count = 0_u64;
    for function in &source.plan().functions {
        storage_count = add(storage_count, function.storage.len())?;
    }
    let mut effect_count = 0_u64;
    let mut read_count = 0_u64;
    let mut write_count = 0_u64;
    for function in functions {
        effect_count = add(effect_count, function.effects.len())?;
        for effect in &function.effects {
            match effect {
                AbstractSpillMemoryEffect::Read { .. } => read_count = increment(read_count)?,
                AbstractSpillMemoryEffect::Write { .. } => write_count = increment(write_count)?,
            }
        }
    }
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

fn add(total: u64, value: usize) -> Result<u64, AbstractSpillMemoryEffectError> {
    total
        .checked_add(cast(value)?)
        .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)
}

fn increment(total: u64) -> Result<u64, AbstractSpillMemoryEffectError> {
    total
        .checked_add(1)
        .ok_or(AbstractSpillMemoryEffectError::WorkOverflow)
}

fn cast(value: usize) -> Result<u64, AbstractSpillMemoryEffectError> {
    u64::try_from(value).map_err(|_| AbstractSpillMemoryEffectError::WorkOverflow)
}
