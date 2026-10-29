//! Discover canonical scalar demand, then execute dependency-ready jobs.
//!
//! Every selected intermediate observation is known before the first entry
//! executes. Conditions create demand, not execution prerequisites: an entry's
//! own result obligation must not become a self-cycle. Installation retains
//! provisional observations; the caller still owes complete condition discharge
//! and target admission before ordinary execution.

use semantic_vocabulary::ScalarApplicationId;
use terminal_fuel::TerminalFuelMeter;
use terminal_installation::{ScalarApplicationObservations, ScalarConditionSelection};
use terminal_interpreter::{
    TerminalEntryResultRequest, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstantiation, TerminalProgram, TerminalScalarApplicationRequest,
};

mod demand;
#[cfg(test)]
mod tests;

pub fn evaluate_scalar_applications<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: &str,
    requested: &[ScalarApplicationId],
    meter: &mut TerminalFuelMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    let program = TerminalProgram::borrow(observations.artifact());
    let mut conditions = observations.conditions(value_scope, requested)?;
    let mut jobs = demand::discover(&program, observations, requested, &mut conditions)?;
    let order = demand::execution_order(&jobs)?;
    demand::retain_entry_results(observations, &conditions, &mut jobs)?;

    let account = meter.invocation_account().cloned();
    for position in order {
        let job = &jobs[position];
        if observations.value(job.application)?.is_some() {
            continue;
        }
        match &job.execution {
            demand::Execution::Layout => {
                observations
                    .observe_layout(job.application)
                    .map_err(|error| error.to_string())?;
            }
            demand::Execution::Entry {
                machine,
                installation,
            } => {
                let mut instance = program
                    .instantiate(TerminalInstantiation {
                        account: account.as_ref(),
                        installation: Some(installation),
                        entry_results: Some(TerminalEntryResultRequest {
                            machine: *machine,
                            results: &job.results,
                        }),
                        scalar_application: Some(TerminalScalarApplicationRequest {
                            application: job.application,
                            observations,
                        }),
                        ..TerminalInstantiation::default()
                    })
                    .map_err(|error| format!("scalar evaluation entry: {error:?}"))?;
                let result = instance
                    .invoke(meter)
                    .map_err(|error| format!("scalar evaluation execution: {error:?}"))?;
                let TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) =
                    result
                else {
                    return Err(format!(
                        "scalar evaluation did not return a normal scalar: {result:?}"
                    ));
                };
                let mut captured = Vec::with_capacity(job.results.len());
                for (result, _) in &job.results {
                    let value = instance
                        .observed_entry_result(*machine, *result)
                        .map_err(|error| format!("scalar evaluation observation: {error:?}"))?;
                    captured.push((*result, value));
                }
                drop(instance);
                for (result, captured) in captured {
                    observations.record_entry_result(
                        job.application,
                        *machine,
                        result,
                        captured,
                    )?;
                }
                observations.record_value(job.application, value)?;
            }
        }
    }
    Ok(conditions)
}
