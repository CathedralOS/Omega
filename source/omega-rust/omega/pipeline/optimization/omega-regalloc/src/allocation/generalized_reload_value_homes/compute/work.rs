//! Producer-local exact five-axis work accounting.

use omega_optimization_core::OptimizationWorkUsage;

use crate::{
    FunctionGeneralizedReloadValueHomes, GeneralizedReloadValueHomeError,
    GeneralizedReloadValueHomeOutcome,
};

pub(super) fn usage(
    functions: &[FunctionGeneralizedReloadValueHomes],
) -> Result<OptimizationWorkUsage, GeneralizedReloadValueHomeError> {
    let functions_used = count(functions.len())?;
    let mut outcomes = 0_u64;
    let mut assignments = 0_u64;
    let mut candidates = 0_u64;
    let mut homes = 0_u64;
    for outcome in functions.iter().flat_map(|function| &function.outcomes) {
        outcomes = add(outcomes, 1)?;
        match outcome {
            GeneralizedReloadValueHomeOutcome::Assigned(assignment) => {
                assignments = add(assignments, 1)?;
                candidates = add(candidates, count(assignment.candidates.len())?)?;
                homes = add(homes, count(assignment.coexisting_homes.len())?)?;
            }
            GeneralizedReloadValueHomeOutcome::Pressure(pressure) => {
                candidates = add(candidates, count(pressure.candidates.len())?)?;
                homes = add(homes, count(pressure.blocking_homes.len())?)?;
            }
        }
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: add(functions_used, outcomes)?,
        candidates,
        validation_steps: add(
            add(candidates, homes)?,
            outcomes
                .checked_mul(5)
                .ok_or(GeneralizedReloadValueHomeError::WorkOverflow)?,
        )?,
        commits: assignments,
        iterations: add(functions_used, outcomes)?,
    })
}

fn add(left: u64, right: u64) -> Result<u64, GeneralizedReloadValueHomeError> {
    left.checked_add(right)
        .ok_or(GeneralizedReloadValueHomeError::WorkOverflow)
}

fn count(value: usize) -> Result<u64, GeneralizedReloadValueHomeError> {
    u64::try_from(value).map_err(|_| GeneralizedReloadValueHomeError::WorkOverflow)
}
