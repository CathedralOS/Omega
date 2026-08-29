//! Independent run replay entrance.
//!
//! The order here is the custody contract: rebuild the selected schedule,
//! replay every commit, bind Applied decisions to that replay, validate the
//! ledger/usage records, then validate the external policy mirror.

mod applied_decisions;
mod commits;
mod records;
mod rule_set;

use omega_optimization_core::{OptimizationRuleSetIdentity, OptimizationWorkUsage};
use omega_psi_optimizer::{
    OptimizationRun, OptimizationRunUsage, validate_external_decision_recording,
};

use crate::OptimizedAbstractProjectionError;

pub(super) struct ValidatedRunReplay {
    pub(super) ordered_rule_set: OptimizationRuleSetIdentity,
}

pub(super) fn validate(
    run: &OptimizationRun,
) -> Result<ValidatedRunReplay, OptimizedAbstractProjectionError> {
    let schedule = rule_set::rebuild(run.selections())?;
    let applied = commits::replay(run, &schedule.registries)?;
    applied_decisions::validate(run, &schedule.registries, &applied)?;
    records::validate(run, schedule.ordered_rule_set)?;
    validate_external_decision_recording(run)
        .map_err(|_| OptimizedAbstractProjectionError::ExternalDecisionRecordingMismatch)?;
    Ok(ValidatedRunReplay {
        ordered_rule_set: schedule.ordered_rule_set,
    })
}

pub(super) const fn work_usage(usage: OptimizationRunUsage) -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: usage.rule_evaluations,
        candidates: usage.candidates,
        validation_steps: usage.validation_steps,
        commits: usage.commits,
        iterations: usage.iterations,
    }
}
