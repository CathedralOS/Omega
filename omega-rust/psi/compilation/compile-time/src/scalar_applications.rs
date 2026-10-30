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

/// The embedding selects providers and target meaning; Psi evaluates the
/// canonical jobs and discharges every applicable condition. Address-model
/// refusal remains deferred until predicates have been checked.
pub struct ScalarScopeEvaluation {
    pub application_roots: Option<Vec<terminal_psi::TargetMachineCallable>>,
    pub targets: terminal_installation::AdmittedTargetMachineSelections,
    pub providers: terminal_installation::AdmittedProviderSelections,
    pub address_model: Result<Option<terminal_installation::ScalarAddressModel>, String>,
    pub carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    /// Immutable target-catalogue data; never inferred from the executing host.
    pub layout_target: terminal_installation::LayoutTargetFacts,
}

/// Complete all authored proof scopes without changing the shared artifact.
/// Explicit consumer demand belongs only to the primary value scope; other
/// scopes execute the dependencies of their own retained obligations.
pub fn complete_scalar_applications(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: &str,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    accounting: &mut crate::SourceCompletionAccounting,
    mut select: impl FnMut(&str) -> Result<ScalarScopeEvaluation, String>,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let mut records = Vec::new();
    let result = complete_scopes(
        artifact,
        primary,
        deployment_scopes,
        requested,
        sponsor,
        &mut records,
        &mut select,
    );
    // Partial executions survive failure; a refused scope is not free work.
    accounting.append_scalar_evaluations(records);
    result
}

fn complete_scopes(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: &str,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    records: &mut Vec<crate::evaluation_accounting::CanonicalScalarEvaluationReceipt>,
    select: &mut impl FnMut(&str) -> Result<ScalarScopeEvaluation, String>,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let mut closure =
        terminal_installation::ScalarApplicationClosure::new(artifact.view(), primary);
    if let Some(scopes) = deployment_scopes {
        closure = closure.with_deployment_scopes(scopes);
    }
    let scopes = closure.scopes().map(str::to_owned).collect::<Vec<_>>();
    for scope in scopes {
        let selected = select(&scope)?;
        if selected.targets.target() != scope {
            return Err("scalar evaluation selected a different authored scope".into());
        }
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &selected.targets,
            &selected.providers,
            &selected.layout_target,
        )?
        .with_target_carriers(selected.carrier_layout)
        .with_application_roots(selected.application_roots.as_deref())?;
        let demand = if scope == primary { requested } else { &[] };
        let input = crate::evaluation_accounting::CanonicalScalarScope::capture(
            artifact,
            &scope,
            &selected.providers,
            selected.carrier_layout,
        )?;
        let conditions =
            evaluate_scope(&mut observations, primary, demand, |job, observations| {
                let ceiling = crate::evaluation_accounting::PURE_TERMINAL_FUEL_CEILING;
                let mut meter = match sponsor {
                    Some(account) => {
                        TerminalFuelMeter::with_invocation(account.clone(), Some(ceiling))
                            .map_err(|error| format!("scalar evaluation sponsorship: {error:?}"))?
                    }
                    None => TerminalFuelMeter::with_allowance(ceiling),
                };
                let values = observations.values().observed().collect();
                let result = execute_entry(job, observations, &mut meter);
                let outcome = match &result {
                    Ok(observed) => crate::PureTerminalEvaluationOutcome::Returned {
                        value: TerminalExecutionResult::Scalar(observed.value),
                    },
                    Err(reason) => crate::PureTerminalEvaluationOutcome::Failed {
                        reason: reason.clone(),
                    },
                };
                records.push(
                    crate::evaluation_accounting::CanonicalScalarEvaluationReceipt::new(
                        std::sync::Arc::clone(&input),
                        job.application,
                        values,
                        job.results.clone(),
                        outcome,
                        meter.into_usage(),
                    ),
                );
                result
            })?;
        closure.close_scope(observations, conditions, selected.address_model)?;
    }
    closure.finish()
}

pub fn evaluate_scalar_applications<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: &str,
    requested: &[ScalarApplicationId],
    meter: &mut TerminalFuelMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    evaluate_scope(observations, value_scope, requested, |job, observations| {
        execute_entry(job, observations, meter)
    })
}

fn evaluate_scope<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: &str,
    requested: &[ScalarApplicationId],
    mut execute: impl FnMut(
        &demand::Job,
        &ScalarApplicationObservations<'_>,
    ) -> Result<EntryObservation, String>,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    let program = TerminalProgram::borrow(observations.artifact());
    let mut conditions = observations.conditions(value_scope, requested)?;
    let mut jobs = demand::discover(&program, observations, requested, &mut conditions)?;
    let order = demand::execution_order(&jobs)?;
    demand::retain_entry_results(observations, &conditions, &mut jobs)?;

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
            demand::Execution::Entry { machine, .. } => {
                let observed = execute(job, observations)?;
                for (result, captured) in observed.captured {
                    observations.record_entry_result(
                        job.application,
                        *machine,
                        result,
                        captured,
                    )?;
                }
                observations.record_value(job.application, observed.value)?;
            }
        }
    }
    Ok(conditions)
}

struct EntryObservation {
    value: terminal_installation::TerminalScalarValue,
    captured: Vec<(
        semantic_vocabulary::ValueId,
        terminal_installation::TerminalScalarValue,
    )>,
}

fn execute_entry(
    job: &demand::Job,
    observations: &ScalarApplicationObservations<'_>,
    meter: &mut TerminalFuelMeter,
) -> Result<EntryObservation, String> {
    let demand::Execution::Entry {
        machine,
        installation,
    } = &job.execution
    else {
        return Err("scalar entry execution requires an actual entry job".into());
    };
    let account = meter.invocation_account().cloned();
    let program = TerminalProgram::borrow(observations.artifact());
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
    let TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) = result else {
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
    Ok(EntryObservation { value, captured })
}
