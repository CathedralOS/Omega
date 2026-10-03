use super::*;
fn result_mut(operation: &mut Operation) -> &mut StructuralOperationResult {
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result
}

fn array_module() -> TerminalModule {
    let mut module = record_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        identity: "CompletedPoints".into(),
        shape: StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(1).unwrap(),
            length: 2,
        },
    });
    let machine = &mut module.machines[0];
    machine.parameters.push(scalar(2));
    result_mut(&mut machine.blocks[0].operations[0]).multiplicity = StructuralMultiplicity::Affine;
    let mut second = machine.blocks[0].operations[0].clone();
    second.id = OperationId::new(2).unwrap();
    result_mut(&mut second).place = PlaceId::new(2).unwrap();
    let OperationKind::EstablishRecord { fields } = &mut second.kind else {
        unreachable!()
    };
    fields[0].value = RecordFieldValue::Scalar {
        value: ValueId::new(2).unwrap(),
        range_obligation: None,
    };
    machine.blocks[0].operations.push(second);
    machine.structural_places.extend([
        StructuralPlaceDeclaration {
            id: PlaceId::new(2).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(2).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
            },
        },
        StructuralPlaceDeclaration {
            id: PlaceId::new(3).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(3).unwrap(),
                structural_type: StructuralTypeId::new(2).unwrap(),
            },
        },
    ]);
    machine.blocks[0].operations.push(Operation {
        id: OperationId::new(3).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: PlaceId::new(3).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishStructuralArray {
            elements: (1..=2)
                .map(|identity| StructuralArgument {
                    place: PlaceId::new(identity).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                })
                .collect(),
        },
    });
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(900).unwrap(),
        trivial_affine_discards: vec![PlaceId::new(3).unwrap()],
    };
    module
}

fn start(
    module: &TerminalModule,
) -> Result<TerminalExecution, crate::errors::TerminalArtifactInterpretError> {
    TerminalExecution::start_artifact(
        &encode_module(module).unwrap(),
        &encode_proof_section(module, &ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[unsigned(30), unsigned(40)],
        TerminalStructuralInputs::default(),
    )
}

#[test]
fn completed_structural_array_roundtrips_and_pays_before_atomic_transfer() {
    let module = array_module();
    let bytes = encode_module(&module).unwrap();
    let decoded = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(decoded, module);
    let mut execution = start(&decoded).expect("independent complete array verification");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    let frontier = execution.live_affine_frontier.clone();
    let storage = execution.structural_scalar_fields.clone();
    assert_eq!(frontier.len(), 2);
    assert!(
        !execution
            .structural_values
            .contains_key(&PlaceId::new(3).unwrap())
    );
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.live_affine_frontier, frontier);
    assert_eq!(execution.structural_scalar_fields, storage);
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    let array = &execution.structural_values[&PlaceId::new(3).unwrap()];
    for (index, expected) in [(0, 30), (1, 40)] {
        let field = crate::values::StructuralScalarRuntimeField::member(
            crate::values::StructuralRuntimePlace {
                opaque_identity: array.opaque_identity,
                path: vec![StructuralPathSegment::FixedIndex(index)],
            },
            StructuralFieldId::new(1).unwrap(),
        );
        assert_eq!(
            execution.structural_scalar_fields.get(&field),
            Some(&unsigned(expected))
        );
    }
    assert_eq!(execution.live_affine_frontier.len(), 1);
    let committed_storage = execution.structural_scalar_fields.clone();
    let committed_frontier = execution.live_affine_frontier.clone();
    let committed_cursor = execution.local_structural_identities.cursor();
    assert!(
        execution
            .execute_establish_structural_array(&decoded.machines[0].blocks[0].operations[2])
            .is_err()
    );
    assert_eq!(execution.structural_scalar_fields, committed_storage);
    assert_eq!(execution.live_affine_frontier, committed_frontier);
    assert_eq!(
        execution.local_structural_identities.cursor(),
        committed_cursor
    );
    assert!(
        !execution
            .structural_values
            .contains_key(&PlaceId::new(1).unwrap())
    );
    assert!(
        !execution
            .structural_values
            .contains_key(&PlaceId::new(2).unwrap())
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
}

#[test]
fn completed_structural_array_rejects_count_type_permission_and_repeated_owner() {
    for corruption in 0..5 {
        let mut module = array_module();
        let operation = &mut module.machines[0].blocks[0].operations[2];
        let OperationKind::EstablishStructuralArray { elements } = &mut operation.kind else {
            unreachable!()
        };
        match corruption {
            0 => {
                elements.pop();
            }
            1 => {
                elements[0].access = StructuralAccess::SharedBorrow;
            }
            2 => {
                elements[1] = elements[0].clone();
            }
            3 => {
                result_mut(operation).multiplicity = StructuralMultiplicity::Unrestricted;
            }
            _ => {
                module.structural_types[1].shape = StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(2).unwrap(),
                    length: 2,
                };
            }
        }
        let rejected = match encode_module(&module) {
            Err(_) => true,
            Ok(bytes) => TerminalExecution::start_artifact(
                &bytes,
                &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
                &AdmissionProfile::default(),
                &[unsigned(30), unsigned(40)],
                TerminalStructuralInputs::default(),
            )
            .is_err(),
        };
        assert!(rejected, "corruption {corruption}");
    }
}

#[test]
fn completed_structural_array_projected_elements_keep_siblings_and_reject_duplicate_move() {
    for duplicate in [false, true] {
        let (mut execution, mut result, fields) = projected_constructor_fixture(false);
        result.structural_type = StructuralTypeId::new(4).unwrap();
        execution.insert_structural_type(StructuralTypeDeclaration {
            id: result.structural_type,
            identity: "SelectedArray".into(),
            shape: StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(1).unwrap(),
                length: if duplicate { 2 } else { 1 },
            },
        });
        let RecordFieldValue::Structural(argument) = &fields[0].value else {
            unreachable!()
        };
        let operation = Operation {
            id: OperationId::new(100).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(result.clone()),
            kind: OperationKind::EstablishStructuralArray {
                elements: vec![argument.clone(); if duplicate { 2 } else { 1 }],
            },
        };
        let before = execution.structural_scalar_fields.clone();
        let frontier = execution.live_affine_frontier.clone();
        let cursor = execution.local_structural_identities.cursor();
        let outcome = execution.execute_establish_structural_array(&operation);
        if duplicate {
            assert!(outcome.is_err());
            assert_eq!(execution.structural_scalar_fields, before);
            assert_eq!(execution.live_affine_frontier, frontier);
            assert_eq!(execution.local_structural_identities.cursor(), cursor);
        } else {
            outcome.unwrap();
            assert!(
                execution
                    .live_affine_frontier
                    .iter()
                    .any(|entry| entry.place == argument.place
                        && entry.path == vec![StructuralPathSegment::Field("right".into())])
            );
            let root = &execution.structural_values[&result.place];
            let cell = crate::values::StructuralScalarRuntimeField::member(
                crate::values::StructuralRuntimePlace {
                    opaque_identity: root.opaque_identity,
                    path: vec![StructuralPathSegment::FixedIndex(0)],
                },
                StructuralFieldId::new(1).unwrap(),
            );
            assert_eq!(
                execution.structural_scalar_fields.get(&cell),
                Some(&unsigned(11))
            );
        }
    }
}

fn descriptor_array_module(shared_parameter: bool) -> TerminalModule {
    let mut module = unit_module();
    let view = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: view,
            identity: "SharedBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }),
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "SharedByteDescriptors".into(),
            shape: StructuralTypeShape::FixedArray {
                element: view,
                length: 1,
            },
        },
    ];
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(scalar(5));
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: view,
            },
        },
        StructuralPlaceDeclaration {
            id: PlaceId::new(2).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(2).unwrap(),
                structural_type: array,
            },
        },
        StructuralPlaceDeclaration {
            id: PlaceId::new(3).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(3).unwrap(),
                structural_type: view,
            },
        },
    ];
    let result = |place, structural_type| {
        OperationResult::Structural(StructuralOperationResult {
            place: PlaceId::new(place).unwrap(),
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        })
    };
    machine.blocks[0].operations = vec![
        Operation {
            id: OperationId::new(1).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: PlaceId::new(1).unwrap(),
                bytes: b"abc".to_vec(),
                qualifications: Vec::new(),
            },
        },
        Operation {
            id: OperationId::new(2).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: result(2, array),
            kind: OperationKind::EstablishStructuralArray {
                elements: vec![StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }],
            },
        },
        Operation {
            id: OperationId::new(3).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: result(3, view),
            kind: OperationKind::StructuralLeafCopy {
                source: PlaceId::new(2).unwrap(),
                path: vec![StructuralPathSegment::FixedIndex(0)],
            },
        },
        Operation {
            id: OperationId::new(4).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(scalar(4)),
            kind: OperationKind::ByteSequenceLength {
                source: PlaceId::new(3).unwrap(),
            },
        },
    ];
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(4).unwrap(),
        cleanup_actions: Vec::new(),
    };
    if shared_parameter {
        machine.blocks[0].operations.remove(0);
        machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        };
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: PlaceId::new(1).unwrap(),
                position: 0,
                is_self: false,
                structural_type: view,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        let mut caller = unit_module().machines.remove(0);
        caller.id = MachineId::new(901).unwrap();
        caller.contract.id = ContractId::new(901).unwrap();
        caller.entry = BlockId::new(901).unwrap();
        caller.blocks[0].id = caller.entry;
        caller.result = TerminalMachineResult::Scalar(scalar(15));
        caller.structural_places = vec![StructuralPlaceDeclaration {
            id: PlaceId::new(11).unwrap(),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: view,
            },
        }];
        caller.blocks[0].operations = vec![
            Operation {
                id: OperationId::new(11).unwrap(),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Unit,
                kind: OperationKind::EstablishByteSequenceLiteral {
                    destination: PlaceId::new(11).unwrap(),
                    bytes: b"abc".to_vec(),
                    qualifications: Vec::new(),
                },
            },
            Operation {
                id: OperationId::new(12).unwrap(),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Scalar(scalar(14)),
                kind: OperationKind::CallStructuralScalar {
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    callee: MachineId::new(900).unwrap(),
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: PlaceId::new(11).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    }],
                    claim_transfers: Vec::new(),
                },
            },
        ];
        caller.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(901).unwrap(),
            value: ValueId::new(14).unwrap(),
            cleanup_actions: Vec::new(),
        };
        module.entry = caller.id;
        module.machines.push(caller);
    }
    module
}

#[test]
fn completed_structural_array_shared_descriptors_keep_exact_backing_and_indexed_observation() {
    for shared_parameter in [false, true] {
        let module = descriptor_array_module(shared_parameter);
        let (_, result) = run(&module, &[]);
        assert_eq!(result, TerminalExecutionResult::Scalar(unsigned(3)));
    }
    // Runtime receiving independently preserves the exact backing/window, even
    // after the source descriptor slot is rebound to a different byte value.
    let module = descriptor_array_module(false);
    let mut execution = TerminalExecution::start_artifact(
        &encode_module(&module).unwrap(),
        &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(1);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    let source = execution.byte_sequence_values[&PlaceId::new(1).unwrap()]
        .immutable()
        .unwrap()
        .clone();
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    execution.byte_sequence_values.insert(
        PlaceId::new(1).unwrap(),
        crate::byte_sequences::binding::ByteSequenceBinding::Immutable(
            crate::byte_sequences::ByteSequenceView::new(b"replacement".to_vec()),
        ),
    );
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    let copied = execution.byte_sequence_values[&PlaceId::new(3).unwrap()]
        .immutable()
        .unwrap();
    assert_eq!(copied.bytes(), b"abc");
    assert_eq!(copied.bytes().as_ptr(), source.bytes().as_ptr());
}

#[test]
fn completed_structural_array_unrestricted_reentry_replaces_only_the_result_home() {
    let mut module = array_module();
    for operation in &mut module.machines[0].blocks[0].operations {
        result_mut(operation).multiplicity = StructuralMultiplicity::Unrestricted;
    }
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.clear();
    let mut execution = start(&module).expect("unrestricted complete array");
    let mut meter = TerminalFuelMeter::with_allowance(3);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    let operation = &module.machines[0].blocks[0].operations[2];
    let destination = PlaceId::new(3).unwrap();
    let first = execution.structural_values[&destination].clone();
    let source = execution.structural_values[&PlaceId::new(1).unwrap()].clone();
    let source_field = crate::values::StructuralScalarRuntimeField::member(
        crate::values::StructuralRuntimePlace {
            opaque_identity: source.opaque_identity,
            path: source.path.clone(),
        },
        StructuralFieldId::new(1).unwrap(),
    );
    execution
        .structural_scalar_fields
        .insert(source_field, unsigned(99));
    execution
        .execute_establish_structural_array(operation)
        .unwrap();
    let second = &execution.structural_values[&destination];
    assert_ne!(first.opaque_identity, second.opaque_identity);
    for (identity, expected) in [(first.opaque_identity, 30), (second.opaque_identity, 99)] {
        let field = crate::values::StructuralScalarRuntimeField::member(
            crate::values::StructuralRuntimePlace {
                opaque_identity: identity,
                path: vec![StructuralPathSegment::FixedIndex(0)],
            },
            StructuralFieldId::new(1).unwrap(),
        );
        assert_eq!(
            execution.structural_scalar_fields.get(&field),
            Some(&unsigned(expected))
        );
    }
    assert!(execution.live_affine_frontier.is_empty());
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
}
