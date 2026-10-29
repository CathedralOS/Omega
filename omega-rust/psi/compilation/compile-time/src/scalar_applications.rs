//! Execute dependency-ready canonical scalar jobs, without reopening source.
//!
//! Geometry and complete Omega bodies alternate in one dependency worklist.
//! The existing interpreter owns call/provider reachability and execution;
//! installation owns the target-bound provisional observations. This operation
//! does not choose condition applicability or publish an admitted target view.
//! Its caller must discharge that complete closure before ordinary execution.

use semantic_vocabulary::ScalarApplicationId;
use terminal_fuel::TerminalFuelMeter;
use terminal_installation::{ScalarApplicationObservations, ScalarLayoutObservationError};
use terminal_interpreter::{
    TerminalEntryResultRequest, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstantiation, TerminalProgram, TerminalRuntimeSupplyRequest,
    TerminalScalarApplicationRequest,
};
use terminal_psi::{ScalarApplication, ScalarRangeObservation};

#[cfg(test)]
mod tests;

pub fn evaluate_scalar_applications(
    observations: &mut ScalarApplicationObservations<'_>,
    requested: &[ScalarApplicationId],
    meter: &mut TerminalFuelMeter,
) -> Result<(), String> {
    let artifact = observations.artifact();
    let program = TerminalProgram::borrow(artifact);
    let account = meter.invocation_account().cloned();
    for &root in requested {
        let mut pending = vec![root];
        while let Some(application) = pending.last().copied() {
            if observations.value(application)?.is_some() {
                pending.pop();
                continue;
            }
            let declaration = artifact
                .module()
                .scalar_applications
                .declaration(application)
                .ok_or("scalar evaluation dependency is absent")?;
            match declaration {
                ScalarApplication::Missing => {
                    return Err("scalar evaluation dependency is absent".into());
                }
                ScalarApplication::LayoutObservation { .. } => {
                    match observations.observe_layout(application) {
                        Ok(_) => {
                            pending.pop();
                        }
                        Err(ScalarLayoutObservationError::Geometry(
                            terminal_semantics::StorageGeometryError::UnresolvedExtent(dependency),
                        )) => {
                            require_dependency(&mut pending, dependency)?;
                        }
                        Err(error) => return Err(error.to_string()),
                    }
                }
                ScalarApplication::Evaluate { entry, .. } => {
                    let machine = observations.entry(application)?;
                    let installation = program
                        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                            entry: Some(*entry),
                            installation: Some(observations.providers()),
                            target_machines: Some(observations.targets()),
                            target_layouts: Some(observations.layouts()),
                            host_imports: &[],
                            x86_flags: None,
                            memory_ordering: None,
                        })
                        .map_err(|error| format!("scalar evaluation supplies: {error:?}"))?;
                    let requirements = program
                        .execution_requirements(Some(&installation))
                        .map_err(|error| format!("scalar evaluation dependencies: {error:?}"))?;
                    if !requirements.imports.is_empty()
                        || !requirements.missing_providers.is_empty()
                        || !requirements.external_completions.is_empty()
                    {
                        return Err(
                            "scalar evaluation needs an unavailable provider or external effect"
                                .into(),
                        );
                    }
                    let mut dependency = None;
                    for candidate in requirements.scalar_applications {
                        if observations.value(candidate)?.is_none() {
                            dependency = Some(candidate);
                            break;
                        }
                    }
                    if let Some(dependency) = dependency {
                        require_dependency(&mut pending, dependency)?;
                        continue;
                    }
                    let mut results = Vec::new();
                    for observation in artifact
                        .module()
                        .scalar_applications
                        .conditions
                        .observations()
                    {
                        if let ScalarRangeObservation::EntryResult {
                            application: owner,
                            machine: entry,
                            result,
                            scalar_type,
                        } = observation
                            && *owner == application
                        {
                            if *entry != machine {
                                return Err("scalar condition names another evaluation root".into());
                            }
                            if !results.contains(&(*result, *scalar_type)) {
                                results.push((*result, *scalar_type));
                            }
                        }
                    }
                    let mut instance = program
                        .instantiate(TerminalInstantiation {
                            account: account.as_ref(),
                            installation: Some(&installation),
                            entry_results: Some(TerminalEntryResultRequest {
                                machine,
                                results: &results,
                            }),
                            scalar_application: Some(TerminalScalarApplicationRequest {
                                application,
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
                    let captured = results
                        .iter()
                        .map(|(result, _)| {
                            instance
                                .observed_entry_result(machine, *result)
                                .map(|value| (*result, value))
                                .map_err(|error| {
                                    format!("scalar evaluation observation: {error:?}")
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    drop(instance);
                    for (result, value) in captured {
                        observations.record_entry_result(application, machine, result, value)?;
                    }
                    observations.record_value(application, value)?;
                    pending.pop();
                }
            }
        }
    }
    Ok(())
}

fn require_dependency(
    pending: &mut Vec<ScalarApplicationId>,
    dependency: ScalarApplicationId,
) -> Result<(), String> {
    if pending.contains(&dependency) {
        return Err(format!(
            "scalar evaluation has a dependency cycle at application {}",
            dependency.get()
        ));
    }
    pending.push(dependency);
    Ok(())
}
