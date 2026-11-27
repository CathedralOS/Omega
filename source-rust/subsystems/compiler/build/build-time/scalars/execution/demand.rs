use psi_scalar_applications::{ScalarApplicationObservations, ScalarConditionSelection};
use semantic_vocabulary::{MachineId, ScalarApplicationId, ScalarType, ValueId};
use terminal_interpreter::{
    TerminalProgram, TerminalRuntimeSupplies, TerminalRuntimeSupplyRequest,
};
use terminal_psi::{ScalarApplication, ScalarRangeObservation};

pub(super) struct Job<'artifact> {
    pub application: ScalarApplicationId,
    pub execution: Execution<'artifact>,
    dependencies: Vec<ScalarApplicationId>,
    pub results: Vec<(ValueId, ScalarType)>,
    pub additional_results: bool,
}

pub(super) enum Execution<'artifact> {
    Layout,
    Entry {
        machine: MachineId,
        installation: TerminalRuntimeSupplies,
        // Imports the selected target executes for this entry; `None`
        // when the entry demands no import.
        realized_imports: Option<Box<dyn super::AdmittedImportRealizations>>,
        prerequisites: psi_scalar_applications::ScalarEntryPrerequisites<'artifact>,
    },
}

pub(super) fn discover<'artifact>(
    program: &TerminalProgram<'_>,
    observations: &ScalarApplicationObservations<'artifact>,
    requested: &[ScalarApplicationId],
    conditions: &mut ScalarConditionSelection<'_>,
    import_realizations: Option<&dyn super::TargetImportRealizations>,
) -> Result<Vec<Job<'artifact>>, String> {
    let mut demanded = Vec::new();
    extend_unique(&mut demanded, requested.iter().copied());
    extend_unique(&mut demanded, conditions.applications()?);
    let mut jobs = Vec::new();
    let mut discovered = 0;
    while discovered < demanded.len() {
        let batch_end = demanded.len();
        conditions.include_applications(&demanded[discovered..batch_end]);
        extend_unique(&mut demanded, conditions.applications()?);
        for position in discovered..batch_end {
            let application = demanded[position];
            let declaration = observations
                .artifact()
                .module()
                .scalars
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
                    conditions.include_evaluation_entry(machine)?;
                    extend_unique(&mut demanded, conditions.applications()?);
                    let installation = program
                        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                            assembly_returns: None,
                            native_code_layouts: None,
                            entry: Some(*entry),
                            installation: Some(observations.providers()),
                            target_machines: observations.targets(),
                            target_layouts: observations.layouts(),
                            host_imports: &[],
                            external_completions: &[],
                            x86_flags: None,
                            memory_ordering: None,
                        })
                        .map_err(|error| format!("scalar evaluation supplies: {error:?}"))?;
                    let requirements = program
                        .execution_requirements(Some(&installation))
                        .map_err(|error| format!("scalar evaluation dependencies: {error:?}"))?;
                    // A constant may reach an operation its target
                    // executes without a checked body, such as a
                    // canonical float operation. Only the target's owner
                    // knows which those are, so we ask it for exactly the
                    // imports this entry demands and refuse the rest.
                    let realized_imports = match import_realizations {
                        Some(realizations) if !requirements.imports.is_empty() => {
                            Some(realizations.admit(&requirements.imports)?)
                        }
                        _ => None,
                    };
                    let unrealized = requirements
                        .imports
                        .iter()
                        .copied()
                        .filter(|boundary| {
                            realized_imports
                                .as_ref()
                                .is_none_or(|realized| !realized.realizes(*boundary))
                        })
                        .collect::<Vec<_>>();
                    if !requirements.missing_providers.is_empty() || !unrealized.is_empty() {
                        let module = observations.artifact().module();
                        let names = |boundaries: &[_]| boundary_names(module, boundaries);
                        return Err(format!(
                            "scalar evaluation requires exact authored selection before evaluation; missing providers: [{}]; operations target `{}` cannot execute during constant evaluation: [{}]",
                            names(&requirements.missing_providers),
                            observations.target().unwrap_or("<unscoped>"),
                            names(&unrealized),
                        ));
                    }
                    if !requirements.external_completions.is_empty() {
                        return Err(
                            "scalar evaluation needs an unavailable external completion".into()
                        );
                    }
                    extend_unique(&mut demanded, requirements.scalar_entry_conditions);
                    let prerequisites = psi_scalar_applications::ScalarEntryPrerequisites::prepare(
                        observations,
                        application,
                    )?;
                    let prerequisite_applications = prerequisites.applications()?;
                    (
                        Execution::Entry {
                            machine,
                            installation,
                            realized_imports,
                            prerequisites,
                        },
                        {
                            let mut dependencies = requirements.scalar_applications;
                            extend_unique(&mut dependencies, prerequisite_applications);
                            dependencies
                        },
                    )
                }
            };
            extend_unique(&mut demanded, dependencies.iter().copied());
            jobs.push(Job {
                application,
                execution,
                dependencies,
                results: Vec::new(),
                additional_results: false,
            });
        }
        discovered = batch_end;
    }
    Ok(jobs)
}

// A diagnostic names an operation by its callable identity, which is what
// the author can find in source; the slot number is only a fallback.
fn boundary_names(
    module: &terminal_psi::TerminalModule,
    boundaries: &[semantic_vocabulary::BoundaryMachineId],
) -> String {
    boundaries
        .iter()
        .map(|boundary| {
            terminal_psi::find_boundary_machine(&module.interfaces.boundary_machines, *boundary)
                .and_then(|declaration| module.identities.text(declaration.identity))
                .filter(|identity| !identity.is_empty())
                .map_or_else(
                    || format!("{boundary:?}"),
                    |identity| format!("`{identity}`"),
                )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

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
                let reason = match &jobs[position].execution {
                    Execution::Entry { prerequisites, .. } => {
                        prerequisites.dependency_reason(dependency)?
                    }
                    Execution::Layout => None,
                };
                if let Some(reason) = reason {
                    return Err(format!(
                        "A constant calculation cannot use its unfinished result to prove a call's precondition. The constant invocation cannot establish its own pending target condition: {reason}; scalar evaluation has a dependency cycle at application {}",
                        dependency.get()
                    ));
                }
                return Err(format!(
                    "A constant calculation depends on its own unfinished result; scalar evaluation has a dependency cycle at application {}",
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
        if observations.value(*application)?.is_some()
            && let Err(reason) =
                observations.entry_result(*application, *machine, *result, *scalar_type)
        {
            if observations.extends_completed_application(*application) {
                // A later phase can ask for an additional local result. That
                // expanded output request needs a real, separately charged
                // execution; an identical completed request never runs again.
                job.additional_results = true;
            } else {
                return Err(reason);
            }
        }
        push_unique(&mut job.results, (*result, *scalar_type));
    }
    Ok(())
}
fn extend_unique<T: PartialEq>(target: &mut Vec<T>, additions: impl IntoIterator<Item = T>) {
    for addition in additions {
        push_unique(target, addition);
    }
}
fn push_unique<T: PartialEq>(target: &mut Vec<T>, addition: T) {
    if !target.contains(&addition) {
        target.push(addition);
    }
}
