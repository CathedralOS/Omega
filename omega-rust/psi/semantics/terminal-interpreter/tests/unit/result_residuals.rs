use super::*;
use semantic_vocabulary::StructuralPlaceKind;

fn field(name: &str) -> StructuralPathSegment {
    StructuralPathSegment::Field(name.into())
}

fn residual(path: Vec<StructuralPathSegment>, structural_type: u64) -> StructuralAffineDiscard {
    StructuralAffineDiscard {
        place: place_id(1),
        path,
        structural_type: structural_type_id(structural_type),
    }
}

fn produced_partial_module(ordinary: bool, nested: bool) -> TerminalModule {
    let mut module = partial_affine_field_module();
    if nested {
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "Row".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(1),
                length: 2,
            },
        });
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(4),
            identity: "Grid".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(3),
                length: 2,
            },
        });
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
            unreachable!()
        };
        fields[1].field_type = StructuralFieldType::Structural(structural_type_id(4));
        fields.push(StructuralFieldDeclaration {
            id: StructuralFieldId::new(3).unwrap(),
            identity: "tail".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(1)),
        });
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        structural_arguments[0].path.extend([
            StructuralPathSegment::FixedIndex(0),
            StructuralPathSegment::FixedIndex(1),
        ]);
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &mut module.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        *residual_affine_discards = vec![
            residual(vec![field("tail")], 1),
            residual(
                vec![field("right"), StructuralPathSegment::FixedIndex(1)],
                3,
            ),
            residual(
                vec![
                    field("right"),
                    StructuralPathSegment::FixedIndex(0),
                    StructuralPathSegment::FixedIndex(0),
                ],
                1,
            ),
            residual(vec![field("left")], 1),
        ];
    }
    let root_type = structural_type_id(2);
    let producer = if ordinary {
        let mut identity = module.machines[1].clone();
        identity.id = machine_id(3);
        identity.entry = block_id(3);
        identity.contract = empty_contract(contract_id(3));
        identity.structural_parameters[0].place = place_id(4);
        identity.structural_parameters[0].structural_type = root_type;
        identity.structural_places = vec![
            StructuralPlaceDeclaration {
                id: place_id(4),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: place_id(5),
                kind: StructuralPlaceKind::Result,
            },
        ];
        identity.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: place_id(5),
            structural_type: root_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
        identity.blocks[0].id = block_id(3);
        identity.blocks[0].terminator = Terminator::ReturnStructural {
            edge: edge_id(3),
            source: place_id(4),
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        module.machines.push(identity);
        let caller = &mut module.machines[0];
        caller.structural_parameters[0].place = place_id(3);
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(3),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
        OperationKind::CallStructuralWithScalarArguments {
            callee: machine_id(3),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(3),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    } else {
        module.machines[0].structural_parameters.clear();
        module.boundary_machines.push(BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "produce_pair".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Structural(
                terminal_psi::BoundaryStructuralResultDeclaration {
                    structural_type: root_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                },
            ),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        }
    };
    let caller = &mut module.machines[0];
    caller.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: operation_id(2),
        structural_type: root_type,
    };
    caller.blocks[0].operations.insert(
        0,
        Operation {
            id: operation_id(2),
            result: OperationResult::Structural(StructuralOperationResult {
                place: place_id(1),
                structural_type: root_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: producer,
        },
    );
    module
}

#[derive(Default)]
struct ProducePair {
    calls: usize,
}

impl TerminalEffectHandler for ProducePair {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("structural result required")
    }
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            structural_arguments,
            result: terminal_psi::BoundaryMachineResult::Structural(result),
            ..
        } = effect
        else {
            panic!("exact structural producer")
        };
        assert!(structural_arguments.is_empty());
        self.calls += 1;
        Ok(TerminalEffectResult::Structural(TerminalStructuralValue {
            opaque_identity: 50,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }))
    }
}

fn start(module: &TerminalModule, ordinary: bool) -> TerminalExecution {
    let semantic = encode_module(module).expect("call-result residual roots encode");
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = ProofBundle::default();
    verify_module(module, &proof, &AdmissionProfile::default())
        .expect("exact result residual complement independently verifies");
    let proof = encode_proof_bundle(&proof).unwrap();
    let arguments = if ordinary {
        vec![TerminalStructuralValue {
            opaque_identity: 50,
            structural_type: structural_type_id(2),
            qualifications: Vec::new(),
            path: Vec::new(),
        }]
    } else {
        Vec::new()
    };
    TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .unwrap()
}

#[test]
fn result_residuals_preserve_maximal_subtrees_and_charge_cleanup_only_at_return() {
    for ordinary in [false, true] {
        for nested in [false, true] {
            let module = produced_partial_module(ordinary, nested);
            let Terminator::ReturnUnitPartialAffine {
                residual_affine_discards,
                ..
            } = &module.machines[0].blocks[0].terminator
            else {
                unreachable!()
            };
            let expected = residual_affine_discards
                .iter()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>();
            let mut reference_effects = None;
            let mut reference_usage = None;
            for incremental in [false, true] {
                let mut execution = start(&module, ordinary);
                let mut provider = ProducePair::default();
                let mut meter = if incremental {
                    TerminalFuelMeter::with_allowance(0)
                } else {
                    TerminalFuelMeter::unbounded()
                };
                let mut complete = false;
                let mut paused_at_return = false;
                for _ in 0..64 {
                    match execution
                        .resume_with_effect_handler(&mut meter, &mut provider)
                        .unwrap()
                    {
                        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                            assert!(incremental);
                            if exhaustion.site == FuelChargeSite::Edge(edge_id(1)) {
                                assert_eq!(
                                    execution
                                        .live_affine_frontier()
                                        .cloned()
                                        .collect::<std::collections::BTreeSet<_>>(),
                                    expected
                                );
                                assert!(
                                    meter.usage().at(FuelChargeSite::Edge(edge_id(1))).is_none()
                                );
                                paused_at_return = true;
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
                assert_eq!(paused_at_return, incremental);
                assert_eq!(provider.calls, usize::from(!ordinary));
                assert!(execution.live_affine_frontier().next().is_none());
                assert_eq!(meter.usage().total_units(), if ordinary { 5 } else { 4 });
                if let Some(reference) = &reference_effects {
                    assert_eq!(execution.effects(), reference);
                } else {
                    reference_effects = Some(execution.effects().to_vec());
                }
                if let Some(reference) = &reference_usage {
                    assert_eq!(meter.usage(), reference);
                } else {
                    reference_usage = Some(meter.usage().clone());
                }
            }
        }
    }
}

#[test]
fn fully_transferred_result_uses_the_existing_empty_unit_return() {
    for ordinary in [false, true] {
        let mut module = produced_partial_module(ordinary, false);
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
        let mut execution = start(&module, ordinary);
        let mut provider = ProducePair::default();
        let mut meter = TerminalFuelMeter::with_allowance(if ordinary { 6 } else { 5 });
        assert!(
            matches!(execution.resume_with_effect_handler(&mut meter, &mut provider).unwrap(), TerminalExecutionStatus::SponsorExhausted(exhaustion) if exhaustion.site == FuelChargeSite::Edge(edge_id(1)))
        );
        assert!(execution.live_affine_frontier().next().is_none());
        meter.replenish(1).unwrap();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut meter, &mut provider)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(provider.calls, usize::from(!ordinary));
    }
}

#[test]
fn result_partial_move_crash_has_no_residual_cleanup_successor() {
    for ordinary in [false, true] {
        let mut module = produced_partial_module(ordinary, true);
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &module.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        let expected = residual_affine_discards
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        module.machines[0].contract.crash_routes = vec![CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![CrashRouteGuard::Truth],
        }];
        module.machines[0].blocks[0].terminator = Terminator::Crash {
            edge: edge_id(1),
            cause: CrashCause::Abort,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        };
        let mut execution = start(&module, ordinary);
        let mut provider = ProducePair::default();
        let mut meter = TerminalFuelMeter::unbounded();
        let status = execution
            .resume_with_effect_handler(&mut meter, &mut provider)
            .unwrap();
        assert!(
            matches!(&status, TerminalExecutionStatus::Crashed(crash) if crash.cause == CrashCause::Abort)
        );
        assert_eq!(
            execution
                .live_affine_frontier()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        let effects = execution.effects().to_vec();
        let usage = meter.usage().clone();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut meter, &mut provider)
                .unwrap(),
            status
        );
        assert_eq!(execution.effects(), effects);
        assert_eq!(meter.usage(), &usage);
        assert_eq!(provider.calls, usize::from(!ordinary));
    }
}

#[test]
fn codec_rejoins_partial_result_root_with_its_exact_call_producer() {
    let original = produced_partial_module(false, false);
    encode_module(&original).expect("valid control encodes before mutations");
    for mutation in 0..4 {
        let mut changed = original.clone();
        if mutation == 0 {
            let StructuralPlaceKind::OperationResult { producer, .. } =
                &mut changed.machines[0].structural_places[0].kind
            else {
                unreachable!()
            };
            *producer = operation_id(1);
        } else if mutation == 1 {
            changed.machines[0].structural_places[0].kind =
                StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal: 0,
                    structural_type: structural_type_id(2),
                    construction: None,
                };
        } else if mutation == 2 {
            let OperationResult::Structural(result) =
                &mut changed.machines[0].blocks[0].operations[0].result
            else {
                unreachable!()
            };
            result.qualifications.push(structural_domain_id(9));
        } else {
            let Terminator::ReturnUnitPartialAffine {
                residual_affine_discards,
                ..
            } = &mut changed.machines[0].blocks[0].terminator
            else {
                unreachable!()
            };
            residual_affine_discards[0].structural_type = structural_type_id(2);
        }
        assert!(encode_module(&changed).is_err(), "mutation {mutation}");
    }
}
