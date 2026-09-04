//! Replay-local exact five-axis usage reconstruction.

use omega_optimization_core::OptimizationWorkUsage;

use crate::{
    FunctionGeneralizedReloadValueHomes, GeneralizedReloadValueHomeError,
    GeneralizedReloadValueHomeOutcome,
};

pub(super) fn reconstruct(
    functions: &[FunctionGeneralizedReloadValueHomes],
) -> Result<OptimizationWorkUsage, GeneralizedReloadValueHomeError> {
    let function_count = to_u64(functions.len())?;
    let mut outcome_count = 0_u64;
    let mut assignment_count = 0_u64;
    let mut candidate_count = 0_u64;
    let mut roster_count = 0_u64;
    for outcome in functions.iter().flat_map(|function| &function.outcomes) {
        outcome_count = checked(outcome_count, 1)?;
        match outcome {
            GeneralizedReloadValueHomeOutcome::Assigned(assignment) => {
                assignment_count = checked(assignment_count, 1)?;
                candidate_count = checked(candidate_count, to_u64(assignment.candidates.len())?)?;
                roster_count = checked(roster_count, to_u64(assignment.coexisting_homes.len())?)?;
            }
            GeneralizedReloadValueHomeOutcome::Pressure(pressure) => {
                candidate_count = checked(candidate_count, to_u64(pressure.candidates.len())?)?;
                roster_count = checked(roster_count, to_u64(pressure.blocking_homes.len())?)?;
            }
        }
    }
    let comparison_steps = outcome_count
        .checked_mul(5)
        .ok_or(GeneralizedReloadValueHomeError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: checked(function_count, outcome_count)?,
        candidates: candidate_count,
        validation_steps: checked(checked(candidate_count, roster_count)?, comparison_steps)?,
        commits: assignment_count,
        iterations: checked(function_count, outcome_count)?,
    })
}

fn checked(left: u64, right: u64) -> Result<u64, GeneralizedReloadValueHomeError> {
    left.checked_add(right)
        .ok_or(GeneralizedReloadValueHomeError::WorkOverflow)
}

fn to_u64(value: usize) -> Result<u64, GeneralizedReloadValueHomeError> {
    u64::try_from(value).map_err(|_| GeneralizedReloadValueHomeError::WorkOverflow)
}
