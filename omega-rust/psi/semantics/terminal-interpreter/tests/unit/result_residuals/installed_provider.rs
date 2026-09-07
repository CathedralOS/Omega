use super::*;
use terminal_interpreter::{
    ProviderInstallationSelection, admit_provider_installation_from_artifact,
};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
};

fn module(nested: bool) -> TerminalModule {
    let mut module = produced_partial_module(true, nested);
    let attachment = structural_type_id(10);
    module.structural_types.push(StructuralTypeDeclaration {
        id: attachment,
        identity: "test::Provider".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.machines[2].attachment = Some(attachment);
    let parameter = module.machines[2].structural_parameters[0].clone();
    let mut boundary_parameter = parameter.clone();
    boundary_parameter.place = place_id(6);
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(1),
        identity: "test::forward".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![boundary_parameter],
        result: BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type: structural_type_id(2),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        }),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module
        .provider_candidates
        .push(ProviderCandidateConformance {
            boundary: boundary_id(1),
            requirement_identity: "test::forward".into(),
            provider_identity: "test::Provider".into(),
            candidate_identity: "test::Provider::forward".into(),
            candidate: machine_id(3),
            signature: ProviderSignature {
                parameters: vec![ProviderSignatureParameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    qualifications: parameter.qualifications,
                    projected_qualifications: parameter.projected_qualifications,
                }],
            },
            refinement: ProviderRefinement {
                positional_parameters: vec![ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                }],
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    module.machines[0].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
        boundary: boundary_id(1),
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: place_id(3),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        completion_receipts: Vec::new(),
    };
    module
}

fn arguments() -> [TerminalStructuralValue; 1] {
    [TerminalStructuralValue {
        opaque_identity: 50,
        structural_type: structural_type_id(2),
        qualifications: Vec::new(),
        path: Vec::new(),
    }]
}

fn selection() -> ProviderInstallationSelection {
    ProviderInstallationSelection {
        boundary: boundary_id(1),
        provider_identity: "test::Provider".into(),
        candidate: machine_id(3),
    }
}

fn installed_start(module: &TerminalModule) -> TerminalExecution {
    installed_start_with_arguments(module, &arguments())
}

fn installed_start_with_arguments(
    module: &TerminalModule,
    structural_arguments: &[TerminalStructuralValue],
) -> TerminalExecution {
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let profile = AdmissionProfile::default();
    let installation =
        admit_provider_installation_from_artifact(&semantic, &proof, &profile, &[selection()])
            .expect("verified structural-result provider installation");
    TerminalExecution::start_artifact_with_provider_installation(
        &semantic,
        &proof,
        &profile,
        &[],
        structural_arguments,
        &installation,
    )
    .unwrap()
}

fn consume_remaining_left(module: &mut TerminalModule) {
    let mut second = module.machines[0].blocks[0].operations[1].clone();
    second.id = operation_id(3);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut second.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![field("left")];
    module.machines[0].blocks[0].operations.push(second);
    module.machines[0].blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: Vec::new(),
    };
}

#[test]
fn installed_structural_provider_preserves_result_residuals_across_fuel_suspension() {
    for (nested, full) in [(false, false), (true, false), (false, true)] {
        let mut expected_effects = None;
        let mut expected_usage = None;
        for installed in [false, true] {
            let mut module = if installed {
                module(nested)
            } else {
                produced_partial_module(true, nested)
            };
            if full {
                consume_remaining_left(&mut module);
            }
            for incremental in [false, true] {
                let mut execution = if installed {
                    installed_start(&module)
                } else {
                    start(&module, true)
                };
                let mut meter = if incremental {
                    TerminalFuelMeter::with_allowance(0)
                } else {
                    TerminalFuelMeter::unbounded()
                };
                let mut host = ProducePair::default();
                let mut complete = false;
                let mut before_return = false;
                let mut after_return = false;
                for _ in 0..32 {
                    match execution
                        .resume_with_effect_handler(&mut meter, &mut host)
                        .unwrap()
                    {
                        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                            assert!(incremental);
                            if exhaustion.site == FuelChargeSite::Edge(edge_id(3)) {
                                // The provider still owns the input. The caller result
                                // must not be installed before this return is charged.
                                assert_eq!(
                                    execution
                                        .live_affine_frontier()
                                        .map(|entry| entry.place)
                                        .collect::<Vec<_>>(),
                                    [place_id(4)]
                                );
                                assert!(
                                    meter.usage().at(FuelChargeSite::Edge(edge_id(3))).is_none()
                                );
                                before_return = true;
                            }
                            if exhaustion.site == FuelChargeSite::Operation(operation_id(1)) {
                                assert_eq!(
                                    execution
                                        .live_affine_frontier()
                                        .map(|entry| entry.place)
                                        .collect::<Vec<_>>(),
                                    [place_id(1)]
                                );
                                after_return = true;
                            }
                            meter.replenish(1).unwrap();
                        }
                        TerminalExecutionStatus::Complete(result) => {
                            assert_eq!(result, TerminalExecutionResult::Unit);
                            complete = true;
                            break;
                        }
                        status => panic!("unexpected {status:?}"),
                    }
                }
                assert!(complete);
                assert_eq!(before_return, incremental);
                assert_eq!(after_return, incremental);
                assert_eq!(meter.usage().total_units(), if full { 7 } else { 5 });
                assert_eq!(host.calls, 0);
                assert!(execution.live_affine_frontier().next().is_none());
                if let Some(reference) = &expected_effects {
                    assert_eq!(execution.effects(), reference);
                } else {
                    expected_effects = Some(execution.effects().to_vec());
                }
                if let Some(reference) = &expected_usage {
                    assert_eq!(meter.usage(), reference);
                } else {
                    expected_usage = Some(meter.usage().clone());
                }
            }
        }
    }
}

#[test]
fn installed_structural_provider_preserves_identity_into_a_projected_boundary_effect() {
    let mut module = module(false);
    let parameter = module.machines[1].structural_parameters[0].clone();
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(2),
        identity: "test::observe_leaf".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![parameter.clone()],
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(2),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: parameter.place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            completion_receipts: Vec::new(),
        },
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[1].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.clear();

    #[derive(Default)]
    struct ObserveLeaf {
        calls: usize,
    }
    impl TerminalEffectHandler for ObserveLeaf {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                boundary,
                structural_arguments,
                completion_receipts,
                result,
                ..
            } = effect
            else {
                panic!("leaf observation");
            };
            assert_eq!(*boundary, boundary_id(2));
            assert_eq!(*result, BoundaryMachineResult::Unit);
            assert!(completion_receipts.is_empty());
            assert_eq!(
                structural_arguments,
                &[TerminalStructuralValue {
                    opaque_identity: 50,
                    structural_type: structural_type_id(1),
                    qualifications: Vec::new(),
                    path: vec![field("right")],
                }]
            );
            self.calls += 1;
            Ok(())
        }
    }

    let mut expected_effects = None;
    for incremental in [false, true] {
        let mut execution = installed_start(&module);
        let mut meter = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut host = ObserveLeaf::default();
        loop {
            match execution
                .resume_with_effect_handler(&mut meter, &mut host)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(incremental);
                    meter.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    break;
                }
                status => panic!("unexpected {status:?}"),
            }
        }
        assert_eq!(host.calls, 1);
        assert_eq!(meter.usage().total_units(), 6);
        assert!(execution.live_affine_frontier().next().is_none());
        if let Some(expected) = &expected_effects {
            assert_eq!(execution.effects(), expected);
        } else {
            expected_effects = Some(execution.effects().to_vec());
        }
    }
}

#[test]
fn installed_structural_provider_rejects_missing_foreign_or_drifted_custody() {
    let module = module(false);
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let profile = AdmissionProfile::default();
    let installation =
        admit_provider_installation_from_artifact(&semantic, &proof, &profile, &[selection()])
            .unwrap();
    let mut missing = start(&module, true);
    assert!(matches!(
        missing.resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut ProducePair::default()),
        Err(TerminalInterpretError::ProviderInstallationMissing(boundary)) if boundary == boundary_id(1)
    ));
    assert_eq!(
        missing
            .live_affine_frontier()
            .map(|entry| entry.place)
            .collect::<Vec<_>>(),
        [place_id(3)]
    );
    assert!(missing.effects().is_empty());

    let mut wrong_selection = selection();
    wrong_selection.provider_identity = "test::Foreign".into();
    assert!(
        admit_provider_installation_from_artifact(&semantic, &proof, &profile, &[wrong_selection],)
            .is_err()
    );

    let mut foreign = module.clone();
    foreign.provider_candidates[0].provider_identity = "test::Foreign".into();
    let foreign_semantic = encode_module(&foreign).unwrap();
    assert!(matches!(
        TerminalExecution::start_artifact_with_provider_installation(
            &foreign_semantic,
            &proof,
            &profile,
            &[],
            &arguments(),
            &installation,
        ),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::ProviderInstallationIdentityMismatch
            )
        )
    ));
    for drift in 0..3 {
        let mut malformed = module.clone();
        let mut result = malformed.machines[2].result.structural().unwrap().clone();
        match drift {
            0 => result.structural_type = structural_type_id(1),
            1 => result.multiplicity = StructuralMultiplicity::Unrestricted,
            2 => malformed.provider_candidates[0].candidate = machine_id(2),
            _ => unreachable!(),
        }
        malformed.machines[2].result = TerminalMachineResult::Structural(result);
        assert!(verify_module(&malformed, &ProofBundle::default(), &profile).is_err());
    }
}
