//! Prepare target scopes and retain each scalar execution's actual receipt.
//! The source owner supplies immutable scopes to the artifact executor.
use crate::evaluation::evaluation_accounting::CanonicalScalarEvaluationReceipt;
use semantic_vocabulary::ScalarApplicationId;
use terminal_installation::{ScalarApplicationClosure, ScalarApplicationObservations};
use terminal_interpreter::TerminalExecutionResult;
pub use artifact_evaluation::scalar_applications::{
    InvocationTarget, TargetImportRealizations, AdmittedImportRealizations,
    evaluate_scalar_applications, evaluate_scalar_applications_with_address_model,
    complete_invocation_scalar_applications,
};
#[cfg(test)]
use terminal_work::WorkMeter;
#[cfg(test)]
use terminal_interpreter::{TerminalInstantiation, TerminalProgram, TerminalExecutionStatus, TerminalScalarApplicationRequest};
pub struct ScalarScopeEvaluation {
    pub application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
    /// No deployment means ordinary Psi evaluation, not host selection.
    /// Layout facts must be absent as well; target-only operations then refuse
    /// at their actual demand rather than suppressing pure conditions.
    pub targets: Option<terminal_installation::TerminalTargetContext>,
    pub providers: terminal_installation::AdmittedProviderSelections,
    pub address_model: Result<Option<terminal_installation::ScalarAddressModel>, String>,
    pub carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    pub layout_target: Option<terminal_installation::LayoutTargetFacts>,
    /// `None` leaves every bodyless import unavailable to evaluation.
    pub import_realizations: Option<Box<dyn TargetImportRealizations>>,
}

/// A completed Product scope and the actual target execution inputs that
/// produced it. This owner is invocation-local; saved artifacts carry no cache.
/// Native completion may add roots and demands, but cannot swap its providers,
/// import adapters, physical layout, address model or sponsored work account.
pub struct ScalarApplicationCompletion {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: Option<String>,
    completed: terminal_installation::AdmittedScalarApplications,
    scopes: Vec<ScalarScopeEvaluation>,
    sponsor: Option<evaluation_accounting::EvaluationInvocationAccount>,
}

impl ScalarApplicationCompletion {
    pub fn artifact(&self) -> &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }

    pub fn primary_target(&self) -> Option<&str> {
        self.primary.as_deref()
    }

    pub fn admitted(&self) -> &terminal_installation::AdmittedScalarApplications {
        &self.completed
    }

    pub fn into_admitted(self) -> terminal_installation::AdmittedScalarApplications {
        self.completed
    }

    pub fn extend(
        self,
        deployment_scopes: Option<&[String]>,
        roots: &[(String, Vec<terminal_psi::ProofDeclarationId>)],
        requested: &[ScalarApplicationId],
        accounting: &mut crate::evaluation::SourceCompletionAccounting,
    ) -> Result<terminal_installation::AdmittedScalarApplications, String> {
        if roots.iter().any(|(target, _)| {
            !self.scopes.iter().any(|scope| {
                scope
                    .targets
                    .as_ref()
                    .is_some_and(|selected| selected.target() == target)
            })
        }) {
            return Err("scalar completion extension names another authored target".into());
        }
        let mut scopes = self.scopes.into_iter().map(Some).collect::<Vec<_>>();
        let mut records = Vec::new();
        let mut completed_scopes = Vec::new();
        let result = complete_scopes(
            &self.artifact,
            self.primary.as_deref(),
            deployment_scopes,
            requested,
            self.sponsor.as_ref(),
            &mut records,
            &mut completed_scopes,
            Some(&self.completed),
            &mut |target| {
                let position = scopes
                    .iter()
                    .position(|scope| {
                        scope.as_ref().is_some_and(|scope| {
                            scope.targets.as_ref().map(|selected| selected.target()) == target
                        })
                    })
                    .ok_or("scalar completion extension has no retained target inputs")?;
                let mut selected = scopes[position].take().expect("selected retained scope");
                if let Some((_, declarations)) = roots
                    .iter()
                    .find(|(scope, _)| Some(scope.as_str()) == target)
                {
                    selected.application_roots = Some(declarations.clone());
                }
                Ok(selected)
            },
        );
        accounting.append_scalar_evaluations(records);
        result
    }
}

pub fn complete_scalar_applications<'scope>(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    accounting: &mut crate::evaluation::SourceCompletionAccounting,
    select: impl FnMut(Option<&str>) -> Result<ScalarScopeEvaluation, String>,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    prepare_scalar_applications(
        artifact,
        primary,
        deployment_scopes,
        requested,
        sponsor,
        accounting,
        select,
    )
    .map(ScalarApplicationCompletion::into_admitted)
}

pub fn prepare_scalar_applications<'scope>(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    accounting: &mut crate::evaluation::SourceCompletionAccounting,
    mut select: impl FnMut(Option<&str>) -> Result<ScalarScopeEvaluation, String>,
) -> Result<ScalarApplicationCompletion, String> {
    let primary = primary.into();
    let mut records = Vec::new();
    let mut scopes = Vec::new();
    let completed = complete_scopes(
        artifact,
        primary,
        deployment_scopes,
        requested,
        sponsor,
        &mut records,
        &mut scopes,
        None,
        &mut select,
    );
    accounting.append_scalar_evaluations(records);
    Ok(ScalarApplicationCompletion {
        artifact: std::sync::Arc::clone(artifact),
        primary: primary.map(str::to_owned),
        completed: completed?,
        scopes,
        sponsor: sponsor.cloned(),
    })
}

fn complete_scopes(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: Option<&str>,
    deployment_scopes: Option<&[String]>,
    requested: &[ScalarApplicationId],
    sponsor: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    records: &mut Vec<CanonicalScalarEvaluationReceipt>,
    completed_scopes: &mut Vec<ScalarScopeEvaluation>,
    previous: Option<&terminal_installation::AdmittedScalarApplications>,
    select: &mut impl FnMut(Option<&str>) -> Result<ScalarScopeEvaluation, String>,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let mut closure = ScalarApplicationClosure::new(artifact.view(), primary);
    if let Some(scopes) = deployment_scopes {
        closure = closure.with_deployment_scopes(scopes);
    }
    let scopes = closure
        .scopes()
        .map(|scope| scope.map(str::to_owned))
        .collect::<Vec<_>>();
    for scope in scopes {
        let scope = scope.as_deref();
        let selected = select(scope)?;
        if selected.targets.as_ref().map(|targets| targets.target()) != scope {
            return Err("scalar evaluation selected a different authored scope".into());
        }
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            selected.targets.as_ref(),
            &selected.providers,
            selected.layout_target.as_ref(),
        )?
        .with_target_carriers(selected.carrier_layout)?
        .with_application_roots(selected.application_roots.as_deref())?;
        if let Some(previous) = previous {
            observations = observations.with_completed_scope(previous, &selected.address_model)?;
        }
        let demand = if scope == primary { requested } else { &[] };
        let input = crate::evaluation::evaluation_accounting::CanonicalScalarScope::capture(
            artifact,
            scope,
            &selected.providers,
            selected.carrier_layout,
        )?;
        let (conditions, executions) = artifact_evaluation::scalar_applications::evaluate_accounted_scope(
            &mut observations,
            primary,
            demand,
            selected.address_model.as_ref().ok().and_then(Option::as_ref),
            selected.import_realizations.as_deref(),
            sponsor,
            crate::evaluation::evaluation_accounting::PURE_PSI_WORK_BUDGET,
        );
        for execution in executions {
            let outcome = match execution.outcome {
                Ok(value) => crate::evaluation::PureTerminalEvaluationOutcome::Returned {
                    value: TerminalExecutionResult::Scalar(value),
                },
                Err(reason) => crate::evaluation::PureTerminalEvaluationOutcome::Failed { reason },
            };
            records.push(CanonicalScalarEvaluationReceipt::new(
                std::sync::Arc::clone(&input), execution.application, execution.values,
                execution.captures, outcome, execution.usage,
            ));
        }
        let conditions = conditions?;
        closure.close_scope(observations, conditions, selected.address_model.clone())?;
        completed_scopes.push(selected);
    }
    closure.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::Arena;
    use bignum::BigInt;
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
        AdmittedProviderSelections, ScalarConditionSelection, TerminalTargetContext,
        admit_provider_installation, terminal_target_context,
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
            let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
                        target_layouts: admission.primary().layouts(),
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
            let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
                vec![Some("macos_arm64"), Some("windows_x86_64")]
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
            let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
                    target_layouts: admission.primary().layouts(),
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
                let targets = terminal_target_context(artifact.view(), target);
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
                let other_selections = terminal_target_context(artifact.view(), other_target);
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
                            target_layouts: admission.primary().layouts(),
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
        fn unscoped_scalar_closure_retains_required_authored_scopes() {
            let artifact = scoped_graph(41);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let mut closure = ScalarApplicationClosure::new(artifact.view(), None);
            assert_eq!(
                closure.scopes().collect::<Vec<_>>(),
                vec![None, Some("macos_arm64"), Some("windows_x86_64")]
            );
            let mut observations =
                ScalarApplicationObservations::new(artifact.view(), None, &providers, None)
                    .unwrap();
            let conditions = evaluate_scalar_applications(
                &mut observations,
                None,
                &[application(5)],
                &mut WorkMeter::unbounded(),
            )
            .unwrap();
            closure
                .close_scope(observations, conditions, Ok(None))
                .unwrap();
            assert!(
                closure
                    .finish()
                    .unwrap_err()
                    .contains("unfinished authored scope")
            );
        }

        #[test]
        fn scalar_closure_requires_supplemental_scopes_without_replacing_primary_values() {
            let artifact = scoped_graph(41);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let macos = terminal_target_context(artifact.view(), "macos_arm64");
            let windows = terminal_target_context(artifact.view(), "windows_x86_64");
            for complete_supplement in [false, true] {
                let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
                assert_eq!(
                    closure.scopes().collect::<Vec<_>>(),
                    vec![Some("macos_arm64"), Some("windows_x86_64")]
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
                assert_eq!(admission.primary().targets(), Some(&macos));
                assert_eq!(
                    admission.supplemental().next().unwrap().targets(),
                    Some(&windows)
                );
            }
        }

        #[test]
        fn scalar_closure_refuses_false_debt_and_does_not_drop_evaluated_entry_conditions() {
            let artifact = graph(23, false, false);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
            let targets = terminal_target_context(artifact.view(), "macos_arm64");
            for model_target in [None, Some("windows_x86_64"), Some("macos_arm64")] {
                let (observations, conditions) = evaluated(&artifact, &targets, &providers, 5);
                let model = model_target.map(|target| ScalarAddressModel {
                    target: target.into(),
                    carrier:
                        semantic_vocabulary::ClosedIntegerType::supported_address_representation(),
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

    fn unscoped_constant(
        expected: u64,
        needs_address: bool,
    ) -> std::sync::Arc<CanonicalTerminalArtifact> {
        let mut module = TerminalModule::for_entry(machine_id(1));
        module.machines.push(machine(
            1,
            vec![OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(41),
            }],
        ));
        module
            .scalar_applications
            .insert(ScalarApplication::Evaluate {
                entry: TargetMachineCallable::Machine(machine_id(1)),
                scalar_type: scalar(),
            })
            .unwrap();
        let mut nodes = Arena::new();
        let left = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
            ScalarRangeObservation::Application(application(1)),
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
                ScalarConditionSubject::Application(application(1)),
                semantic_vocabulary::ScalarConditionRole::Result,
                "unscoped constant result".into(),
                nodes,
                root,
            )
            .unwrap();
        if needs_address {
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
        }
        std::sync::Arc::new(canonical(module))
    }

    fn complete_unscoped(
        artifact: &std::sync::Arc<CanonicalTerminalArtifact>,
        accounting: &mut crate::evaluation::SourceCompletionAccounting,
    ) -> Result<terminal_installation::AdmittedScalarApplications, String> {
        complete_scalar_applications(
            artifact,
            None,
            None,
            &[application(1)],
            None,
            accounting,
            |scope| {
                assert_eq!(scope, None);
                Ok(unscoped_inputs(artifact))
            },
        )
    }

    fn unscoped_inputs(artifact: &CanonicalTerminalArtifact) -> ScalarScopeEvaluation {
        ScalarScopeEvaluation {
            application_roots: None,
            targets: None,
            providers: admit_provider_installation(artifact.view(), &[], &[]).unwrap(),
            address_model: Ok(None),
            carrier_layout: None,
            layout_target: None,
            import_realizations: None,
        }
    }

    #[test]
    fn product_completion_extends_native_demand_without_reexecuting_conditions() {
        let mut module = unscoped_constant(41, false).view().module().clone();
        module.scalar_applications.conditions = Default::default();
        module
            .scalar_applications
            .conditions
            .retain_completion(ScalarConditionSubject::Module, application(1));
        module.machines.push(machine(
            2,
            vec![OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(42),
            }],
        ));
        module
            .scalar_applications
            .insert(ScalarApplication::Evaluate {
                entry: TargetMachineCallable::Machine(machine_id(2)),
                scalar_type: scalar(),
            })
            .unwrap();
        let artifact = std::sync::Arc::new(canonical(module));
        let mut accounting = crate::evaluation::SourceCompletionAccounting::default();
        let first =
            prepare_scalar_applications(&artifact, None, None, &[], None, &mut accounting, |_| {
                Ok(unscoped_inputs(&artifact))
            })
            .unwrap();
        assert_eq!(
            first.admitted().values().value(application(1)),
            Some(integer(41))
        );
        assert_eq!(accounting.receipts().len(), 1);
        let native = first
            .extend(None, &[], &[application(2)], &mut accounting)
            .unwrap();
        assert_eq!(native.values().value(application(1)), Some(integer(41)));
        assert_eq!(native.values().value(application(2)), Some(integer(42)));
        assert_eq!(
            accounting.receipts().len(),
            2,
            "Product formation's completed entry must not run again for native demand"
        );
    }

    fn completion_inputs(
        artifact: &CanonicalTerminalArtifact,
        target: &str,
    ) -> ScalarScopeEvaluation {
        ScalarScopeEvaluation {
            application_roots: None,
            targets: Some(terminal_target_context(artifact.view(), target)),
            providers: admit_provider_installation(artifact.view(), &[], &[]).unwrap(),
            address_model: Ok(None),
            carrier_layout: None,
            layout_target: Some(terminal_installation::LayoutTargetFacts {
                profile: target.into(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            }),
            import_realizations: None,
        }
    }

    #[test]
    fn product_completion_charges_an_expanded_capture_request_and_checks_prior_results() {
        let mut module = sibling_extent_graph().view().module().clone();
        module
            .scalar_applications
            .conditions
            .retain_completion(ScalarConditionSubject::Module, application(6));
        let artifact = std::sync::Arc::new(canonical(module));
        let target = "macos_arm64";
        let mut accounting = crate::evaluation::SourceCompletionAccounting::default();
        let completed = prepare_scalar_applications(
            &artifact,
            target,
            None,
            &[],
            None,
            &mut accounting,
            |_| Ok(completion_inputs(&artifact, target)),
        )
        .unwrap();
        assert_eq!(accounting.receipts().len(), 1);
        let initial = completed.admitted().clone();
        let completed = completed
            .extend(
                Some(&[target.into()]),
                &[(target.into(), Vec::new())],
                &[application(8)],
                &mut accounting,
            )
            .unwrap();
        assert_eq!(completed.values().value(application(8)), Some(integer(9)));
        let applications = accounting
            .receipts()
            .iter()
            .map(|receipt| match receipt {
                crate::evaluation::PureEvaluationRecord::CanonicalScalar(receipt) => receipt.application(),
                _ => panic!("only scalar executions were requested"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            applications,
            [application(6), application(6), application(7)],
            "the newly requested local capture has a real additional execution receipt"
        );

        let inputs = completion_inputs(&artifact, target);
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            inputs.targets.as_ref(),
            &inputs.providers,
            inputs.layout_target.as_ref(),
        )
        .unwrap()
        .with_completed_scope(&completed, &Ok(None))
        .unwrap();
        assert_eq!(
            observations
                .entry_result(application(6), machine_id(3), value_id(1), scalar())
                .unwrap(),
            integer(3)
        );
        assert!(
            observations
                .record_value(application(6), integer(5))
                .unwrap_err()
                .contains("previously observed value")
        );
        assert!(
            observations
                .record_entry_result(application(6), machine_id(3), value_id(1), integer(4))
                .unwrap_err()
                .contains("previously captured result")
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
        assert!(
            observations
                .record_value(application(6), integer(4))
                .unwrap_err()
                .contains("more than once")
        );
        let mut expanded = ScalarApplicationObservations::new(
            artifact.view(),
            inputs.targets.as_ref(),
            &inputs.providers,
            inputs.layout_target.as_ref(),
        )
        .unwrap()
        .with_completed_scope(&initial, &Ok(None))
        .unwrap();
        expanded
            .request_additional_entry_results(application(6), &[(value_id(1), scalar())])
            .unwrap();
        assert_eq!(expanded.value(application(6)).unwrap(), None);
        assert!(
            expanded
                .record_value(application(6), integer(5))
                .unwrap_err()
                .contains("previously observed value")
        );
        expanded
            .record_entry_result(application(6), machine_id(3), value_id(1), integer(3))
            .unwrap();
        expanded.record_value(application(6), integer(4)).unwrap();
        assert!(!expanded.extends_completed_application(application(6)));
    }

    #[test]
    fn product_completion_refuses_changed_execution_inputs_before_reusing_observations() {
        let artifact = unscoped_constant(41, false);
        let target = "macos_arm64";
        let mut accounting = crate::evaluation::SourceCompletionAccounting::default();
        let completion = prepare_scalar_applications(
            &artifact,
            target,
            None,
            &[application(1)],
            None,
            &mut accounting,
            |_| Ok(completion_inputs(&artifact, target)),
        )
        .unwrap();
        let other = unscoped_constant(42, false);
        for changed in ["artifact", "target", "layout", "carrier", "address"] {
            let code = if changed == "artifact" {
                &other
            } else {
                &artifact
            };
            let mut inputs = completion_inputs(
                code,
                if changed == "target" {
                    "linux_arm64"
                } else {
                    target
                },
            );
            if changed == "layout" {
                inputs.layout_target.as_mut().unwrap().case_tag_size = 8;
            }
            if changed == "address" {
                inputs.address_model = Ok(Some(terminal_installation::ScalarAddressModel {
                    target: target.into(),
                    carrier:
                        semantic_vocabulary::ClosedIntegerType::supported_address_representation(),
                    semantics_version: 1,
                    commitment: [1; 32],
                }));
            }
            let observations = ScalarApplicationObservations::new(
                code.view(),
                inputs.targets.as_ref(),
                &inputs.providers,
                inputs.layout_target.as_ref(),
            )
            .unwrap()
            .with_target_carriers((changed == "carrier").then(test_carriers))
            .unwrap();
            let error = observations
                .with_completed_scope(completion.admitted(), &inputs.address_model)
                .err()
                .unwrap_or_else(|| panic!("{changed}: changed scope must not reuse observations"));
            assert!(
                error.contains("execution identity")
                    || error.contains("authored target")
                    || error.contains("provider or target inputs"),
                "{changed}: {error}"
            );
        }
        let inputs = completion_inputs(&artifact, target);
        let restored = || {
            ScalarApplicationObservations::new(
                artifact.view(),
                inputs.targets.as_ref(),
                &inputs.providers,
                inputs.layout_target.as_ref(),
            )
            .unwrap()
            .with_completed_scope(completion.admitted(), &Ok(None))
            .unwrap()
        };
        assert!(
            restored()
                .with_target_carriers(Some(test_carriers()))
                .err()
                .unwrap()
                .contains("retained carrier layout")
        );
        let model = terminal_installation::ScalarAddressModel {
            target: target.into(),
            carrier: semantic_vocabulary::ClosedIntegerType::supported_address_representation(),
            semantics_version: 1,
            commitment: [1; 32],
        };
        let error = evaluate_scalar_applications_with_address_model(
            &mut restored(),
            target,
            &[application(1)],
            Some(&model),
            &mut WorkMeter::unbounded(),
        )
        .err()
        .unwrap();
        assert!(error.contains("retained address model"), "{error}");
        assert!(
            restored()
                .with_invocation_roots(&[])
                .err()
                .unwrap()
                .contains("cannot become an invocation")
        );
        assert_eq!(accounting.receipts().len(), 1);
    }

    #[test]
    fn product_conditions_refuse_before_any_later_native_demand() {
        let mut module = unscoped_constant(42, false).view().module().clone();
        module
            .scalar_applications
            .conditions
            .retain_completion(ScalarConditionSubject::Module, application(1));
        let artifact = std::sync::Arc::new(canonical(module));
        let mut accounting = crate::evaluation::SourceCompletionAccounting::default();
        let error =
            prepare_scalar_applications(&artifact, None, None, &[], None, &mut accounting, |_| {
                Ok(unscoped_inputs(&artifact))
            })
            .err()
            .expect("false Product condition refuses immediately");
        assert!(error.contains("predicate is false"), "{error}");
        assert_eq!(accounting.receipts().len(), 1);
    }

    fn test_carriers() -> semantic_vocabulary::TargetCarrierLayout {
        semantic_vocabulary::TargetCarrierLayout {
            pointer_byte_size: 8,
            pointer_byte_alignment: 8,
            descriptor_byte_size: 16,
            descriptor_byte_alignment: 8,
            case_tag_byte_size: 4,
            case_tag_byte_alignment: 4,
        }
    }

    #[test]
    fn unscoped_evaluation_rejects_deployment_inputs_before_recording_execution() {
        struct UnexpectedImports;
        impl TargetImportRealizations for UnexpectedImports {
            fn admit(
                &self,
                _: &[semantic_vocabulary::BoundaryMachineId],
            ) -> Result<Box<dyn AdmittedImportRealizations>, String> {
                panic!("unscoped evaluation must not consult target imports");
            }
        }
        let artifact = unscoped_constant(41, false);
        for input in ["carrier layout", "address model", "import realizations"] {
            let mut accounting = crate::evaluation::SourceCompletionAccounting::default();
            let error = complete_scalar_applications(
                &artifact,
                None,
                None,
                &[application(1)],
                None,
                &mut accounting,
                |_| {
                    let mut selected = unscoped_inputs(&artifact);
                    match input {
                        "carrier layout" => selected.carrier_layout = Some(test_carriers()),
                        "address model" => {
                            selected.address_model =
                                Ok(Some(terminal_installation::ScalarAddressModel {
                                    target: "macos_arm64".into(),
                                    carrier: semantic_vocabulary::ClosedIntegerType::supported_address_representation(),
                                    semantics_version: 1,
                                    commitment: [1; 32],
                                }));
                        }
                        _ => selected.import_realizations = Some(Box::new(UnexpectedImports)),
                    }
                    Ok(selected)
                },
            )
            .unwrap_err();
            assert!(error.contains(input), "{input}: {error}");
            assert!(accounting.receipts().is_empty(), "{input}");
        }
    }

    #[test]
    fn unscoped_scalar_conditions_execute_and_retain_targetless_receipts() {
        let legacy = compiler_storage::encode(&1_u32, Default::default()).unwrap();
        assert!(
            compiler_storage::decode::<crate::evaluation::PureEvaluationRecord>(&legacy, Default::default())
                .unwrap_err()
                .to_string()
                .contains("without explicit deployment scope")
        );
        for expected in [41, 42] {
            let artifact = unscoped_constant(expected, false);
            let mut accounting = crate::evaluation::SourceCompletionAccounting::default();
            let completed = complete_unscoped(&artifact, &mut accounting);
            if expected == 41 {
                let completed = completed.unwrap();
                assert_eq!(completed.values().value(application(1)), Some(integer(41)));
                assert!(completed.primary().targets().is_none());
                assert!(completed.primary().layouts().is_none());
                assert!(completed.primary().address_model().is_none());
            } else {
                assert!(completed.unwrap_err().contains("predicate is false"));
            }
            assert_eq!(
                accounting.receipts().len(),
                1,
                "failed conditions still account for the executed invocation"
            );
            let encoded = compiler_storage::encode(&accounting, Default::default()).unwrap();
            let restored: crate::evaluation::SourceCompletionAccounting =
                compiler_storage::decode(&encoded, Default::default()).unwrap();
            restored.validate_charges().unwrap();
            let crate::evaluation::PureEvaluationRecord::CanonicalScalar(receipt) = &restored.receipts()[0]
            else {
                panic!("scalar invocation retains its canonical receipt");
            };
            assert_eq!(receipt.target(), None);
            assert_eq!(
                receipt.artifact().view().identity(),
                artifact.view().identity()
            );
        }
    }

    #[test]
    fn unscoped_scalar_conditions_refuse_an_actual_address_model_demand() {
        let artifact = unscoped_constant(41, true);
        let error = complete_unscoped(&artifact, &mut Default::default()).unwrap_err();
        assert!(error.contains("address model"), "{error}");
    }

    #[test]
    fn unscoped_layout_observations_use_portable_geometry_not_host_pointer_size() {
        for pointer in [false, true] {
            let mut module = graph(24, false, false).view().module().clone();
            if pointer {
                module.structural_types[1].shape = StructuralTypeShape::Reference {
                    referent: type_id(1),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                };
            }
            let artifact = canonical(module);
            let providers = admit_provider_installation(artifact.view(), &[], &[]).unwrap();
            if pointer {
                let error =
                    ScalarApplicationObservations::new(artifact.view(), None, &providers, None)
                        .unwrap()
                        .with_target_carriers(Some(test_carriers()))
                        .err()
                        .unwrap();
                assert!(error.contains("carrier layout requires an explicit deployment"));
            }
            let mut observations =
                ScalarApplicationObservations::new(artifact.view(), None, &providers, None)
                    .unwrap();
            let outcome = evaluate_scalar_applications(
                &mut observations,
                None,
                &[application(5)],
                &mut WorkMeter::unbounded(),
            );
            if pointer {
                assert!(outcome.err().unwrap().contains("MissingCarrierLayout"));
            } else {
                outcome.unwrap();
                assert_eq!(
                    observations.value(application(5)).unwrap(),
                    Some(integer(24))
                );
            }
        }
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
        targets: &'a TerminalTargetContext,
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
            structural_places: Vec::new(),
            call_formations: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
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
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            contract: terminal_psi::MachineContract {
                selected_application: None,
                declaration: None,
                parameters: Vec::new(),
                parameter_order: Vec::new(),
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                content_entry_claims: Vec::new(),
                result: TerminalMachineResult::Scalar(result),
                declared_service_reach: None,
                closed_reach_application: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
                ..MachineContract::empty(ContractId::new(identity).unwrap())
            },
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
                declaration_identity: Default::default(),
                requires: Vec::new(),
                id: type_id(identity),
                identity: identities.intern(&format!("graph::type{identity}")),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                descriptor_cleanup: false,
                nominal_cleanup: None,
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
                        invocation_refusal: None,
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
        let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
        let windows = terminal_target_context(artifact.view(), "windows_x86_64");
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
                declaration_identity: Default::default(),
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
        let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
        let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
            let selections = terminal_target_context(artifact.view(), target);
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
            let targets = terminal_target_context(artifact.view(), "macos_arm64");
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
            let selections = terminal_target_context(artifact.view(), "macos_arm64");
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
        let selections = terminal_target_context(artifact.view(), "macos_arm64");
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
        let windows = terminal_target_context(artifact.view(), "windows_x86_64");
        let macos = terminal_target_context(artifact.view(), "macos_arm64");
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
