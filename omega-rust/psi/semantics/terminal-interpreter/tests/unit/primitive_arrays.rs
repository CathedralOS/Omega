//! Borrowed scalar callees and the caller's array must observe one backing.
use super::{
    AdmissionProfile, BindingRelevance, FuelChargeSite, IntegerSign, IntegerType, IntegerValue,
    ModuleError, Operation, OperationKind, OperationResult, ProofBundle, ScalarType,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalFuelMeter, TerminalMachineResult, TerminalModule, TerminalScalarValue,
    TerminalStructuralValue, Terminator, ValueDeclaration, VerificationError, block_id,
    contract_id, decode_module, edge_id, empty_contract, encode_module, encode_proof_section,
    machine_id, operation_id, place_id, structural_field_id, structural_type_id, value_id,
    verify_module, write_only_primitive_call_module,
};
use terminal_interpreter::TerminalStructuralByteArrayValue;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_verifier::validate_module;

fn direct_array_module(nested: bool) -> (TerminalModule, Vec<StructuralPathSegment>) {
    let (mut module, array_path) = array_module(nested);
    let initializer = module.machines[1].blocks[0].operations[0].clone();
    let mut path = Vec::new();
    if nested {
        path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
            structural_field_id(1),
        ));
    }
    path.push(semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(1));
    let mut read = module.machines[0].blocks[0].operations[1].clone();
    read.kind = OperationKind::PrimitiveScalarRead {
        source: place_id(91),
        path: path.clone(),
    };
    module.machines[0].blocks[0].operations = vec![
        initializer,
        Operation {
            static_reach_binding: None,
            id: operation_id(93),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: place_id(91),
                path,
                value: value_id(92),
            },
        },
        read,
    ];
    module.machines.truncate(1);
    (module, array_path)
}

#[test]
fn direct_primitive_array_paths_round_trip_and_mutate_original_backing() {
    for nested in [false, true] {
        let (module, array_path) = direct_array_module(nested);
        let mut execution = start(&module, &array_path);
        let mut meter = TerminalFuelMeter::with_allowance(2);
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(
            execution.structural_byte_array(700, &array_path).unwrap(),
            &[11, 7, 255]
        );
        meter = TerminalFuelMeter::with_allowance(20);
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            expected(7)
        );
        assert_eq!(
            execution.structural_byte_array(700, &array_path).unwrap(),
            &[11, 7, 255]
        );
    }
}

#[test]
fn direct_primitive_array_paths_retain_affine_borrow_and_owned_root_authority() {
    for (access, multiplicity) in [
        (
            StructuralAccess::MutableBorrow,
            StructuralMultiplicity::Affine,
        ),
        (
            StructuralAccess::Owned,
            StructuralMultiplicity::Unrestricted,
        ),
    ] {
        let (mut module, array_path) = direct_array_module(true);
        module.machines[0].structural_parameters[0].access = access;
        module.machines[0].structural_parameters[0].multiplicity = multiplicity;
        if access != StructuralAccess::Owned {
            let machine = &mut module.machines[0];
            machine.attachment = Some(machine.structural_parameters[0].structural_type);
            machine.structural_parameters[0].is_self = true;
            machine.structural_places[0].kind =
                semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: true,
                };
        }
        let mut execution = start(&module, &array_path);
        assert_eq!(
            execution
                .resume(
                    &mut TerminalFuelMeter::unbounded(),
                    &mut AcceptTerminalEffects
                )
                .unwrap(),
            expected(7)
        );
        assert_eq!(
            execution.structural_byte_array(700, &array_path).unwrap(),
            &[11, 7, 255]
        );
    }
}

#[test]
fn direct_primitive_array_paths_update_constructed_scalar_payload_without_shadow_storage() {
    for selected in [0, 1] {
        let (mut module, _) = direct_array_module(false);
        let caller = &mut module.machines[0];
        caller.structural_parameters.clear();
        caller.structural_places = vec![StructuralPlaceDeclaration {
            id: place_id(91),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(94),
                structural_type: structural_type_id(93),
            },
        }];
        let mut eleven = caller.blocks[0].operations[0].clone();
        eleven.id = operation_id(95);
        let OperationResult::Scalar(value) = &mut eleven.result else {
            panic!("scalar");
        };
        value.id = value_id(95);
        eleven.kind = OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(11),
        };
        caller.blocks[0].operations.insert(1, eleven);
        caller.blocks[0].operations.insert(
            2,
            Operation {
                static_reach_binding: None,
                id: operation_id(94),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: place_id(91),
                    structural_type: structural_type_id(93),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::EstablishScalarArray {
                    elements: vec![value_id(95); 3],
                },
            },
        );
        let OperationKind::PrimitiveScalarRead { path, .. } =
            &mut caller.blocks[0].operations[4].kind
        else {
            panic!("read");
        };
        *path = vec![semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(selected)];
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_module(&bytes).unwrap(), module);
        let mut execution = TerminalExecution::start_artifact(
            &bytes,
            &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
        )
        .unwrap();
        assert_eq!(
            execution
                .resume(
                    &mut TerminalFuelMeter::unbounded(),
                    &mut AcceptTerminalEffects
                )
                .unwrap(),
            expected(if selected == 0 { 11 } else { 7 })
        );
        let mut unavailable = module.clone();
        unavailable.machines[0].blocks[0].operations.swap(2, 3);
        assert!(
            validate_module(&unavailable).is_err(),
            "store before complete establishment"
        );
    }
}

#[test]
fn direct_primitive_array_paths_reject_wrong_bounds_access_and_leaf() {
    let (module, _) = direct_array_module(true);
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut changed = module.clone();
        changed.machines[0].structural_parameters[0].access = access;
        assert!(validate_module(&changed).is_err());
    }
    for wrong in [
        vec![semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(1)],
        vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
            structural_field_id(1),
        )],
        vec![
            semantic_vocabulary::CanonicalStructuralPathSegment::Field(structural_field_id(1)),
            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(3),
        ],
        vec![
            semantic_vocabulary::CanonicalStructuralPathSegment::Field(structural_field_id(99)),
            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(1),
        ],
    ] {
        let mut changed = module.clone();
        let OperationKind::PrimitiveScalarRead { path, .. } =
            &mut changed.machines[0].blocks[0].operations[2].kind
        else {
            panic!("read");
        };
        *path = wrong;
        assert!(validate_module(&changed).is_err());
    }
}

fn array_module(nested: bool) -> (TerminalModule, Vec<StructuralPathSegment>) {
    let mut module = write_only_primitive_call_module();
    let array_type = structural_type_id(93);
    module.structural_types.push(StructuralTypeDeclaration {
        id: array_type,
        identity: "test::ThreeBytes".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(91),
            length: 3,
        },
    });
    let mut root_type = array_type;
    let mut array_path = Vec::new();
    if nested {
        root_type = structural_type_id(94);
        module.structural_types.push(StructuralTypeDeclaration {
            id: root_type,
            identity: "test::Buffer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "bytes".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(array_type),
                }],
            },
        });
        array_path.push(StructuralPathSegment::Field("bytes".into()));
    }
    let scalar = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(101),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    };
    let mut element_path = array_path.clone();
    element_path.push(StructuralPathSegment::FixedIndex(1));
    let caller = &mut module.machines[0];
    caller.structural_parameters[0].structural_type = root_type;
    caller.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(103),
        ..scalar
    });
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        panic!("writer call");
    };
    structural_arguments[0].path = element_path.clone();
    caller.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(101),
        result: OperationResult::Scalar(scalar),
        kind: OperationKind::CallStructuralScalar {
            erased_arguments: Vec::new(),
            callee: machine_id(101),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(91),
                path: element_path,
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: scalar.id,
        cleanup_actions: Vec::new(),
    };
    let mut reader = module.machines[1].clone();
    reader.id = machine_id(101);
    reader.contract = empty_contract(contract_id(101));
    reader.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    reader.structural_parameters[0].place = place_id(102);
    reader.structural_places[0].id = place_id(102);
    reader.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(104),
        ..scalar
    });
    let scalar = ValueDeclaration {
        id: value_id(105),
        ..scalar
    };
    reader.entry = block_id(101);
    reader.blocks[0].id = reader.entry;
    reader.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(102),
        result: OperationResult::Scalar(scalar),
        kind: OperationKind::PrimitiveScalarRead {
            path: Vec::new(),
            source: place_id(102),
        },
    }];
    reader.blocks[0].terminator = Terminator::Return {
        edge: edge_id(101),
        value: scalar.id,
        cleanup_actions: Vec::new(),
    };
    module.machines.push(reader);
    (module, array_path)
}

fn start(module: &TerminalModule, array_path: &[StructuralPathSegment]) -> TerminalExecution {
    // Decode and independently verify the real artifact, not private execution state.
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    TerminalExecution::start_artifact(
        &semantic,
        &encode_proof_section(module, &ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 700,
                structural_type: module.machines[0].structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            byte_arrays: &[TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: array_path.to_vec(),
                bytes: vec![11, 128, 255],
            }],
            ..Default::default()
        },
    )
    .unwrap()
}

fn expected(value: u128) -> TerminalExecutionStatus {
    TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(value),
        },
    ))
}

#[test]
fn borrowed_primitive_array_element_store_and_read_share_caller_backing() {
    for nested in [true, false] {
        let (module, path) = array_module(nested);
        let mut execution = start(&module, &path);
        let mut meter = TerminalFuelMeter::with_allowance(0);
        let mut complete = false;
        for _ in 0..32 {
            match execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    let bytes = execution.structural_byte_array(700, &path).unwrap();
                    let committed = meter
                        .usage()
                        .at(FuelChargeSite::Operation(operation_id(93)));
                    assert_eq!(
                        bytes,
                        if committed.is_some() {
                            &[11, 7, 255]
                        } else {
                            &[11, 128, 255]
                        }
                    );
                    meter.replenish(1).unwrap();
                }
                status => {
                    assert_eq!(status, expected(7));
                    complete = true;
                    break;
                }
            }
        }
        assert!(
            complete,
            "the acyclic element calls must finish within their fuel steps"
        );
        assert_eq!(
            execution.structural_byte_array(700, &path),
            Some(&[11, 7, 255][..])
        );
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation_id(93)))
                .unwrap()
                .executions(),
            1
        );
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            expected(7)
        );
        assert!(
            execution.structural_primitive_values().is_empty(),
            "array storage must not also be a copied primitive input"
        );
    }
}

#[test]
fn borrowed_primitive_array_read_keeps_element_identity() {
    let (mut module, path) = array_module(false);
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        panic!("reader call");
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(2)];
    let mut execution = start(&module, &path);
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        expected(255)
    );
    assert_eq!(
        execution.structural_byte_array(700, &path),
        Some(&[11, 7, 255][..])
    );
}

#[test]
fn nested_array_element_loans_keep_explicit_initialized_backing() {
    let (mut module, _) = array_module(false);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(94),
        identity: "test::Rows".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(93),
            length: 2,
        },
    });
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(94);
    for operation in &mut module.machines[0].blocks[0].operations {
        let arguments = match &mut operation.kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => panic!("element call"),
        };
        arguments[0]
            .path
            .insert(0, StructuralPathSegment::FixedIndex(1));
    }
    let path = vec![StructuralPathSegment::FixedIndex(1)];
    let mut execution = start(&module, &path);
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        expected(7)
    );
    assert_eq!(
        execution.structural_byte_array(700, &path),
        Some(&[11, 7, 255][..])
    );
    assert_eq!(
        execution.structural_byte_array(700, &[StructuralPathSegment::FixedIndex(0)]),
        None,
        "an unprovided row must not acquire implicit initialized contents"
    );
}

#[test]
fn overlapping_mutable_and_shared_element_loans_reject() {
    let (mut module, _) = array_module(false);
    let writer = &mut module.machines[1];
    writer.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let mut shared = writer.structural_parameters[0].clone();
    shared.place = place_id(120);
    shared.position = 1;
    shared.access = StructuralAccess::SharedBorrow;
    writer.structural_parameters.push(shared);
    writer.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(120),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("writer call");
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    let mut shared = structural_arguments[0].clone();
    shared.access = StructuralAccess::SharedBorrow;
    structural_arguments.push(shared);
    let error = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            VerificationError::Module(ModuleError::OverlappingExclusiveStructuralArguments { .. })
        ),
        "{error:?}"
    );
}

#[test]
fn borrowed_primitive_array_access_rejects_bad_index_and_write_only_read() {
    for mutation in 0..3 {
        let (mut module, _) = array_module(false);
        match mutation {
            0 => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut module.machines[0].blocks[0].operations[0].kind
                else {
                    panic!("writer call");
                };
                structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(3)];
            }
            1 => {
                module.machines[2].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow;
                let OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } = &mut module.machines[0].blocks[0].operations[1].kind
                else {
                    panic!("reader call");
                };
                structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow;
            }
            2 => {
                module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "forged array access {mutation}"
        );
    }
}

#[test]
fn mutable_array_element_loan_survives_a_structural_result_call() {
    let (mut module, path) = array_module(false);
    let writer = &mut module.machines[1];
    writer.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    writer.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(111),
        structural_type: structural_type_id(93),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    writer.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(110),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(110),
                structural_type: structural_type_id(93),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(111),
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ]);
    let result = StructuralOperationResult {
        place: place_id(110),
        structural_type: structural_type_id(93),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    writer.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(110),
        result: OperationResult::Structural(result.clone()),
        kind: OperationKind::EstablishScalarArray {
            elements: vec![value_id(92); 3],
        },
    });
    writer.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(92),
        source: place_id(110),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(112),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(91),
            structural_type: structural_type_id(93),
        },
    });
    caller.blocks[0].operations[0] = Operation {
        static_reach_binding: None,
        id: operation_id(91),
        result: OperationResult::Structural(StructuralOperationResult {
            place: place_id(112),
            ..result
        }),
        kind: OperationKind::CallStructural {
            callee: machine_id(92),
            structural_arguments: vec![StructuralArgument {
                place: place_id(91),
                path: vec![StructuralPathSegment::FixedIndex(1)],
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
    };
    let mut execution = start(&module, &path);
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        expected(7)
    );
    assert_eq!(
        execution.structural_byte_array(700, &path),
        Some(&[11, 7, 255][..])
    );
}
