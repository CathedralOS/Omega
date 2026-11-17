//! Execute admitted scalar observations from immutable Psi and target inputs.
//! Source completion owns preparation and charging records; this engine has no
//! source, declaration-selection or compilation callback.
use semantic_vocabulary::ScalarApplicationId;
use terminal_installation::{ScalarApplicationClosure, TerminalScalarValue, ScalarApplicationObservations, ScalarConditionSelection};
use terminal_interpreter::{TerminalEntryResultRequest, TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram, TerminalScalarApplicationRequest};
use terminal_work::WorkMeter;

mod unique {
    pub(crate) fn extend_unique<T: PartialEq>(
        target: &mut Vec<T>,
        additions: impl IntoIterator<Item = T>,
    ) {
        for addition in additions {
            push_unique(target, addition);
        }
    }
    pub(crate) fn push_unique<T: PartialEq>(target: &mut Vec<T>, addition: T) {
        if !target.contains(&addition) {
            target.push(addition);
        }
    }
}

mod demand {
    use super::unique::{extend_unique, push_unique};
    use semantic_vocabulary::{MachineId, ScalarApplicationId, ScalarType, ValueId};
    use terminal_installation::{ScalarApplicationObservations, ScalarConditionSelection};
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
            prerequisites: terminal_installation::ScalarEntryPrerequisites<'artifact>,
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
                                native_code_layouts: None,
                                entry: Some(*entry),
                                installation: Some(observations.providers()),
                                target_machines: observations.targets(),
                                target_layouts: observations.layouts(),
                                host_imports: &[],
                                x86_flags: None,
                                memory_ordering: None,
                            })
                            .map_err(|error| format!("scalar evaluation supplies: {error:?}"))?;
                        let requirements = program
                            .execution_requirements(Some(&installation))
                            .map_err(|error| {
                                format!("scalar evaluation dependencies: {error:?}")
                            })?;
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
                                "scalar evaluation needs an unavailable external completion".into(),
                            );
                        }
                        extend_unique(&mut demanded, requirements.scalar_entry_conditions);
                        let prerequisites =
                            terminal_installation::ScalarEntryPrerequisites::prepare(
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
                terminal_psi::find_boundary_machine(&module.boundary_machines, *boundary)
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
            if !matches!(&job.execution, Execution::Entry { machine: entry, .. } if entry == machine)
            {
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
}

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
    meter: &mut WorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    evaluate_scope(
        observations,
        value_scope.into(),
        requested,
        None,
        None,
        |job, admission| execute_entry(job, admission, meter),
    )
}

pub fn evaluate_scalar_applications_with_address_model<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: &str,
    requested: &[ScalarApplicationId],
    address_model: Option<&terminal_installation::ScalarAddressModel>,
    meter: &mut WorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    evaluate_scope(
        observations,
        Some(value_scope),
        requested,
        address_model,
        None,
        |job, admission| execute_entry(job, admission, meter),
    )
}

/// The deployment target one build-time invocation runs under.
///
/// An invocation artifact can retain scalar conditions: ranges, predicates and
/// completions that checking deferred because their values depend on the
/// target. Terminal execution refuses to start until those are admitted, and
/// admitting them needs the target's layout facts, carriers and address model.
/// Those are target-owner knowledge, so the caller that names the target
/// supplies them here; a target without layout facts cannot discharge any
/// condition and the invocation refuses.
#[derive(Clone, Copy)]
pub struct InvocationTarget<'target> {
    pub name: Option<&'target str>,
    pub layout_target: Option<&'target terminal_installation::LayoutTargetFacts>,
    pub carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    pub address_model: Option<&'target terminal_installation::ScalarAddressModel>,
}

/// Discharge the scalar conditions retained for one invocation entry.
///
/// We close the single scope of `target` rooted at the entry's checked proof
/// declaration, which is the invocation-scoped closure calling policies
/// already use, and evaluate every application the entry demands on Terminal
/// Psi under `meter`. Each condition selected for that root is checked as soon
/// as its observations are ready, and a false one fails the invocation; none
/// is skipped.
pub fn complete_invocation_scalar_applications<'artifact>(
    artifact: terminal_codec::TerminalExecutionView<'artifact>,
    target: InvocationTarget<'_>,
    selected: &'artifact terminal_installation::TerminalTargetContext,
    providers: &'artifact terminal_installation::AdmittedProviderSelections,
    declaration: terminal_psi::ProofDeclarationId,
    requested: &[ScalarApplicationId],
    meter: &mut WorkMeter,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let facts = target
        .layout_target
        .ok_or("invocation target has no immutable layout catalogue")?;
    let name = target
        .name
        .ok_or("scalar observations require an actual selected target")?;
    let mut closure =
        ScalarApplicationClosure::for_invocation(artifact, Some(name), &[declaration]);
    let mut observations =
        ScalarApplicationObservations::new(artifact, Some(selected), providers, Some(facts))?
            .with_target_carriers(target.carrier_layout)?
            .with_invocation_roots(&[declaration])?;
    let conditions = evaluate_scope(
        &mut observations,
        Some(name),
        requested,
        target.address_model,
        None,
        |job, admission| execute_entry(job, admission, meter),
    )?;
    closure.close_scope(observations, conditions, Ok(target.address_model.cloned()))?;
    closure.finish()
}

fn evaluate_scope<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: Option<&str>,
    requested: &[ScalarApplicationId],
    address_model: Option<&terminal_installation::ScalarAddressModel>,
    import_realizations: Option<&dyn TargetImportRealizations>,
    mut execute: impl FnMut(
        &demand::Job<'artifact>,
        &terminal_installation::AdmittedScalarEntry<'_, 'artifact>,
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
    let program = TerminalProgram::borrow(observations.artifact());
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
    admission: &terminal_installation::AdmittedScalarEntry<'_, '_>,
    meter: &mut WorkMeter,
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
pub struct ScalarExecutionReceipt {
    pub application: ScalarApplicationId,
    pub values: Vec<(ScalarApplicationId, TerminalScalarValue)>,
    pub captures: Vec<(semantic_vocabulary::ValueId, semantic_vocabulary::ScalarType)>,
    pub outcome: Result<TerminalScalarValue, String>,
    pub usage: terminal_work::WorkUsage,
}

pub fn evaluate_accounted_scope<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: Option<&str>,
    requested: &[ScalarApplicationId],
    address_model: Option<&terminal_installation::ScalarAddressModel>,
    import_realizations: Option<&dyn TargetImportRealizations>,
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    ceiling: u64,
) -> (Result<ScalarConditionSelection<'artifact>, String>, Vec<ScalarExecutionReceipt>) {
    let mut records = Vec::new();
    let result = evaluate_scope(observations, value_scope, requested, address_model, import_realizations, |job, admission| {
        let mut meter = match sponsor {
            Some(account) => WorkMeter::with_invocation(account.clone(), Some(ceiling))
                .map_err(|error| format!("scalar evaluation sponsorship: {error:?}"))?,
            None => WorkMeter::with_budget(ceiling),
        };
        let values = admission.observations().values().observed().collect();
        let result = execute_entry(job, admission, &mut meter);
        records.push(ScalarExecutionReceipt {
            application: job.application,
            values,
            captures: job.results.clone(),
            outcome: result.as_ref().map(|observed| observed.value).map_err(Clone::clone),
            usage: meter.into_usage(),
        });
        result
    });
    (result, records)
}
