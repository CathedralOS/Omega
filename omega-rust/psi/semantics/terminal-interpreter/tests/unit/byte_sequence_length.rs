use super::*;

fn byte_count_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn length_operation(ordinal: u64, source: u64) -> Operation {
    Operation {
        id: operation_id(ordinal),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(ordinal),
            scalar_type: byte_count_type(),
        }),
        kind: OperationKind::ByteSequenceLength {
            source: place_id(source),
        },
    }
}

fn return_length(machine: &mut TerminalMachine, result: u64, edge: u64) {
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(result + 100),
        scalar_type: byte_count_type(),
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(edge),
        value: value_id(result),
        cleanup_actions: Vec::new(),
    };
}

fn literal_module(bytes: Vec<u8>) -> TerminalModule {
    let mut module = byte_sequence_literal_module(bytes);
    module.boundary_machines.clear();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.truncate(1);
    machine.blocks[0].operations.push(length_operation(2, 1));
    return_length(machine, 2, 1);
    module
}

fn parameter_module() -> TerminalModule {
    let mut module = literal_module(Vec::new());
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.remove(0);
    machine.structural_places[0].kind = semantic_vocabulary::StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    module
}

fn scalar_count(count: u64) -> TerminalExecutionResult {
    TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(u128::from(count)),
    })
}

#[test]
fn literal_lengths_are_exact_u64_and_meter_once_across_resume() {
    for bytes in [Vec::new(), vec![0xff], vec![0, 0x80, 0xff], vec![7; 257]] {
        let expected = scalar_count(bytes.len() as u64);
        let module = literal_module(bytes);
        let semantic = encode_module(&module).unwrap();
        assert_eq!(decode_module(&semantic).unwrap(), module);
        let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        assert_eq!(measured.value(), expected);
        assert_eq!(measured.usage().total_units(), 3);
        assert_eq!(
            measured
                .usage()
                .at(FuelChargeSite::Operation(operation_id(2)))
                .unwrap()
                .units(),
            1
        );
        let mut execution =
            TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
                .unwrap();
        let mut meter = TerminalFuelMeter::with_allowance(0);
        loop {
            match execution.resume(&mut meter).unwrap() {
                TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, expected);
                    break;
                }
                status => panic!("unexpected length status: {status:?}"),
            }
        }
        assert_eq!(meter.usage(), measured.usage());
        assert!(execution.effects().is_empty());
    }
}

#[test]
fn byte_sequence_length_artifact_rejects_stale_vocabulary() {
    let semantic = encode_module(&literal_module(vec![0xff])).unwrap();
    assert_eq!(&semantic[10..12], &90_u16.to_le_bytes());
    for generation in [88_u16, 89, 91] {
        let mut stale = semantic.clone();
        stale[10..12].copy_from_slice(&generation.to_le_bytes());
        assert!(decode_module(&stale).is_err());
    }
}

#[test]
fn verifier_rejects_wrong_length_result_and_unknown_or_unestablished_source() {
    let base = literal_module(vec![0xff]);
    for scalar_type in [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
    ] {
        let mut module = base.clone();
        module.machines[0].blocks[0].operations[1].result =
            OperationResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type,
            });
        assert!(matches!(
            terminal_verifier::validate_module_representation(&module),
            Err(ModuleError::ByteSequenceLengthRequiresU64Result(_))
        ));
        assert!(encode_module(&module).is_err());
    }
    let mut unit_result = base.clone();
    unit_result.machines[0].blocks[0].operations[1].result = OperationResult::Unit;
    assert!(terminal_verifier::validate_module_representation(&unit_result).is_err());
    for source in [2, 999] {
        let mut module = base.clone();
        module.machines[0].blocks[0].operations[1].kind = OperationKind::ByteSequenceLength {
            source: place_id(source),
        };
        assert!(matches!(
            terminal_verifier::validate_module_representation(&module),
            Err(ModuleError::InvalidByteSequenceLengthSource { .. })
        ));
        assert!(encode_module(&module).is_err());
    }
    let mut early = base;
    early.machines[0].blocks[0].operations.swap(0, 1);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&early),
        Err(ModuleError::ByteSequenceViewNotEstablished { .. })
    ));
    assert!(encode_module(&early).is_err());
}

#[test]
fn parameter_length_requires_whole_readable_borrowed_view_and_real_contents() {
    let base = parameter_module();
    terminal_verifier::validate_module_representation(&base).unwrap();
    for access in [StructuralAccess::Owned, StructuralAccess::WriteOnlyBorrow] {
        let mut module = base.clone();
        module.machines[0].structural_parameters[0].access = access;
        assert!(terminal_verifier::validate_module_representation(&module).is_err());
        assert!(encode_module(&module).is_err());
    }
    for shape in [
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
        StructuralTypeShape::Record { fields: Vec::new() },
    ] {
        let mut module = base.clone();
        module.structural_types[0].shape = shape;
        assert!(terminal_verifier::validate_module_representation(&module).is_err());
        assert!(encode_module(&module).is_err());
    }
    // Mutable access admits metadata, not immutable read/subslice semantics or
    // an opaque host identity standing in for an initialized mutable binding.
    let mut mutable = base.clone();
    mutable.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    terminal_verifier::validate_module_representation(&mutable).unwrap();
    let mutable_bytes = encode_module(&mutable).unwrap();
    assert_eq!(decode_module(&mutable_bytes).unwrap(), mutable);
    let semantic = encode_module(&base).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 1,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .unwrap();
    assert_eq!(
        execution.resume(&mut TerminalFuelMeter::unbounded()),
        Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
            place_id(1)
        ))
    );
}

#[test]
fn nested_repeated_calls_measure_invocation_bytes_and_restore_caller() {
    let mut module = literal_module(vec![0, 0x80, 0xff]);
    let mut boundary = byte_sequence_literal_module(Vec::new())
        .boundary_machines
        .remove(0);
    boundary.structural_parameters.clear();
    boundary.scalar_parameters = vec![byte_count_type()];
    module.boundary_machines.push(boundary);
    let root = &mut module.machines[0];
    root.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(5),
        kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 1,
            structural_type: structural_type_id(1),
        },
    });
    root.blocks[0].operations.truncate(1);
    root.blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination: place_id(5),
            bytes: Vec::new(),
        },
    });
    for (ordinal, source) in [(4, 1), (6, 5), (8, 1)] {
        let mut call = length_operation(ordinal, source);
        call.result = OperationResult::Unit;
        call.kind = OperationKind::CallUnit {
            callee: machine_id(2),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(source),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        root.blocks[0].operations.push(call);
    }
    root.blocks[0].operations.push(length_operation(2, 1));
    for ordinal in [2, 3] {
        let mut helper = parameter_module().machines.remove(0);
        helper.id = machine_id(ordinal);
        helper.entry = block_id(ordinal);
        helper.blocks[0].id = block_id(ordinal);
        helper.contract.id = contract_id(ordinal);
        helper.structural_parameters[0].place = place_id(ordinal + 1);
        helper.structural_places[0].id = place_id(ordinal + 1);
        helper.blocks[0].operations = vec![length_operation(ordinal * 10, ordinal + 1)];
        if ordinal == 2 {
            helper.blocks[0].operations[0].kind = OperationKind::CallStructuralScalar {
                callee: machine_id(3),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(3),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            };
        }
        return_length(&mut helper, ordinal * 10, ordinal);
        if ordinal == 2 {
            helper.result = TerminalMachineResult::Unit;
            helper.blocks[0].operations.push(Operation {
                id: operation_id(21),
                result: OperationResult::Unit,
                kind: OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: vec![value_id(20)],
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            });
            helper.blocks[0].terminator = Terminator::ReturnUnit {
                edge: edge_id(2),
                trivial_affine_discards: Vec::new(),
            };
        }
        module.machines.push(helper);
    }
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution =
            TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
                .unwrap();
        let mut meter = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut handler = RecordingHandler::default();
        loop {
            match execution
                .resume_with_effect_handler(&mut meter, &mut handler)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, scalar_count(3));
                    break;
                }
                status => panic!("unexpected nested length status: {status:?}"),
            }
        }
        let counts = handler
            .effects
            .iter()
            .map(|effect| {
                let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                    panic!("expected boundary");
                };
                let [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(count),
                        ..
                    },
                ] = arguments.as_slice()
                else {
                    panic!("expected exact scalar count");
                };
                *count
            })
            .collect::<Vec<_>>();
        assert_eq!(counts, [3, 0, 3]);
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation_id(30)))
                .unwrap()
                .units(),
            3
        );
        if let Some(usage) = &reference {
            assert_eq!(meter.usage(), usage);
        } else {
            reference = Some(meter.usage().clone());
        }
    }
}
