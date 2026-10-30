//! Complete scope coverage and execution over canonical graph inputs.
use super::*;
use terminal_installation::{ScalarAddressModel, ScalarApplicationClosure};

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
            "unchecked entry".into(),
            nodes,
            root,
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
            little_endian: true,
        },
    )
    .unwrap();
    let conditions = evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
        &[application(5)],
        &mut TerminalFuelMeter::unbounded(),
    )
    .unwrap();
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
            little_endian: true,
        },
    )
    .unwrap();
    let conditions = evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
        &[application(7)],
        &mut TerminalFuelMeter::unbounded(),
    )
    .unwrap();
    assert_eq!(
        observations.value(application(6)).unwrap(),
        Some(integer(41))
    );
    assert_eq!(
        observations.value(application(7)).unwrap(),
        Some(integer(41))
    );
    let mut closure = ScalarApplicationClosure::new(artifact.view(), "macos_arm64");
    assert!(
        closure
            .close_scope(observations, conditions, Ok(None))
            .unwrap_err()
            .contains("predicate is false")
    );
}

#[test]
fn scalar_closure_derives_supplemental_scope_from_shared_condition_owner() {
    let mut module = scoped_graph(41).view().module().clone();
    module.scalar_applications.conditions = Default::default();
    let scope = &mut module.scalar_applications.proof_scope;
    scope.plan.has_obligations = false;
    scope.plan.obligation_owners = Default::default();
    scope
        .declarations
        .insert(terminal_psi::ProofScopeDeclaration::Unavailable(
            "shared helper".into(),
        ));
    scope.plan.machines.insert(
        semantic_vocabulary::proof_applications::ProofApplicationMachine {
            machine: terminal_psi::ProofDeclarationId(3),
            product_selected: true,
            ..Default::default()
        },
    );
    for caller in [1, 2] {
        let calls = scope.calls.calls.insert_many([
            semantic_vocabulary::proof_applications::ProofApplicationCall {
                callee: terminal_psi::ProofDeclarationId(3),
                ..Default::default()
            },
        ]);
        scope
            .calls
            .machines
            .insert(semantic_vocabulary::proof_applications::ProofCallMachine {
                machine: terminal_psi::ProofDeclarationId(caller),
                calls,
                ..Default::default()
            });
    }
    module
        .scalar_applications
        .conditions
        .retain_range(
            ScalarConditionSubject::Declaration(terminal_psi::ProofDeclarationId(3)),
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
    entry
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
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
        });
    module.machines.push(entry);
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
            little_endian: true,
        },
    )
    .unwrap();
    let conditions = evaluate_scalar_applications(
        &mut observations,
        "macos_arm64",
        &[application(5)],
        &mut TerminalFuelMeter::unbounded(),
    )
    .unwrap();
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
            entry: Some(TargetMachineCallable::Machine(machine_id(3))),
            installation: Some(&providers),
            target_machines: Some(&targets),
            target_layouts: Some(admission.primary().layouts()),
            host_imports: &[],
            x86_flags: None,
            memory_ordering: None,
        })
        .unwrap();
    let arguments = [terminal_interpreter::TerminalStructuralValue::minted(
        1,
        type_id(3),
        Vec::new(),
    )];
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
                .invoke(&mut TerminalFuelMeter::unbounded())
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
                little_endian: true,
            },
        )
        .unwrap();
        let conditions = evaluate_scalar_applications(
            &mut observations,
            target,
            &[application(5)],
            &mut TerminalFuelMeter::unbounded(),
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
        let other_selections = admit_target_machine_selections(artifact.view(), other_target);
        let other_layouts =
            terminal_installation::admit_target_layouts(artifact.view(), &other_selections)
                .unwrap();
        let program = TerminalProgram::borrow(artifact.view());
        let other_installation = program
            .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
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
            let result = instance
                .invoke(&mut TerminalFuelMeter::unbounded())
                .unwrap();
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
    for (ordinal, target) in [(1, "macos_arm64"), (2, "windows_x86_64")] {
        scope
            .declarations
            .insert(terminal_psi::ProofScopeDeclaration::Unavailable(format!(
                "{target} owner"
            )));
        scope.plan.machines.insert(
            semantic_vocabulary::proof_applications::ProofApplicationMachine {
                machine: terminal_psi::ProofDeclarationId(ordinal),
                authored_target: Some(target.into()),
                source_target: Some(target.into()),
                product_selected: true,
                ..Default::default()
            },
        );
    }
    scope.plan.obligation_owners = scope.plan.symbols.insert_many([
        terminal_psi::ProofDeclarationId(1),
        terminal_psi::ProofDeclarationId(2),
    ]);
    for (owner, value) in [(1, 41), (2, windows_value)] {
        module
            .scalar_applications
            .conditions
            .retain_range(
                ScalarConditionSubject::Declaration(terminal_psi::ProofDeclarationId(owner)),
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
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &macos,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: macos.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
            },
        )
        .unwrap();
        let conditions = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(5)],
            &mut TerminalFuelMeter::unbounded(),
        )
        .unwrap();
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
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &windows,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: windows.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
            },
        )
        .unwrap();
        let conditions = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(2)],
            &mut TerminalFuelMeter::unbounded(),
        )
        .unwrap();
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
                little_endian: true,
            },
        )
        .unwrap();
        let conditions = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(5)],
            &mut TerminalFuelMeter::unbounded(),
        )
        .unwrap();
        let conditions = if omit_entry_debt {
            observations.conditions("macos_arm64", &[]).unwrap()
        } else {
            conditions
        };
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
        let mut observations = ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &terminal_installation::LayoutTargetFacts {
                profile: targets.target().to_owned(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
            },
        )
        .unwrap();
        let conditions = evaluate_scalar_applications(
            &mut observations,
            "macos_arm64",
            &[application(5)],
            &mut TerminalFuelMeter::unbounded(),
        )
        .unwrap();
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
