use super::*;

fn helper(ordinal: u64, places: &[u64]) -> TerminalMachine {
    let mut machine = unit_module().machines.remove(0);
    machine.id = machine_id(ordinal);
    machine.entry = block_id(ordinal);
    machine.blocks[0].id = machine.entry;
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(ordinal),
        trivial_affine_discards: Vec::new(),
    };
    machine.contract.id = contract_id(ordinal);
    for (position, place) in places.iter().enumerate() {
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: place_id(*place),
                position: position as u32,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(*place),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: position as u32,
                is_self: false,
            },
        });
    }
    machine
}

fn borrowed_arguments(places: &[u64]) -> Vec<StructuralArgument> {
    places
        .iter()
        .map(|place| StructuralArgument {
            place: place_id(*place),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        })
        .collect()
}

fn boundary(ordinal: u64, places: &[u64]) -> Operation {
    Operation {
        id: operation_id(ordinal),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: borrowed_arguments(places),
            completion_receipts: Vec::new(),
        },
    }
}

fn unit_call(ordinal: u64, callee: u64, places: &[u64]) -> Operation {
    Operation {
        id: operation_id(ordinal),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(callee),
            arguments: Vec::new(),
            structural_arguments: borrowed_arguments(places),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

fn nested_module() -> TerminalModule {
    let mut module = byte_sequence_literal_module(vec![0, 0x80, 0xff]);
    let mut second_parameter = module.boundary_machines[0].structural_parameters[0].clone();
    second_parameter.place = place_id(4);
    second_parameter.position = 1;
    module.boundary_machines[0]
        .structural_parameters
        .push(second_parameter);
    let root = &mut module.machines[0];
    root.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 1,
            structural_type: structural_type_id(1),
        },
    });
    root.blocks[0].operations.truncate(1);
    root.blocks[0].operations.extend([
        Operation {
            id: operation_id(3),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place_id(3),
                bytes: Vec::new(),
            },
        },
        unit_call(10, 2, &[1, 3]),
        unit_call(11, 2, &[3, 1]),
        unit_call(12, 2, &[1, 1]),
        boundary(13, &[1, 3]),
    ]);
    let mut outer = helper(2, &[5, 6]);
    outer.blocks[0].operations = vec![unit_call(20, 3, &[5, 6]), boundary(21, &[5, 6])];
    let mut inner = helper(3, &[7, 8]);
    inner.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(9),
        kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: structural_type_id(1),
        },
    });
    inner.blocks[0].operations = vec![
        Operation {
            id: operation_id(30),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place_id(9),
                bytes: vec![0x7f],
            },
        },
        boundary(31, &[7, 8]),
        boundary(32, &[9, 7]),
    ];
    module.machines.extend([outer, inner]);
    module
}

fn assert_byte_effects(module: &TerminalModule, expected: &[Vec<Vec<u8>>]) {
    let semantic = encode_module(module).expect("byte forwarding module encodes");
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution =
            TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
                .expect("byte forwarding module verifies");
        let mut handler = RecordingHandler::default();
        let mut meter = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        for _ in 0..256 {
            match execution
                .resume_with_effect_handler(&mut meter, &mut handler)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(incremental);
                    meter.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(_) => {
                    complete = true;
                    break;
                }
                status => panic!("unexpected byte forwarding status: {status:?}"),
            }
        }
        assert!(complete);
        let payloads = handler
            .effects
            .iter()
            .map(|effect| {
                let TerminalEffect::BoundaryCall {
                    structural_arguments,
                    byte_sequence_arguments,
                    ..
                } = effect
                else {
                    panic!("expected a byte boundary effect");
                };
                assert_eq!(structural_arguments.len(), byte_sequence_arguments.len());
                assert!(
                    structural_arguments
                        .iter()
                        .all(|value| value.path.is_empty())
                );
                byte_sequence_arguments
                    .iter()
                    .map(|bytes| bytes.clone().expect("exact payload, including empty"))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(payloads, expected);
        assert_eq!(execution.effects(), handler.effects);
        if let Some((effects, usage)) = &reference {
            assert_eq!(execution.effects(), effects);
            assert_eq!(meter.usage(), usage);
        } else {
            reference = Some((execution.effects().to_vec(), meter.usage().clone()));
        }
    }
}

fn expected_nested_bytes() -> Vec<Vec<Vec<u8>>> {
    let raw = vec![0, 0x80, 0xff];
    vec![
        vec![raw.clone(), vec![]],
        vec![vec![0x7f], raw.clone()],
        vec![raw.clone(), vec![]],
        vec![vec![], raw.clone()],
        vec![vec![0x7f], vec![]],
        vec![vec![], raw.clone()],
        vec![raw.clone(), raw.clone()],
        vec![vec![0x7f], raw.clone()],
        vec![raw.clone(), raw.clone()],
        vec![raw, vec![]],
    ]
}

#[test]
fn nested_repeated_unit_helpers_keep_each_invocations_exact_bytes() {
    assert_byte_effects(&nested_module(), &expected_nested_bytes());
}

#[test]
fn structural_scalar_helpers_forward_bytes_and_restore_the_caller() {
    let mut module = nested_module();
    let result = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(20),
        scalar_type: ScalarType::Boolean,
    };
    module.machines[1].blocks[0].operations[0] = Operation {
        id: operation_id(20),
        result: OperationResult::Scalar(result),
        kind: OperationKind::CallStructuralScalar {
            callee: machine_id(3),
            arguments: Vec::new(),
            structural_arguments: borrowed_arguments(&[5, 6]),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    let inner = &mut module.machines[2];
    inner.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(31),
        ..result
    });
    inner.blocks[0].operations.push(Operation {
        id: operation_id(33),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(30),
            ..result
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    inner.blocks[0].terminator = Terminator::Return {
        edge: edge_id(3),
        value: value_id(30),
        cleanup_actions: Vec::new(),
    };
    assert_byte_effects(&module, &expected_nested_bytes());
}

#[test]
fn unrelated_unit_scalar_and_structural_result_calls_preserve_caller_bytes() {
    let mut module = payloadless_call_module();
    let mut bytes = byte_sequence_literal_module(vec![0, 0xff]);
    let byte_type = structural_type_id(2);
    bytes.structural_types[0].id = byte_type;
    bytes.boundary_machines[0].structural_parameters[0].structural_type = byte_type;
    bytes.boundary_machines[0].structural_parameters[0].place = place_id(6);
    module.structural_types.extend(bytes.structural_types);
    module.boundary_machines = bytes.boundary_machines;
    let root = &mut module.machines[0];
    root.result = TerminalMachineResult::Unit;
    root.structural_places
        .retain(|place| place.id != place_id(4));
    root.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: Vec::new(),
    };
    root.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(5),
        kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: byte_type,
        },
    });
    root.blocks[0].operations.insert(
        0,
        Operation {
            id: operation_id(3),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place_id(5),
                bytes: vec![0, 0xff],
            },
        },
    );
    root.blocks[0].operations.splice(
        1..1,
        [
            unit_call(5, 3, &[]),
            Operation {
                id: operation_id(6),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(10),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::Call {
                    callee: machine_id(4),
                    arguments: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
            Operation {
                id: operation_id(7),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(11),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::CallStructuralScalar {
                    callee: machine_id(4),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        ],
    );
    root.blocks[0].operations.push(boundary(4, &[5]));
    let mut scalar = helper(4, &[]);
    scalar.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(41),
        scalar_type: ScalarType::Boolean,
    });
    scalar.blocks[0].operations = vec![Operation {
        id: operation_id(40),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(40),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    }];
    scalar.blocks[0].terminator = Terminator::Return {
        edge: edge_id(4),
        value: value_id(40),
        cleanup_actions: Vec::new(),
    };
    module.machines.extend([helper(3, &[]), scalar]);
    assert_byte_effects(&module, &[vec![vec![0, 0xff]]]);
}

#[test]
fn literal_byte_boundaries_reject_owned_and_mutable_access() {
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = byte_sequence_literal_module(vec![0, 0xff]);
        module.boundary_machines[0].structural_parameters[0].access = access;
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        structural_arguments[0].access = access;
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err()
        );
        assert!(encode_module(&module).is_err());
    }
}

#[test]
fn opaque_identity_does_not_supply_missing_incoming_byte_contents() {
    let mut module = byte_sequence_literal_module(vec![0xff]);
    let incoming = helper(2, &[3]);
    module.machines[0].structural_parameters = incoming.structural_parameters;
    module.machines[0]
        .structural_places
        .extend(incoming.structural_places);
    module.machines[0].blocks[0]
        .operations
        .push(boundary(3, &[3]));
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    for (path, expected) in [
        (
            Vec::new(),
            TerminalInterpretError::VerifiedStructuralPlaceMissing(place_id(3)),
        ),
        (
            vec![StructuralPathSegment::Field("bytes".into())],
            TerminalInterpretError::VerifiedOperationMalformed,
        ),
    ] {
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[TerminalStructuralValue {
                opaque_identity: place_id(1).get(),
                structural_type: structural_type_id(1),
                qualifications: Vec::new(),
                path,
            }],
        )
        .unwrap();
        let mut handler = RecordingHandler::default();
        assert_eq!(
            execution.resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut handler),
            Err(expected),
        );
        assert_eq!(
            handler.effects.len(),
            1,
            "missing contents must not reach the handler"
        );
    }
}
