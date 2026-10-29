//! Discover the whole demand closure before freezing each entry's captures.
//! Only executable reads and geometry extents constrain execution order.

use semantic_vocabulary::{MachineId, ScalarApplicationId, ScalarType, ValueId};
use terminal_installation::{ScalarApplicationObservations, ScalarConditionSelection};
use terminal_interpreter::{
    TerminalProgram, TerminalRuntimeSupplies, TerminalRuntimeSupplyRequest,
};
use terminal_psi::{ScalarApplication, ScalarRangeObservation};

pub(super) struct Job {
    pub application: ScalarApplicationId,
    pub execution: Execution,
    dependencies: Vec<ScalarApplicationId>,
    pub results: Vec<(ValueId, ScalarType)>,
}

pub(super) enum Execution {
    Layout,
    Entry {
        machine: MachineId,
        installation: TerminalRuntimeSupplies,
    },
}

pub(super) fn discover(
    program: &TerminalProgram<'_>,
    observations: &ScalarApplicationObservations<'_>,
    requested: &[ScalarApplicationId],
    conditions: &mut ScalarConditionSelection<'_>,
) -> Result<Vec<Job>, String> {
    let mut demanded = Vec::new();
    extend_demand(&mut demanded, requested.iter().copied());
    extend_demand(&mut demanded, conditions.applications()?);
    let mut jobs = Vec::new();
    let mut discovered = 0;
    while discovered < demanded.len() {
        let batch_end = demanded.len();
        conditions.include_applications(&demanded[discovered..batch_end]);
        extend_demand(&mut demanded, conditions.applications()?);
        for position in discovered..batch_end {
            let application = demanded[position];
            let declaration = observations
                .artifact()
                .module()
                .scalar_applications
                .declaration(application)
                .ok_or("scalar evaluation dependency is absent")?;
            let (execution, dependencies) = match declaration {
                ScalarApplication::Missing => {
                    return Err("scalar evaluation dependency is absent".into());
                }
                ScalarApplication::LayoutObservation { .. } => (
                    Execution::Layout,
                    observations.layout_dependencies(application)?,
                ),
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
                    (
                        Execution::Entry {
                            machine,
                            installation,
                        },
                        requirements.scalar_applications,
                    )
                }
            };
            extend_demand(&mut demanded, dependencies.iter().copied());
            jobs.push(Job {
                application,
                execution,
                dependencies,
                results: Vec::new(),
            });
        }
        discovered = batch_end;
    }
    Ok(jobs)
}

fn extend_demand(
    demanded: &mut Vec<ScalarApplicationId>,
    applications: impl IntoIterator<Item = ScalarApplicationId>,
) {
    for application in applications {
        if !demanded.contains(&application) {
            demanded.push(application);
        }
    }
}

/// Reject the entire execution cycle before running any of its unrelated jobs.
pub(super) fn execution_order(jobs: &[Job]) -> Result<Vec<usize>, String> {
    let mut completed = vec![false; jobs.len()];
    let mut order = Vec::with_capacity(jobs.len());
    for root in 0..jobs.len() {
        if completed[root] {
            continue;
        }
        let mut pending = vec![(root, 0)];
        while let Some((position, next_dependency)) = pending.last_mut() {
            let position = *position;
            if *next_dependency == jobs[position].dependencies.len() {
                completed[position] = true;
                order.push(position);
                pending.pop();
                continue;
            }
            let dependency = jobs[position].dependencies[*next_dependency];
            *next_dependency += 1;
            let dependency_position = jobs
                .iter()
                .position(|job| job.application == dependency)
                .ok_or("scalar execution dependency was not discovered")?;
            if pending
                .iter()
                .any(|(position, _)| *position == dependency_position)
            {
                return Err(format!(
                    "scalar evaluation has a dependency cycle at application {}",
                    dependency.get()
                ));
            }
            if !completed[dependency_position] {
                pending.push((dependency_position, 0));
            }
        }
    }
    Ok(order)
}

pub(super) fn retain_entry_results(
    observations: &ScalarApplicationObservations<'_>,
    conditions: &ScalarConditionSelection<'_>,
    jobs: &mut [Job],
) -> Result<(), String> {
    for observation in conditions.observations()? {
        let ScalarRangeObservation::EntryResult {
            application,
            machine,
            result,
            scalar_type,
        } = observation
        else {
            continue;
        };
        let job = jobs
            .iter_mut()
            .find(|job| job.application == *application)
            .ok_or("scalar condition entry was not discovered")?;
        if !matches!(&job.execution, Execution::Entry { machine: entry, .. } if entry == machine) {
            return Err("scalar condition names another evaluation root".into());
        }
        if observations.value(*application)?.is_some() {
            observations.entry_result(*application, *machine, *result, *scalar_type)?;
        }
        if !job.results.contains(&(*result, *scalar_type)) {
            job.results.push((*result, *scalar_type));
        }
    }
    Ok(())
}
