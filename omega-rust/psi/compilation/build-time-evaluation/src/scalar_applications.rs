/*
    Dependency-ordered Terminal Psi evaluation of demanded scalar applications
    and range observations.

    We topologically order demanded scalar applications across machine call
    graphs, execute admitted target-machine closures in the Terminal
    interpreter, and record proved scalar range observations.
*/

use crate::evaluation_accounting::CanonicalScalarEvaluationReceipt;
use psi_work::WorkMeter;
use semantic_vocabulary::ScalarApplicationId;
use terminal_installation::{ScalarApplicationClosure, TerminalScalarValue};
use terminal_installation::{ScalarApplicationObservations, ScalarConditionSelection};
use terminal_interpreter::{
    TerminalEntryResultRequest, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstantiation, TerminalProgram, TerminalScalarApplicationRequest,
};

mod demand {
    use crate::unique::{extend_unique, push_unique};
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
                                target_machines: Some(observations.targets()),
                                target_layouts: Some(observations.layouts()),
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
                                observations.targets().target(),
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
            if observations.value(*application)?.is_some() {
                observations.entry_result(*application, *machine, *result, *scalar_type)?;
            }
            push_unique(&mut job.results, (*result, *scalar_type));
        }
        Ok(())
    }
}

pub struct ScalarScopeEvaluation {
    pub application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
    pub targets: terminal_installation::AdmittedTargetMachineSelections,
    pub providers: terminal_installation::AdmittedProviderSelections,
    pub address_model: Result<Option<terminal_installation::ScalarAddressModel>, String>,
    pub carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    pub layout_target: terminal_installation::LayoutTargetFacts,
    /// `None` leaves every bodyless import unavailable to evaluation.
    pub import_realizations: Option<Box<dyn TargetImportRealizations>>,
}

/// The selected target's execution of imports that have no checked body.
///
/// Constant evaluation runs on Terminal Psi, where a canonical float operation
/// is a call to a bodyless boundary. The numeric values specification has the
/// selected target's declared realization table decide how each such operation
/// is realized, and that table belongs to the target's owner, outside this
/// crate. The caller that names the target therefore supplies this, and an
/// import it does not realize stays unavailable to constant evaluation.
pub trait TargetImportRealizations {
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
    accounting.append_scalar_evaluations(records);
    result
}

fn complete_scopes(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: &str,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    records: &mut Vec<CanonicalScalarEvaluationReceipt>,
    select: &mut impl FnMut(&str) -> Result<ScalarScopeEvaluation, String>,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let mut closure = ScalarApplicationClosure::new(artifact.view(), primary);
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
        let conditions = evaluate_scope(
            &mut observations,
            primary,
            demand,
            selected
                .address_model
                .as_ref()
                .ok()
                .and_then(Option::as_ref),
            selected.import_realizations.as_deref(),
            |job, admission| {
                let observations = admission.observations();
                let ceiling = crate::evaluation_accounting::PURE_PSI_WORK_BUDGET;
                let mut meter = match sponsor {
                    Some(account) => WorkMeter::with_invocation(account.clone(), Some(ceiling))
                        .map_err(|error| format!("scalar evaluation sponsorship: {error:?}"))?,
                    None => WorkMeter::with_budget(ceiling),
                };
                let values = observations.values().observed().collect();
                let result = execute_entry(job, admission, &mut meter);
                let outcome = match &result {
                    Ok(observed) => crate::PureTerminalEvaluationOutcome::Returned {
                        value: TerminalExecutionResult::Scalar(observed.value),
                    },
                    Err(reason) => crate::PureTerminalEvaluationOutcome::Failed {
                        reason: reason.clone(),
                    },
                };
                records.push(CanonicalScalarEvaluationReceipt::new(
                    std::sync::Arc::clone(&input),
                    job.application,
                    values,
                    job.results.clone(),
                    outcome,
                    meter.into_usage(),
                ));
                result
            },
        )?;
        closure.close_scope(observations, conditions, selected.address_model)?;
    }
    closure.finish()
}

pub fn evaluate_scalar_applications<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: &str,
    requested: &[ScalarApplicationId],
    meter: &mut WorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    evaluate_scope(
        observations,
        value_scope,
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
        value_scope,
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
    pub name: &'target str,
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
pub(crate) fn complete_invocation_scalar_applications<'artifact>(
    artifact: terminal_codec::TerminalExecutionView<'artifact>,
    target: InvocationTarget<'_>,
    selected: &'artifact terminal_installation::AdmittedTargetMachineSelections,
    providers: &'artifact terminal_installation::AdmittedProviderSelections,
    declaration: terminal_psi::ProofDeclarationId,
    requested: &[ScalarApplicationId],
    meter: &mut WorkMeter,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let facts = target
        .layout_target
        .ok_or("invocation target has no immutable layout catalogue")?;
    let mut closure =
        ScalarApplicationClosure::for_invocation(artifact, target.name, &[declaration]);
    let mut observations =
        ScalarApplicationObservations::new(artifact, selected, providers, facts)?
            .with_target_carriers(target.carrier_layout)
            .with_invocation_roots(&[declaration])?;
    let conditions = evaluate_scope(
        &mut observations,
        target.name,
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
    value_scope: &str,
    requested: &[ScalarApplicationId],
    address_model: Option<&terminal_installation::ScalarAddressModel>,
    import_realizations: Option<&dyn TargetImportRealizations>,
    mut execute: impl FnMut(
        &demand::Job<'artifact>,
        &terminal_installation::AdmittedScalarEntry<'_, 'artifact>,
    ) -> Result<EntryObservation, String>,
) -> Result<ScalarConditionSelection<'artifact>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use arena::Arena;
    use numerics::bignum::BigInt;
    use numerics::integer_bounds::IntegerBoundStep;
    use numerics::scalar_predicates::{PredicateNode, PredicateOperator, ScalarValue};
    use semantic_vocabulary::proof_applications::{
        ProofApplicationCall, ProofApplicationMachine, ProofCallMachine,
    };
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, StructuralTypeId, ValueId,
    };
    use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
    use terminal_installation::{
        AdmittedProviderSelections, AdmittedTargetMachineSelections, ScalarConditionSelection,
        admit_provider_installation, admit_target_machine_selections,
    };
    use terminal_interpreter::{TerminalRuntimeSupplyRequest, TerminalScalarValue};
    use terminal_psi::{
        ArrayExtent, Block, LayoutMeasurement, MachineContract, Operation, OperationKind,
        OperationResult, ScalarApplication, ScalarConditionSubject, ScalarPredicateNode,
        ScalarRangeObservation, ScalarRangeRequirement, StructuralTypeDeclaration,
        StructuralTypeShape, TargetMachineCallable, TerminalIdentityTableBuilder, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    };

    mod closure {
        use super::*;
        use semantic_vocabulary::proof_applications::{ProofApplicationMachine, ProofCallMachine};
        use terminal_installation::{ScalarAddressModel, ScalarApplicationClosure};
        use terminal_interpreter::TerminalStructuralValue;
        use terminal_psi::{ProofScopeDeclaration, StructuralPlaceDeclaration};

        #[test]
        fn scalar_closure_does_not_admit_unchecked_entries_as_roots_or_callees() {
            let mut module = graph(24, false, false).view().module().clone();
            module.machines.push(machine(
                3,
                vec![OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(41),
                }],
            ));
            module.machines.push(machine(
                4,
                vec![OperationKind::Call {
                    callee: machine_id(3),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }],
            ));
            module
                .scalar_applications
                .insert(ScalarApplication::Evaluate {
                    entry: TargetMachineCallable::Machine(machine_id(3)),
                    scalar_type: scalar(),
                })
                .unwrap();
            module
                .scalar_applications
                .insert(ScalarApplication::Evaluate {
                    entry: TargetMachineCallable::Machine(machine_id(4)),
                    scalar_type: scalar(),
                })
                .unwrap();
            let mut nodes = Arena::new();
            let root = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                ScalarValue::Boolean(false),
            )));
            module
                .scalar_applications
                .conditions
                .retain_predicate(
                    ScalarConditionSubject::Application(application(6)),
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    "unchecked entry".into(),
                    nodes,
                    root,
                )
                .unwrap();
            let artifact = canonical(module);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
            let (observations, conditions) = evaluated(&artifact, &targets, &providers, 5);
            assert_eq!(observations.value(application(6)).unwrap(), None);
            let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
            closure
                .close_scope(observations, conditions, Ok(None))
                .unwrap();
            let admission = closure.finish().unwrap();
            let program = TerminalProgram::borrow(artifact.view());
            for entry in [3, 4] {
                let installation = program
                    .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                        native_code_layouts: None,
                        entry: Some(TargetMachineCallable::Machine(machine_id(entry))),
                        installation: Some(&providers),
                        target_machines: Some(&targets),
                        target_layouts: Some(admission.primary().layouts()),
                        host_imports: &[],
                        x86_flags: None,
                        memory_ordering: None,
                    })
                    .unwrap();
                let requirements = program.execution_requirements(Some(&installation)).unwrap();
                assert_eq!(requirements.scalar_entry_conditions, vec![application(6)]);
                assert!(requirements.scalar_applications.is_empty());
                let error = program
                    .instantiate(TerminalInstantiation {
                        installation: Some(&installation),
                        scalar_admission: Some(&admission),
                        ..Default::default()
                    })
                    .err()
                    .expect("unchecked entry must refuse before execution");
                assert!(format!("{error:?}").contains("unchecked scalar entry 6"));
            }
            let mut observations = ScalarApplicationObservations::new(
                artifact.view(),
                &targets,
                &providers,
                &terminal_installation::LayoutTargetFacts {
                    profile: targets.target().to_owned(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                },
            )
            .unwrap();
            let error = evaluate_scalar_applications(
                &mut observations,
                "macos_arm64",
                &[application(7)],
                &mut WorkMeter::unbounded(),
            )
            .err()
            .expect("a helper's false prerequisite must refuse before it runs");
            assert!(error.contains("predicate is false"), "{error}");
            assert_eq!(observations.value(application(6)).unwrap(), None);
            assert_eq!(observations.value(application(7)).unwrap(), None);
        }

        #[test]
        fn scalar_closure_derives_supplemental_scope_from_shared_condition_owner() {
            let mut module = scoped_graph(41).view().module().clone();
            module.scalar_applications.conditions = Default::default();
            let scope = &mut module.scalar_applications.proof_scope;
            let owners = scope
                .plan
                .symbols
                .span(scope.plan.obligation_owners)
                .expect("the scoped graph records its per-target owners")
                .to_vec();
            scope.plan.has_obligations = false;
            scope.plan.obligation_owners = Default::default();
            let helper = next_declaration(scope);
            scope
                .declarations
                .insert(ProofScopeDeclaration::Unavailable("shared helper".into()));
            scope.plan.machines.insert(ProofApplicationMachine {
                machine: helper,
                product_selected: true,
                ..Default::default()
            });
            for caller in owners {
                let calls = scope.calls.calls.insert_many([
                    semantic_vocabulary::proof_applications::ProofApplicationCall {
                        callee: helper,
                        ..Default::default()
                    },
                ]);
                scope.calls.insert_machine(ProofCallMachine {
                    machine: caller,
                    calls,
                    ..Default::default()
                });
            }
            module
                .scalar_applications
                .conditions
                .retain_range(
                    ScalarConditionSubject::Declaration(helper),
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    ScalarRangeRequirement::Fixed {
                        minimum: BigInt::from_u64(41),
                        maximum: BigInt::from_u64(41),
                    },
                    vec![IntegerBoundStep::Observation(
                        ScalarRangeObservation::Literal(BigInt::from_u64(41)),
                    )],
                )
                .unwrap();
            let artifact = canonical(module);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
            let observations = ScalarApplicationObservations::new(
                artifact.view(),
                &targets,
                &providers,
                &terminal_installation::LayoutTargetFacts {
                    profile: targets.target().to_owned(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                },
            )
            .unwrap();
            let conditions = observations.conditions("macos_arm64", &[]).unwrap();
            assert_eq!(conditions.ranges(), &[0]);
            let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
            assert_eq!(
                closure.scopes().collect::<Vec<_>>(),
                vec!["macos_arm64", "windows_x86_64"]
            );
            closure
                .close_scope(observations, conditions, Ok(None))
                .unwrap();
            assert!(
                closure
                    .finish()
                    .err()
                    .unwrap()
                    .contains("unfinished authored scope")
            );
        }

        #[test]
        fn scalar_closure_supplies_resolved_extent_to_ordinary_entry_storage() {
            let mut module = graph(24, false, false).view().module().clone();
            let mut entry = machine(
                3,
                vec![OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(24),
                }],
            );
            entry
                .parameter_order
                .push(terminal_psi::BoundaryParameterKind::Structural);
            entry.structural_places.push(StructuralPlaceDeclaration {
                id: semantic_vocabulary::PlaceId::new(1).unwrap(),
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter { position: 0 },
            });
            entry
                .structural_parameters
                .push(terminal_psi::StructuralParameterDeclaration {
                    place: semantic_vocabulary::PlaceId::new(1).unwrap(),
                    position: 0,
                    structural_type: type_id(3),
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                });
            module.machines.push(entry);
            let artifact = canonical(module);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
            let (observations, conditions) = evaluated(&artifact, &targets, &providers, 5);
            let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
            closure
                .close_scope(observations, conditions, Ok(None))
                .unwrap();
            let admission = closure.finish().unwrap();
            assert!(matches!(
                artifact
                    .view()
                    .module()
                    .structural_type(type_id(3))
                    .unwrap()
                    .shape,
                StructuralTypeShape::FixedArray {
                    length: ArrayExtent::Application(_),
                    ..
                }
            ));
            assert!(matches!(
                admission
                    .geometry()
                    .iter()
                    .find(|row| row.id == type_id(3))
                    .unwrap()
                    .shape,
                StructuralTypeShape::FixedArray {
                    length: ArrayExtent::Literal(16),
                    ..
                }
            ));
            let program = TerminalProgram::borrow(artifact.view());
            let installation = program
                .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                    native_code_layouts: None,
                    entry: Some(TargetMachineCallable::Machine(machine_id(3))),
                    installation: Some(&providers),
                    target_machines: Some(&targets),
                    target_layouts: Some(admission.primary().layouts()),
                    host_imports: &[],
                    x86_flags: None,
                    memory_ordering: None,
                })
                .unwrap();
            let arguments = [TerminalStructuralValue::minted(1, type_id(3), Vec::new())];
            for count in [15, 16, 17] {
                let arrays = [terminal_interpreter::TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: Vec::new(),
                    bytes: vec![73; count],
                }];
                let instance = program.instantiate(TerminalInstantiation {
                    installation: Some(&installation),
                    scalar_admission: Some(&admission),
                    structural_inputs: terminal_interpreter::TerminalStructuralInputs {
                        arguments: &arguments,
                        byte_arrays: &arrays,
                        ..Default::default()
                    },
                    ..Default::default()
                });
                if count == 16 {
                    let result = instance
                        .unwrap()
                        .invoke(&mut WorkMeter::unbounded())
                        .unwrap();
                    assert!(
                        matches!(result, TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) if value == integer(24))
                    );
                } else {
                    assert!(instance.is_err(), "wrong-sized entry backing must refuse");
                }
            }
        }

        #[test]
        fn scalar_closure_admits_ordinary_execution_after_complete_checking() {
            let artifact = graph(24, false, false);
            let bytes = artifact.to_bytes();
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            for target in ["macos_arm64", "windows_x86_64"] {
                let targets = admit_target_machine_selections(artifact.view(), target);
                let mut observations = ScalarApplicationObservations::new(
                    artifact.view(),
                    &targets,
                    &providers,
                    &terminal_installation::LayoutTargetFacts {
                        profile: targets.target().to_owned(),
                        pointer_size: 8,
                        pointer_alignment: 8,
                        case_tag_size: 4,
                        case_tag_alignment: 4,
                        little_endian: true,
                    },
                )
                .unwrap();
                let conditions = evaluate_scalar_applications(
                    &mut observations,
                    target,
                    &[application(5)],
                    &mut WorkMeter::unbounded(),
                )
                .unwrap();
                let mut closure = ScalarApplicationClosure::new(artifact.view(), target);
                closure
                    .close_scope(observations, conditions, Ok(None))
                    .unwrap();
                let admission = closure.finish().unwrap();
                let other_target = if target == "macos_arm64" {
                    "windows_x86_64"
                } else {
                    "macos_arm64"
                };
                let other_selections =
                    admit_target_machine_selections(artifact.view(), other_target);
                let other_layouts = terminal_installation::admit_target_layouts(
                    artifact.view(),
                    &other_selections,
                    &layout_plans::LayoutTargetFacts {
                        profile: other_selections.target().to_owned(),
                        pointer_size: 8,
                        pointer_alignment: 8,
                        case_tag_size: 4,
                        case_tag_alignment: 4,
                        little_endian: true,
                    },
                )
                .unwrap();
                let program = TerminalProgram::borrow(artifact.view());
                let other_installation = program
                    .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                        native_code_layouts: None,
                        entry: Some(TargetMachineCallable::Machine(machine_id(2))),
                        installation: Some(&providers),
                        target_machines: Some(&other_selections),
                        target_layouts: Some(&other_layouts),
                        host_imports: &[],
                        x86_flags: None,
                        memory_ordering: None,
                    })
                    .unwrap();
                assert!(
                    program
                        .instantiate(TerminalInstantiation {
                            installation: Some(&other_installation),
                            scalar_admission: Some(&admission),
                            ..Default::default()
                        })
                        .is_err(),
                    "completed values cannot move to another target"
                );
                let reloaded = CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
                for code in [artifact.view(), reloaded.view()] {
                    let program = TerminalProgram::borrow(code);
                    let installation = program
                        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                            native_code_layouts: None,
                            entry: Some(TargetMachineCallable::Machine(machine_id(2))),
                            installation: Some(&providers),
                            target_machines: Some(&targets),
                            target_layouts: Some(admission.primary().layouts()),
                            host_imports: &[],
                            x86_flags: None,
                            memory_ordering: None,
                        })
                        .unwrap();
                    let mut instance = program
                        .instantiate(TerminalInstantiation {
                            installation: Some(&installation),
                            scalar_admission: Some(&admission),
                            ..Default::default()
                        })
                        .unwrap();
                    let result = instance.invoke(&mut WorkMeter::unbounded()).unwrap();
                    assert!(
                        matches!(result, TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) if value == integer(24))
                    );
                }
                assert_eq!(artifact.to_bytes(), bytes);
            }
        }

        fn scoped_graph(windows_value: u64) -> CanonicalTerminalArtifact {
            let mut module = graph(24, false, false).view().module().clone();
            let scope = &mut module.scalar_applications.proof_scope;
            scope.plan.has_obligations = true;
            let mut owners = Vec::new();
            for target in ["macos_arm64", "windows_x86_64"] {
                let owner = next_declaration(scope);
                scope
                    .declarations
                    .insert(ProofScopeDeclaration::Unavailable(format!(
                        "{target} owner"
                    )));
                scope.plan.machines.insert(ProofApplicationMachine {
                    machine: owner,
                    authored_target: Some(target.into()),
                    source_target: Some(target.into()),
                    product_selected: true,
                    ..Default::default()
                });
                owners.push(owner);
            }
            scope.plan.obligation_owners = scope.plan.symbols.insert_many(owners.iter().copied());
            for (owner, value) in owners.into_iter().zip([41, windows_value]) {
                module
                    .scalar_applications
                    .conditions
                    .retain_range(
                        ScalarConditionSubject::Declaration(owner),
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        ScalarRangeRequirement::Fixed {
                            minimum: BigInt::from_u64(41),
                            maximum: BigInt::from_u64(41),
                        },
                        vec![IntegerBoundStep::Observation(
                            ScalarRangeObservation::Literal(BigInt::from_u64(value)),
                        )],
                    )
                    .unwrap();
            }
            canonical(module)
        }

        #[test]
        fn scalar_closure_requires_supplemental_scopes_without_replacing_primary_values() {
            let artifact = scoped_graph(41);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let macos = admit_target_machine_selections(artifact.view(), "macos_arm64");
            let windows = admit_target_machine_selections(artifact.view(), "windows_x86_64");
            for complete_supplement in [false, true] {
                let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
                assert_eq!(
                    closure.scopes().collect::<Vec<_>>(),
                    vec!["macos_arm64", "windows_x86_64"]
                );
                let (observations, conditions) = evaluated(&artifact, &macos, &providers, 5);
                closure
                    .close_scope(observations, conditions, Ok(None))
                    .unwrap();
                if !complete_supplement {
                    assert!(
                        closure
                            .finish()
                            .err()
                            .unwrap()
                            .contains("unfinished authored scope")
                    );
                    continue;
                }
                let (observations, conditions) = evaluated(&artifact, &windows, &providers, 2);
                assert_eq!(observations.value(application(5)).unwrap(), None);
                closure
                    .close_scope(observations, conditions, Ok(None))
                    .unwrap();
                let admission = closure.finish().unwrap();
                assert_eq!(admission.values().value(application(5)), Some(integer(24)));
                assert_eq!(admission.primary().targets(), &macos);
                assert_eq!(admission.supplemental()[0].targets(), &windows);
            }
        }

        #[test]
        fn scalar_closure_refuses_false_debt_and_does_not_drop_evaluated_entry_conditions() {
            let artifact = graph(23, false, false);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
            for omit_entry_debt in [false, true] {
                let mut observations = ScalarApplicationObservations::new(
                    artifact.view(),
                    &targets,
                    &providers,
                    &terminal_installation::LayoutTargetFacts {
                        profile: targets.target().to_owned(),
                        pointer_size: 8,
                        pointer_alignment: 8,
                        case_tag_size: 4,
                        case_tag_alignment: 4,
                        little_endian: true,
                    },
                )
                .unwrap();
                let error = evaluate_scalar_applications(
                    &mut observations,
                    "macos_arm64",
                    &[application(5)],
                    &mut WorkMeter::unbounded(),
                )
                .err()
                .expect("the completed entry's false result predicate must reject");
                assert!(error.contains("predicate is false"), "{error}");
                assert_eq!(
                    observations.value(application(5)).unwrap(),
                    Some(integer(24))
                );
                let entry = [application(5)];
                let conditions = observations
                    .conditions("macos_arm64", if omit_entry_debt { &[] } else { &entry })
                    .unwrap();
                let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
                let error = closure
                    .close_scope(observations, conditions, Err("unavailable model".into()))
                    .unwrap_err();
                assert!(
                    error.contains(if omit_entry_debt {
                        "unselected range debt"
                    } else {
                        "predicate is false"
                    }),
                    "{error}"
                );
                assert!(closure.finish().is_err());
            }
        }

        #[test]
        fn scalar_closure_requires_exact_address_model_inputs() {
            let mut module = graph(24, false, false).view().module().clone();
            module
                .scalar_applications
                .conditions
                .retain_range(
                    ScalarConditionSubject::Module,
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    ScalarRangeRequirement::AddressModel,
                    vec![IntegerBoundStep::Observation(
                        ScalarRangeObservation::Literal(BigInt::from_u64(41)),
                    )],
                )
                .unwrap();
            let artifact = canonical(module);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
            for model_target in [None, Some("windows_x86_64"), Some("macos_arm64")] {
                let (observations, conditions) = evaluated(&artifact, &targets, &providers, 5);
                let model = model_target.map(|target| ScalarAddressModel {
                    target: target.into(),
                    carrier: IntegerType::supported_address_representation(),
                    semantics_version: 1,
                    commitment: [42; 32],
                });
                let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
                let closed = closure.close_scope(observations, conditions, Ok(model.clone()));
                if model_target == Some("macos_arm64") {
                    closed.unwrap();
                    assert_eq!(
                        closure.finish().unwrap().primary().address_model(),
                        model.as_ref()
                    );
                } else {
                    assert!(closed.unwrap_err().contains("address model"));
                    assert!(closure.finish().is_err());
                }
            }
        }
    }

    fn machine_id(value: u64) -> MachineId {
        MachineId::new(value).unwrap()
    }
    fn value_id(value: u64) -> ValueId {
        ValueId::new(value).unwrap()
    }
    fn type_id(value: u64) -> StructuralTypeId {
        StructuralTypeId::new(value).unwrap()
    }
    fn application(value: u64) -> ScalarApplicationId {
        ScalarApplicationId::new(value).unwrap()
    }
    fn integer_type() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    }
    fn scalar() -> ScalarType {
        ScalarType::Integer(integer_type())
    }

    fn evaluated<'a>(
        artifact: &'a CanonicalTerminalArtifact,
        targets: &'a AdmittedTargetMachineSelections,
        providers: &'a AdmittedProviderSelections,
        entry: u64,
    ) -> (
        ScalarApplicationObservations<'a>,
        ScalarConditionSelection<'a>,
    ) {
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            targets,
            providers,
            &terminal_installation::LayoutTargetFacts {
                profile: targets.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            },
        )
        .unwrap();
        let conditions = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(entry)],
            &mut WorkMeter::unbounded(),
        )
        .unwrap();
        (observations, conditions)
    }
    fn integer(value: u128) -> TerminalScalarValue {
        TerminalScalarValue::Integer {
            scalar_type: integer_type(),
            value: IntegerValue::Unsigned(value),
        }
    }

    fn machine(identity: u64, operations: Vec<OperationKind>) -> TerminalMachine {
        let result = ValueDeclaration {
            id: value_id(operations.len() as u64),
            scalar_type: scalar(),
            qualifications: Default::default(),
        };
        TerminalMachine {
            id: machine_id(identity),
            attachment: None,
            declaration: None,
            nominal_cleanup_subject: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_entry_claims: Vec::new(),
            result: TerminalMachineResult::Scalar(result),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![Block {
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: operations
                    .into_iter()
                    .enumerate()
                    .map(|(position, kind)| {
                        Operation::unsuspended(
                            OperationId::new(position as u64 + 1).unwrap(),
                            OperationResult::Scalar(ValueDeclaration {
                                id: value_id(position as u64 + 1),
                                scalar_type: scalar(),
                                qualifications: Default::default(),
                            }),
                            kind,
                        )
                    })
                    .collect(),
                terminator: Terminator::Return {
                    edge: EdgeId::new(1).unwrap(),
                    value: result.id,
                    cleanup_actions: Vec::new(),
                },
            }],
            ranked_scc: None,
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            contract: MachineContract::empty(ContractId::new(identity).unwrap()),
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        }
    }

    fn graph(expected: u64, cyclic: bool, signed_extent: bool) -> CanonicalTerminalArtifact {
        let mut module = TerminalModule::for_entry(machine_id(2));
        let mut identities = TerminalIdentityTableBuilder::default();
        for (identity, shape) in [
            (1, StructuralTypeShape::PrimitiveScalar(scalar())),
            (
                2,
                StructuralTypeShape::FixedArray {
                    element: type_id(1),
                    length: ArrayExtent::Literal(2),
                },
            ),
            (
                3,
                StructuralTypeShape::FixedArray {
                    element: type_id(4),
                    length: ArrayExtent::Application(application(if cyclic { 5 } else { 2 })),
                },
            ),
            (
                4,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                )),
            ),
        ] {
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                id: type_id(identity),
                identity: identities.intern(&format!("graph::type{identity}")),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                scalar_restriction: None,
                shape,
            });
        }
        module.identities = identities.build();
        let mut extent = machine(
            1,
            vec![OperationKind::ReadScalarApplication {
                application: application(1),
            }],
        );
        let extent_type = if signed_extent {
            let signed = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
            extent.result = TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(1),
                scalar_type: signed,
                qualifications: Default::default(),
            });
            extent.blocks[0].operations[0] = Operation::unsuspended(
                OperationId::new(1).unwrap(),
                OperationResult::Scalar(ValueDeclaration {
                    id: value_id(1),
                    scalar_type: signed,
                    qualifications: Default::default(),
                }),
                OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(16),
                },
            );
            signed
        } else {
            scalar()
        };
        module.machines = vec![
            extent,
            machine(
                2,
                vec![
                    OperationKind::ReadScalarApplication {
                        application: application(3),
                    },
                    OperationKind::ReadScalarApplication {
                        application: application(4),
                    },
                    OperationKind::WrappingIntegerAdd {
                        left: value_id(1),
                        right: value_id(2),
                    },
                ],
            ),
        ];
        for row in [
            ScalarApplication::LayoutObservation {
                subject: type_id(2),
                measurement: LayoutMeasurement::Size,
                semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                scalar_type: scalar(),
            },
            ScalarApplication::Evaluate {
                entry: TargetMachineCallable::Machine(machine_id(1)),
                scalar_type: extent_type,
            },
            ScalarApplication::LayoutObservation {
                subject: type_id(3),
                measurement: LayoutMeasurement::Size,
                semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                scalar_type: scalar(),
            },
            ScalarApplication::LayoutObservation {
                subject: type_id(2),
                measurement: LayoutMeasurement::Alignment,
                semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                scalar_type: scalar(),
            },
            ScalarApplication::Evaluate {
                entry: TargetMachineCallable::Machine(machine_id(2)),
                scalar_type: scalar(),
            },
        ] {
            module.scalar_applications.insert(row).unwrap();
        }
        module
            .scalar_applications
            .conditions
            .retain_range(
                ScalarConditionSubject::Application(application(5)),
                semantic_vocabulary::ScalarConditionRole::Result,
                ScalarRangeRequirement::Fixed {
                    minimum: BigInt::from_u64(16),
                    maximum: BigInt::from_u64(16),
                },
                vec![IntegerBoundStep::Observation(
                    ScalarRangeObservation::EntryResult {
                        application: application(5),
                        machine: machine_id(2),
                        result: value_id(1),
                        scalar_type: scalar(),
                    },
                )],
            )
            .unwrap();
        let mut nodes = Arena::new();
        let left = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
            ScalarRangeObservation::Application(application(5)),
        )));
        let right = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
            ScalarValue::Integer(BigInt::from_u64(expected)),
        )));
        let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
            operator: PredicateOperator::Equal,
            left,
            right,
        }));
        module
            .scalar_applications
            .conditions
            .retain_predicate(
                ScalarConditionSubject::Application(application(5)),
                semantic_vocabulary::ScalarConditionRole::Result,
                "total layout size".into(),
                nodes,
                root,
            )
            .unwrap();
        canonical(module)
    }

    fn canonical(mut module: TerminalModule) -> CanonicalTerminalArtifact {
        retain_graph_proof_scope(&mut module);
        let optimization = build_identity_optimization_execution_record(&module).unwrap();
        let artifact = CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap();
        CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap()
    }

    fn retain_graph_proof_scope(module: &mut TerminalModule) {
        use semantic_vocabulary::compiler_evaluation::{
            CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
        };
        use terminal_psi::{ProofCallableAssociation, ProofCallableHeader, ProofScopeDeclaration};

        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let scope = &mut module.scalar_applications.proof_scope;
        for machine in &module.machines {
            let callable = TargetMachineCallable::Machine(machine.id);
            if scope.callable_declaration(callable).is_some() {
                continue;
            }
            let declaration = next_declaration(scope);
            let identity = identities.intern(&graph_machine_name(machine.id));
            scope
                .declarations
                .insert(ProofScopeDeclaration::Nominal(identity));
            scope.callables.insert(ProofCallableAssociation::Callable {
                declaration,
                callable,
            });
            scope.plan.machines.insert(ProofApplicationMachine {
                machine: declaration,
                product_selected: true,
                ..Default::default()
            });
        }
        for machine in &module.machines {
            let declaration = scope
                .callable_declaration(TargetMachineCallable::Machine(machine.id))
                .unwrap();
            if !scope
                .callable_headers
                .iter()
                .any(|(_, header)| header.declaration == declaration)
            {
                scope.callable_headers.insert(ProofCallableHeader {
                    declaration,
                    compiler_evaluation: Ok(CompilerEvaluationDeclaration {
                        name: graph_machine_name(machine.id),
                        custody: DeclarationCustody::Toolchain,
                        effects: CompilerEvaluationEffects::default(),
                        local_terminates: true,
                        entry_requires: false,
                        linear_carrier: None,
                        static_application: None,
                        selections: Ok(Vec::new()),
                    }),
                    ..ProofCallableHeader::default()
                });
            }
            if scope
                .calls
                .machines()
                .iter()
                .any(|(_, row)| row.machine == declaration)
            {
                continue;
            }
            let calls = machine
                .operations()
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Call { callee, .. } => {
                        let callee = scope
                            .callable_declaration(TargetMachineCallable::Machine(*callee))
                            .expect("test call target must have its declaration association");
                        Some(ProofApplicationCall {
                            callee,
                            state: callee,
                            ..Default::default()
                        })
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let calls = scope.calls.calls.insert_many(calls);
            scope.calls.insert_machine(ProofCallMachine {
                machine: declaration,
                calls,
                ..Default::default()
            });
        }
        module.identities = identities.build();
    }

    fn graph_machine_name(machine: MachineId) -> String {
        format!("scalar-graph::machine{}", machine.get())
    }

    fn next_declaration(
        scope: &terminal_psi::ProofApplicationScope,
    ) -> terminal_psi::ProofDeclarationId {
        terminal_psi::ProofDeclarationId(u32::try_from(scope.declarations.len() + 1).unwrap())
    }

    #[test]
    fn canonical_scalar_jobs_do_not_capture_an_unselected_declarations_result() {
        let mut module = graph(24, false, false).view().module().clone();
        let scope = &mut module.scalar_applications.proof_scope;
        let dormant = next_declaration(scope);
        scope
            .declarations
            .insert(terminal_psi::ProofScopeDeclaration::Unavailable(
                "dormant declaration".into(),
            ));
        scope.plan.machines.insert(ProofApplicationMachine {
            machine: dormant,
            authored_target: Some("windows_x86_64".into()),
            source_target: Some("windows_x86_64".into()),
            product_selected: true,
            ..Default::default()
        });
        let result = ValueDeclaration {
            id: value_id(99),
            scalar_type: scalar(),
            qualifications: Default::default(),
        };
        module.machines[1].blocks.push(Block {
            id: BlockId::new(99).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: vec![Operation::unsuspended(
                OperationId::new(99).unwrap(),
                OperationResult::Scalar(result),
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0),
                },
            )],
            terminator: Terminator::Return {
                edge: EdgeId::new(99).unwrap(),
                value: value_id(99),
                cleanup_actions: Vec::new(),
            },
        });
        module
            .scalar_applications
            .conditions
            .retain_range(
                ScalarConditionSubject::Declaration(dormant),
                semantic_vocabulary::ScalarConditionRole::Result,
                ScalarRangeRequirement::Fixed {
                    minimum: BigInt::from_u64(0),
                    maximum: BigInt::from_u64(0),
                },
                vec![IntegerBoundStep::Observation(
                    ScalarRangeObservation::EntryResult {
                        application: application(5),
                        machine: machine_id(2),
                        result: value_id(99),
                        scalar_type: scalar(),
                    },
                )],
            )
            .unwrap();
        let artifact = canonical(module);
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: targets.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            },
        )
        .unwrap();
        evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(5)],
            &mut WorkMeter::unbounded(),
        )
        .unwrap();
        assert_eq!(
            observations.value(application(5)).unwrap(),
            Some(integer(24))
        );
        let windows = admit_target_machine_selections(artifact.view(), "windows_x86_64");
        let mut selected = ScalarApplicationObservations::new(
            artifact.view(),
            &windows,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: windows.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            },
        )
        .unwrap();
        let error = evaluate_scalar_applications(
            &mut selected,
            "macos_arm64",
            &[application(5)],
            &mut WorkMeter::unbounded(),
        )
        .err()
        .unwrap();
        assert!(
            error.contains("entry returned without completing every requested result occurrence"),
            "{error}"
        );
    }

    fn sibling_extent_graph() -> CanonicalTerminalArtifact {
        let mut module = graph(24, false, false).view().module().clone();
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        module.machines.push(machine(
            3,
            vec![
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(3),
                },
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(4),
                },
            ],
        ));
        module.machines.push(machine(
            4,
            vec![OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(5),
            }],
        ));
        for identity in [3, 4] {
            module
                .scalar_applications
                .insert(ScalarApplication::Evaluate {
                    entry: TargetMachineCallable::Machine(machine_id(identity)),
                    scalar_type: scalar(),
                })
                .unwrap();
        }
        for (identity, extent) in [(5, 6), (6, 7)] {
            let mut declaration = module.structural_types[3].clone();
            declaration.id = type_id(identity);
            declaration.identity = identities.intern(&format!("graph::type{identity}"));
            declaration.shape = StructuralTypeShape::FixedArray {
                element: type_id(4),
                length: ArrayExtent::Application(application(extent)),
            };
            module.structural_types.push(declaration);
        }
        let mut record = module.structural_types[3].clone();
        record.id = type_id(7);
        record.identity = identities.intern("graph::type7");
        let fields: Vec<_> = [5, 6]
            .into_iter()
            .map(|identity| terminal_psi::StructuralFieldDeclaration {
                inline_carrier: None,
                id: semantic_vocabulary::StructuralFieldId::new(identity).unwrap(),
                identity: identities.intern(&format!("graph::type7::field{identity}")),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Structural(type_id(identity)),
                storage: None,
            })
            .collect();
        module.native_layouts.insert(
            record.id,
            record.identity,
            terminal_psi::TerminalIdentityIndex::ABSENT,
            terminal_psi::TerminalIdentityIndex::ABSENT,
            fields
                .iter()
                .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: field.identity,
                }),
            [],
        );
        record.shape = StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::Packed,
            fields,
        };
        module.structural_types.push(record);
        module.identities = identities.build();
        module
            .scalar_applications
            .insert(ScalarApplication::LayoutObservation {
                subject: type_id(7),
                measurement: LayoutMeasurement::Size,
                semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                scalar_type: scalar(),
            })
            .unwrap();
        module
            .scalar_applications
            .conditions
            .retain_range(
                ScalarConditionSubject::Application(application(7)),
                semantic_vocabulary::ScalarConditionRole::Result,
                ScalarRangeRequirement::Fixed {
                    minimum: BigInt::from_u64(3),
                    maximum: BigInt::from_u64(3),
                },
                vec![IntegerBoundStep::Observation(
                    ScalarRangeObservation::EntryResult {
                        application: application(6),
                        machine: machine_id(3),
                        result: value_id(1),
                        scalar_type: scalar(),
                    },
                )],
            )
            .unwrap();
        canonical(module)
    }

    #[test]
    fn canonical_scalar_jobs_discover_sibling_conditions_before_execution() {
        let artifact = sibling_extent_graph();
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let (observations, conditions) = evaluated(&artifact, &targets, &providers, 8);
        assert_eq!(conditions.ranges(), &[1]);
        assert!(conditions.predicates().is_empty());
        assert_eq!(
            observations.value(application(8)).unwrap(),
            Some(integer(9))
        );
        assert_eq!(
            observations.value(application(6)).unwrap(),
            Some(integer(4))
        );
        assert_eq!(
            observations
                .entry_result(application(6), machine_id(3), value_id(1), scalar())
                .unwrap(),
            integer(3)
        );
        observations.discharge_range(1, None).unwrap();
        assert_eq!(
            observations.value(application(5)).unwrap(),
            None,
            "unrelated application stays dormant"
        );
    }

    #[test]
    fn canonical_scalar_jobs_refuse_replay_to_fill_a_late_capture() {
        let artifact = sibling_extent_graph();
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: targets.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            },
        )
        .unwrap();
        let mut meter = WorkMeter::unbounded();
        evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(6)],
            &mut meter,
        )
        .unwrap();
        let usage = meter.usage().clone();
        let error = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(8)],
            &mut meter,
        )
        .err()
        .unwrap();
        assert!(error.contains("unobserved entry result"), "{error}");
        assert_eq!(
            meter.usage(),
            &usage,
            "missing capture refuses before any new execution"
        );
        assert_eq!(observations.value(application(7)).unwrap(), None);
        assert_eq!(observations.value(application(8)).unwrap(), None);
    }

    #[test]
    fn canonical_scalar_jobs_compose_geometry_entries_and_exact_observations() {
        let artifact = graph(24, false, false);
        let original = artifact.to_bytes();
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        for target in ["windows_x86_64", "macos_arm64"] {
            let selections = admit_target_machine_selections(artifact.view(), target);
            let mut observations = ScalarApplicationObservations::new(
                artifact.view(),
                &selections,
                &providers,
                &terminal_installation::LayoutTargetFacts {
                    profile: selections.target().to_owned(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                },
            )
            .unwrap();
            let mut meter = WorkMeter::unbounded();
            evaluate_scalar_applications(&mut observations, target, &[application(5)], &mut meter)
                .unwrap();
            for (identity, expected) in [(1, 16), (2, 16), (3, 16), (4, 8), (5, 24)] {
                assert_eq!(
                    observations.value(application(identity)).unwrap(),
                    Some(integer(expected))
                );
            }
            observations.discharge_range(0, None).unwrap();
            observations.discharge_predicate(0).unwrap();
            let usage = meter.usage().clone();
            evaluate_scalar_applications(&mut observations, target, &[application(5)], &mut meter)
                .unwrap();
            assert_eq!(meter.usage(), &usage, "completed jobs are not re-executed");
            assert_eq!(
                artifact.to_bytes(),
                original,
                "target observations never rewrite canonical input"
            );
        }
    }

    #[test]
    fn canonical_scalar_jobs_check_local_prerequisites_before_completion() {
        for expected_operand in [16, 17] {
            let mut module = graph(24, false, false).view().module().clone();
            module
                .scalar_applications
                .conditions
                .retain_range(
                    ScalarConditionSubject::Application(application(5)),
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    ScalarRangeRequirement::Fixed {
                        minimum: BigInt::from_u64(expected_operand),
                        maximum: BigInt::from_u64(expected_operand),
                    },
                    vec![IntegerBoundStep::Observation(
                        ScalarRangeObservation::EntryResult {
                            application: application(5),
                            machine: machine_id(2),
                            result: value_id(1),
                            scalar_type: scalar(),
                        },
                    )],
                )
                .unwrap();
            let artifact = canonical(module);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let targets = admit_target_machine_selections(artifact.view(), "macos_arm64");
            let mut observations = ScalarApplicationObservations::new(
                artifact.view(),
                &targets,
                &providers,
                &terminal_installation::LayoutTargetFacts {
                    profile: targets.target().to_owned(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                },
            )
            .unwrap();
            let result = evaluate_scalar_applications(
                &mut observations,
                "macos_arm64",
                &[application(5)],
                &mut WorkMeter::unbounded(),
            );
            if expected_operand == 16 {
                result.unwrap();
                assert_eq!(
                    observations.value(application(5)).unwrap(),
                    Some(integer(24))
                );
                assert_eq!(
                    observations
                        .entry_result(application(5), machine_id(2), value_id(1), scalar())
                        .unwrap(),
                    integer(16)
                );
                observations.discharge_range(1, None).unwrap();
                observations.discharge_predicate(0).unwrap();
            } else {
                let error = result
                    .err()
                    .expect("a false local prerequisite must stop the entry before completion");
                assert!(
                    error.contains("constant invocation prerequisite"),
                    "{error}"
                );
                assert!(
                    error.contains("bounds [16, 16] lie outside [17, 17]"),
                    "{error}"
                );
                assert_eq!(observations.value(application(5)).unwrap(), None);
                assert!(
                    observations
                        .entry_result(application(5), machine_id(2), value_id(1), scalar())
                        .is_err(),
                    "a private capture from a failed entry must not become a completed observation"
                );
            }
        }
    }

    #[test]
    fn canonical_scalar_jobs_refuse_cycles_and_false_conditions() {
        for cyclic in [false, true] {
            let artifact = graph(23, cyclic, false);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let selections = admit_target_machine_selections(artifact.view(), "macos_arm64");
            let mut observations = ScalarApplicationObservations::new(
                artifact.view(),
                &selections,
                &providers,
                &terminal_installation::LayoutTargetFacts {
                    profile: selections.target().to_owned(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                },
            )
            .unwrap();
            let mut meter = WorkMeter::unbounded();
            let usage = meter.usage().clone();
            let evaluated = evaluate_scalar_applications(
                &mut observations,
                "macos_arm64",
                &[application(4), application(5)],
                &mut meter,
            );
            if cyclic {
                assert!(evaluated.err().unwrap().contains("dependency cycle"));
                assert_eq!(meter.usage(), &usage);
                for identity in 1..=5 {
                    assert_eq!(observations.value(application(identity)).unwrap(), None);
                }
            } else {
                let error = evaluated
                    .err()
                    .expect("a false result predicate must reject the completed evaluation");
                assert!(error.contains("predicate is false"), "{error}");
                assert_eq!(
                    observations.value(application(5)).unwrap(),
                    Some(integer(24))
                );
                assert!(
                    observations
                        .discharge_predicate(0)
                        .unwrap_err()
                        .contains("predicate is false")
                );
                assert!(
                    TerminalProgram::borrow(artifact.view())
                        .instantiate(TerminalInstantiation::default())
                        .is_err(),
                    "a completed value with a false result promise cannot admit ordinary execution"
                );
            }
        }
    }

    #[test]
    fn canonical_scalar_jobs_preserve_nonnegative_signed_extent_carriers() {
        let artifact = graph(24, false, true);
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let selections = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &selections,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: selections.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            },
        )
        .unwrap();
        evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(5)],
            &mut WorkMeter::unbounded(),
        )
        .unwrap();
        observations.discharge_range(0, None).unwrap();
        observations.discharge_predicate(0).unwrap();
        assert_eq!(observations.extent(application(2)).unwrap(), Some(16));
    }

    #[test]
    fn canonical_scalar_jobs_bind_provisional_inputs_to_the_target_and_entry() {
        use terminal_interpreter::{TerminalArtifactInterpretError, TerminalInterpretError};
        let artifact = graph(24, false, false);
        let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let windows = admit_target_machine_selections(artifact.view(), "windows_x86_64");
        let macos = admit_target_machine_selections(artifact.view(), "macos_arm64");
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &windows,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: windows.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            },
        )
        .unwrap();
        observations.observe_layout(application(1)).unwrap();
        let program = TerminalProgram::borrow(artifact.view());
        for (selected, entry) in [
            (&macos, machine_id(1)),
            (&windows, machine_id(2)),
            (&windows, machine_id(1)),
        ] {
            let layouts = terminal_installation::admit_target_layouts(
                artifact.view(),
                selected,
                &layout_plans::LayoutTargetFacts {
                    profile: selected.target().to_owned(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                },
            )
            .unwrap();
            let installation = program
                .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                    native_code_layouts: None,
                    entry: Some(TargetMachineCallable::Machine(entry)),
                    installation: Some(&providers),
                    target_machines: Some(selected),
                    target_layouts: Some(&layouts),
                    host_imports: &[],
                    x86_flags: None,
                    memory_ordering: None,
                })
                .unwrap();
            let prerequisites = terminal_installation::ScalarEntryPrerequisites::prepare(
                &observations,
                application(2),
            )
            .unwrap();
            let admission = prerequisites.admit(&observations, None).unwrap();
            let result = program.instantiate(TerminalInstantiation {
                installation: Some(&installation),
                scalar_application: Some(TerminalScalarApplicationRequest {
                    admission: &admission,
                }),
                ..Default::default()
            });
            if selected == &windows && entry == machine_id(1) {
                assert!(
                    matches!(result.unwrap().invoke(&mut WorkMeter::unbounded()).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) if value == integer(16))
                );
            } else {
                assert!(
                    matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::UnsupportedSemanticVariant(reason)))
                if reason == "scalar evaluation inputs differ from the exact artifact, entry, target or provider selections")
                );
            }
        }
    }

    #[test]
    fn canonical_scalar_jobs_do_not_admit_raw_byte_execution() {
        use terminal_interpreter::{TerminalArtifactInterpretError, TerminalInterpretError};
        let artifact = graph(24, false, false);
        let result = terminal_interpreter::TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            &[],
            terminal_interpreter::TerminalStructuralInputs::default(),
        );
        assert!(
            matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::ScalarApplicationEvaluation(reason)))
        if reason == "ordinary execution requires admission of retained scalar conditions")
        );
    }
}
