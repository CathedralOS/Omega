use super::*;

fn result_consumer_module() -> TerminalModule {
    let mut module = identity_call_module(&[]);
    let mut consumer = module.machines[1].clone();
    consumer.id = machine_id(3);
    consumer.contract.id = contract_id(3);
    consumer.entry = block_id(3);
    consumer.blocks[0].id = block_id(3);
    consumer.result = TerminalMachineResult::Unit;
    consumer.structural_parameters[0].place = place_id(6);
    consumer.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(6),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    consumer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: vec![place_id(6)],
    };
    module.machines.push(consumer);
    let caller = &mut module.machines[0];
    let mut retained = caller.structural_parameters[0].clone();
    retained.place = place_id(7);
    retained.position = 1;
    caller.structural_parameters.push(retained);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(7),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    caller.blocks[0].operations.push(Operation {
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(3),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(3),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: vec![place_id(7)],
    };
    module
}

#[test]
fn unit_calls_consume_whole_affine_results_and_resume_through_both_cleanups() {
    for shape in 0..3 {
        let mut module = result_consumer_module();
        if shape != 0 {
            nested_shape(&mut module, shape == 2);
        }
        let semantic = encode_module(&module).unwrap();
        let decoded = decode_module(&semantic).unwrap();
        assert_eq!(encode_module(&decoded).unwrap(), semantic);
        let bundle = ProofBundle::default();
        let verified = verify_module(&decoded, &bundle, &AdmissionProfile::default()).unwrap();
        let certificate =
            terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, decoded.entry).unwrap();
        terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate).unwrap();
        assert_eq!(certificate.ceiling_units(), 5);
        let proof = encode_proof_bundle(&bundle).unwrap();
        let inputs = [0xaff1, 0xaff2].map(|opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        });
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &inputs,
        )
        .unwrap();
        let sites = [
            (
                FuelChargeSite::Operation(operation_id(1)),
                vec![place_id(1), place_id(7)],
            ),
            (FuelChargeSite::Edge(edge_id(2)), vec![place_id(4)]),
            (
                FuelChargeSite::Operation(operation_id(2)),
                vec![place_id(3), place_id(7)],
            ),
            (FuelChargeSite::Edge(edge_id(3)), vec![place_id(6)]),
            (FuelChargeSite::Edge(edge_id(1)), vec![place_id(7)]),
        ];
        let mut meter = TerminalFuelMeter::with_allowance(0);
        for (units, (site, places)) in sites.iter().enumerate() {
            for _ in 0..2 {
                assert!(matches!(execution.resume(&mut meter).unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(exhaustion) if exhaustion.site == *site));
                assert_eq!(meter.usage().total_units(), units as u64);
                assert!(execution.live_claim_frontier().next().is_none());
                assert_eq!(
                    execution
                        .live_affine_frontier()
                        .cloned()
                        .collect::<Vec<_>>(),
                    places.iter().copied().map(discard).collect::<Vec<_>>()
                );
            }
            meter.replenish(1).unwrap();
        }
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert!(execution.live_affine_frontier().next().is_none());
        assert!(execution.live_claim_frontier().next().is_none());
        assert_eq!(meter.usage().total_units(), 5);
        for (site, _) in sites {
            assert_eq!(meter.usage().at(site).unwrap().units(), 1);
        }
    }
}

#[test]
fn unit_result_uses_reject_dead_results_forged_producers_and_cleanup_mismatches() {
    let base = result_consumer_module();
    verify(&base).unwrap();
    for mutation in 0..9 {
        let mut module = base.clone();
        let caller = &mut module.machines[0];
        match mutation {
            0 => caller.blocks[0].operations.swap(0, 1),
            1 => {
                let mut repeated = caller.blocks[0].operations[1].clone();
                repeated.id = operation_id(3);
                caller.blocks[0].operations.push(repeated);
            }
            2 => {
                caller.structural_places[1].kind = StructuralPlaceKind::OperationResult {
                    producer: operation_id(2),
                    structural_type: structural_type_id(1),
                }
            }
            3 => caller
                .structural_places
                .retain(|place| place.id != place_id(3)),
            4 => {
                caller.blocks[0].terminator = Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: vec![place_id(3), place_id(7)],
                }
            }
            5 => {
                caller.blocks[0].operations.pop();
            }
            6 => {
                caller.blocks[0].terminator = Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                }
            }
            7 => {
                module.machines[2].blocks[0].terminator = Terminator::ReturnUnit {
                    edge: edge_id(3),
                    trivial_affine_discards: Vec::new(),
                }
            }
            8 => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[0].operations[1].kind
                else {
                    unreachable!()
                };
                structural_arguments[0].place = place_id(1);
            }
            _ => unreachable!(),
        }
        assert!(
            verify(&module).is_err(),
            "result custody mutation {mutation} must reject"
        );
    }
}

#[test]
fn unit_result_uses_reject_projection_borrow_and_linear_custody() {
    let base = result_consumer_module();
    verify(&base).unwrap();
    for mutation in 0..4 {
        let mut module = base.clone();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => structural_arguments[0]
                .path
                .push(StructuralPathSegment::Field("field1".into())),
            1 => {
                structural_arguments[0].access = StructuralAccess::SharedBorrow;
                module.machines[2].structural_parameters[0].access = StructuralAccess::SharedBorrow;
            }
            2 => {
                module.machines[2].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Linear
            }
            3 => {
                let mut different = module.structural_types[0].clone();
                different.id = structural_type_id(2);
                different.identity = "test::Different".into();
                module.structural_types.push(different);
                module.machines[2].structural_parameters[0].structural_type = structural_type_id(2);
            }
            _ => unreachable!(),
        }
        assert!(
            verify(&module).is_err(),
            "result shape mutation {mutation} must reject"
        );
    }
}
