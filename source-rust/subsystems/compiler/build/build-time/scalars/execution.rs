//! Execute admitted scalar observations from immutable Psi and target inputs.
//! Source completion owns preparation and charging records; this engine has no
//! source, declaration-selection or compilation callback.
use build_model::work::EvaluationWorkMeter;
use psi_scalar_applications::{ScalarApplicationObservations, ScalarConditionSelection};
use semantic_vocabulary::ScalarApplicationId;
use terminal_interpreter::{
    TerminalEntryResultRequest, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstantiation, TerminalProgram, TerminalScalarApplicationRequest,
};
use terminal_semantics::entry_values::TerminalScalarValue;

mod demand;

/// The selected target's execution of imports that have no checked body.
///
/// Constant evaluation runs on Terminal Psi, where a canonical float operation
/// is a call to a bodyless boundary. The numeric values specification has the
/// selected target's declared realization table decide how each such operation
/// is realized, and that table belongs to the target's owner, outside this
/// crate. The caller that names the target therefore supplies this, and an
/// import it does not realize stays unavailable to constant evaluation.
pub trait TargetImportRealizations: Send + Sync {
    /// Admit the realizations for the imports one entry demands. An import the
    /// target does not realize is left out rather than refused here, so the
    /// evaluator can name it.
    fn admit(
        &self,
        imports: &[semantic_vocabulary::BoundaryMachineId],
    ) -> Result<Box<dyn AdmittedImportRealizations>, String>;
}

/// Realizations admitted for one entry's demanded imports.
pub trait AdmittedImportRealizations {
    fn realizes(&self, boundary: semantic_vocabulary::BoundaryMachineId) -> bool;

    /// Bind every realized import into the instance of the entry they were
    /// admitted for.
    fn bind(
        &self,
        instance: &mut terminal_interpreter::TerminalInstance<'_, '_>,
    ) -> Result<(), String>;
}

pub fn evaluate_scalar_applications<'artifact, 'scope>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: impl Into<Option<&'scope str>>,
    requested: &[ScalarApplicationId],
    address_model: Option<&semantic_vocabulary::ScalarAddressModel>,
    meter: &mut EvaluationWorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    evaluate_scope(
        observations,
        value_scope.into(),
        requested,
        address_model,
        None,
        |job, admission| execute_entry(job, admission, meter),
    )
}

fn evaluate_scope<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: Option<&str>,
    requested: &[ScalarApplicationId],
    address_model: Option<&semantic_vocabulary::ScalarAddressModel>,
    import_realizations: Option<&dyn TargetImportRealizations>,
    mut execute: impl FnMut(
        &demand::Job<'artifact>,
        &psi_scalar_applications::AdmittedScalarEntry<'_, 'artifact>,
    ) -> Result<EntryObservation, String>,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    observations.require_completed_address_model(address_model)?;
    if address_model.is_some_and(|model| {
        Some(model.target.as_str()) != observations.target()
            || !model.carrier.is_address()
            || model.semantics_version == 0
    }) {
        return Err("scalar evaluation address model differs from its selected deployment".into());
    }
    if import_realizations.is_some() && observations.target().is_none() {
        return Err("scalar import realizations require an explicit deployment selection".into());
    }
    let program =
        TerminalProgram::borrow(observations.artifact()).map_err(|error| error.to_string())?;
    let mut conditions = observations.conditions(value_scope, requested)?;
    let mut jobs = demand::discover(
        &program,
        observations,
        requested,
        &mut conditions,
        import_realizations,
    )?;
    let order = demand::execution_order(&jobs)?;
    demand::retain_entry_results(observations, &conditions, &mut jobs)?;
    conditions.discharge_ready(observations)?;

    for position in order {
        let job = &jobs[position];
        if observations.value(job.application)?.is_some() && !job.additional_results {
            continue;
        }
        if job.additional_results {
            observations.request_additional_entry_results(job.application, &job.results)?;
        }
        match &job.execution {
            demand::Execution::Layout => {
                observations
                    .observe_layout(job.application)
                    .map_err(|error| error.to_string())?;
            }
            demand::Execution::Entry {
                machine,
                prerequisites,
                ..
            } => {
                let admission = prerequisites.admit(observations, address_model)?;
                let observed = execute(job, &admission)?;
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
        conditions.discharge_ready(observations)?;
    }
    Ok(conditions)
}

struct EntryObservation {
    value: TerminalScalarValue,
    captured: Vec<(semantic_vocabulary::ValueId, TerminalScalarValue)>,
}

fn execute_entry(
    job: &demand::Job,
    admission: &psi_scalar_applications::AdmittedScalarEntry<'_, '_>,
    meter: &mut EvaluationWorkMeter,
) -> Result<EntryObservation, String> {
    let demand::Execution::Entry {
        machine,
        installation,
        realized_imports,
        ..
    } = &job.execution
    else {
        return Err("scalar entry execution requires an actual entry job".into());
    };
    let observations = admission.observations();
    let account = meter.payment().invocation().cloned();
    let program =
        TerminalProgram::borrow(observations.artifact()).map_err(|error| error.to_string())?;
    let mut instance = program
        .instantiate(TerminalInstantiation {
            float_evaluation:
                terminal_interpreter::float_result_policy::FloatEvaluation::CompileTime,
            account: account.as_ref(),
            installation: Some(installation),
            entry_results: Some(TerminalEntryResultRequest {
                machine: *machine,
                results: &job.results,
            }),
            scalar_application: Some(TerminalScalarApplicationRequest { admission }),
            ..TerminalInstantiation::default()
        })
        .map_err(|error| format!("scalar evaluation entry: {error:?}"))?;
    if let Some(realized) = realized_imports {
        realized.bind(&mut instance)?;
    }
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

/// Actual execution data for one demanded application, including failures.
pub(crate) struct ScalarExecutionReceipt {
    pub application: ScalarApplicationId,
    pub values: Vec<(ScalarApplicationId, TerminalScalarValue)>,
    pub captures: Vec<(
        semantic_vocabulary::ValueId,
        semantic_vocabulary::ScalarType,
    )>,
    pub outcome: Result<TerminalScalarValue, String>,
    pub usage: terminal_work::WorkUsage,
}

pub(crate) fn evaluate_accounted_scope<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: Option<&str>,
    requested: &[ScalarApplicationId],
    address_model: Option<&semantic_vocabulary::ScalarAddressModel>,
    import_realizations: Option<&dyn TargetImportRealizations>,
    sponsor: Option<&build_model::accounting::EvaluationInvocationAccount>,
    ceiling: u64,
) -> (
    Result<ScalarConditionSelection<'artifact>, String>,
    Vec<ScalarExecutionReceipt>,
) {
    let mut records = Vec::new();
    let result = evaluate_scope(
        observations,
        value_scope,
        requested,
        address_model,
        import_realizations,
        |job, admission| {
            let mut meter = match sponsor {
                Some(account) => build_model::work::sponsored_meter(account.clone(), Some(ceiling))
                    .map_err(|error| format!("scalar evaluation sponsorship: {error:?}"))?,
                None => build_model::work::unsponsored_meter(Some(ceiling)),
            };
            let values = admission.observations().values().observed().collect();
            let result = execute_entry(job, admission, &mut meter);
            records.push(ScalarExecutionReceipt {
                application: job.application,
                values,
                captures: job.results.clone(),
                outcome: result
                    .as_ref()
                    .map(|observed| observed.value)
                    .map_err(Clone::clone),
                usage: meter.into_usage(),
            });
            result
        },
    );
    (result, records)
}
