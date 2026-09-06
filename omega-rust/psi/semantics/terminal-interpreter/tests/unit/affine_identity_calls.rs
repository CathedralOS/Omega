use super::*;
use semantic_vocabulary::StructuralPlaceKind;

fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(sign, bits).unwrap())
}

fn identity_call_module(scalar_types: &[ScalarType]) -> TerminalModule {
    let mut module = internal_structural_call_module(false);
    module.structural_domains.clear();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![field(
            1,
            StructuralFieldType::Scalar(integer(IntegerSign::Unsigned, 64)),
        )],
    };
    for (index, machine) in module.machines.iter_mut().enumerate() {
        machine.entry_claims.clear();
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.structural_parameters[0].qualifications.clear();
        let TerminalMachineResult::Structural(result) = &mut machine.result else {
            unreachable!()
        };
        result.multiplicity = StructuralMultiplicity::Affine;
        result.qualifications.clear();
        machine.parameters = scalar_types
            .iter()
            .enumerate()
            .map(|(position, scalar_type)| ValueDeclaration {
                id: value_id((index * 100 + position + 1) as u64),
                scalar_type: *scalar_type,
            })
            .collect();
    }
    let caller = &mut module.machines[0];
    caller.result = TerminalMachineResult::Unit;
    caller
        .structural_places
        .retain(|place| place.id != place_id(2));
    let operation = &mut caller.blocks[0].operations[0];
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    result.qualifications.clear();
    result.claims.clear();
    operation.kind = OperationKind::CallStructuralWithScalarArguments {
        callee: machine_id(2),
        arguments: caller
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect(),
        structural_arguments: vec![StructuralArgument {
            place: place_id(1),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: vec![place_id(3)],
    };
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut module.machines[1].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.clear();
    module
}

fn field(number: u64, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: structural_field_id(number),
        identity: format!("field{number}"),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn discard(place: PlaceId) -> StructuralAffineDiscard {
    StructuralAffineDiscard {
        place,
        path: Vec::new(),
        structural_type: structural_type_id(1),
    }
}

fn nested_shape(module: &mut TerminalModule, array_root: bool) {
    module.structural_types[0].shape = if array_root {
        StructuralTypeShape::FixedArray {
            element: structural_type_id(3),
            length: 3,
        }
    } else {
        StructuralTypeShape::Record {
            fields: vec![field(
                1,
                StructuralFieldType::Structural(structural_type_id(2)),
            )],
        }
    };
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "test::Items".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(3),
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "test::Item".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(
                        2,
                        StructuralFieldType::Scalar(integer(IntegerSign::Unsigned, 64)),
                    ),
                    field(
                        3,
                        StructuralFieldType::Scalar(integer(IntegerSign::Signed, 16)),
                    ),
                ],
            },
        },
    ]);
}

fn verify(module: &TerminalModule) -> Result<(), VerificationError> {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .map(|_| ())
}

fn assert_resumes_without_replay(module: &TerminalModule, chained: bool) {
    let semantic = encode_module(module).expect("identity call encodes");
    let decoded = decode_module(&semantic).expect("identity call decodes");
    assert_eq!(encode_module(&decoded).unwrap(), semantic);
    let proof_bundle = ProofBundle::default();
    let verified = verify_module(&decoded, &proof_bundle, &AdmissionProfile::default())
        .expect("decoded identity call verifies");
    let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, decoded.entry)
        .expect("identity calls have fixed fuel");
    terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate)
        .expect("identity call fuel reconstructs independently");
    assert_eq!(certificate.ceiling_units(), if chained { 5 } else { 3 });
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let scalar_arguments = module.machines[0]
        .parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter.scalar_type else {
                unreachable!()
            };
            TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: match integer_type.sign() {
                    IntegerSign::Signed => IntegerValue::Signed(7),
                    IntegerSign::Unsigned => IntegerValue::Unsigned(7),
                },
            }
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &scalar_arguments,
        &[TerminalStructuralValue {
            opaque_identity: 0xaff1,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("verified affine identity caller starts");
    let mut sites = vec![
        (FuelChargeSite::Operation(operation_id(1)), place_id(1)),
        (FuelChargeSite::Edge(edge_id(2)), place_id(4)),
    ];
    if chained {
        sites.extend([
            (FuelChargeSite::Operation(operation_id(2)), place_id(3)),
            (FuelChargeSite::Edge(edge_id(2)), place_id(4)),
        ]);
    }
    sites.push((
        FuelChargeSite::Edge(edge_id(1)),
        if chained { place_id(6) } else { place_id(3) },
    ));
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for (units, (site, live_place)) in sites.iter().enumerate() {
        // Repeating a paused resume must neither move custody nor charge again.
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
                vec![discard(*live_place)]
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
    assert_eq!(meter.usage().total_units(), sites.len() as u64);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(1)))
            .unwrap()
            .units(),
        1
    );
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Edge(edge_id(2)))
            .unwrap()
            .units(),
        if chained { 2 } else { 1 }
    );
}

#[test]
fn affine_identity_calls_accept_zero_and_mixed_fixed_integer_arguments_and_owned_shapes() {
    let scalar_lists = [
        Vec::new(),
        vec![integer(IntegerSign::Unsigned, 64)],
        vec![
            integer(IntegerSign::Signed, 8),
            integer(IntegerSign::Unsigned, 16),
            integer(IntegerSign::Signed, 32),
            integer(IntegerSign::Unsigned, 64),
        ],
    ];
    for scalar_types in scalar_lists {
        for shape in 0..3 {
            let mut module = identity_call_module(&scalar_types);
            if shape != 0 {
                nested_shape(&mut module, shape == 2);
            }
            assert_resumes_without_replay(&module, false);
        }
    }
}

fn chained_module() -> TerminalModule {
    let mut module = identity_call_module(&[]);
    let caller = &mut module.machines[0];
    let mut operation = caller.blocks[0].operations[0].clone();
    operation.id = operation_id(2);
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.place = place_id(6);
    let OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = place_id(3);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(6),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(2),
            structural_type: structural_type_id(1),
        },
    });
    caller.blocks[0].operations.push(operation);
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![place_id(6)];
    module
}

#[test]
fn affine_identity_calls_consume_an_earlier_result_once() {
    let mut module = chained_module();
    nested_shape(&mut module, false);
    assert_resumes_without_replay(&module, true);
}

#[test]
fn affine_identity_calls_reject_wrong_scalar_counts_and_types() {
    let scalar_types = [
        integer(IntegerSign::Signed, 8),
        integer(IntegerSign::Unsigned, 64),
    ];
    let base = identity_call_module(&scalar_types);
    verify(&base).unwrap();
    for actual in [1, 3] {
        let mut module = base.clone();
        let OperationKind::CallStructuralWithScalarArguments { arguments, .. } =
            &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        arguments.resize(actual, value_id(1));
        assert_eq!(
            verify(&module),
            Err(VerificationError::Module(
                ModuleError::CallArgumentArityMismatch {
                    operation: operation_id(1),
                    expected: 2,
                    actual,
                }
            ))
        );
    }
    let mut module = base.clone();
    let OperationKind::CallStructuralWithScalarArguments { arguments, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    arguments.swap(0, 1);
    assert_eq!(
        verify(&module),
        Err(VerificationError::Module(
            ModuleError::CallArgumentTypeMismatch {
                operation: operation_id(1),
                argument: value_id(2),
                expected: scalar_types[0],
                actual: scalar_types[1],
            }
        ))
    );
    let mut module = base;
    module.machines[0].parameters[0].scalar_type = ScalarType::Boolean;
    module.machines[1].parameters[0].scalar_type = ScalarType::Boolean;
    assert!(
        verify(&module).is_err(),
        "matching Boolean side parameters are outside the slice"
    );
}

#[test]
fn affine_identity_calls_reject_forged_custody_and_result_locations() {
    let base = identity_call_module(&[]);
    verify(&base).unwrap();
    for mutation in 0..10 {
        let mut module = base.clone();
        let caller = &mut module.machines[0];
        let operation = &mut caller.blocks[0].operations[0];
        let OperationResult::Structural(result) = &mut operation.result else {
            unreachable!()
        };
        let OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } = &mut operation.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => result.claims.push(StructuralResultClaimBinding {
                claim: claim_id(1),
                path: Vec::new(),
            }),
            1 => claim_transfers.push(ClaimTransfer {
                claim: claim_id(1),
                argument_index: 0,
            }),
            2 => returned_claim_transfers.push(StructuralResultClaimTransfer {
                callee_claim: claim_id(1),
                caller_claim: claim_id(1),
            }),
            3 => structural_arguments[0].access = StructuralAccess::SharedBorrow,
            4 => caller.structural_parameters[0].access = StructuralAccess::SharedBorrow,
            5 => structural_arguments[0]
                .path
                .push(StructuralPathSegment::Field("field1".into())),
            6 => structural_arguments.clear(),
            7 => structural_arguments.push(structural_arguments[0].clone()),
            8 => caller
                .structural_places
                .retain(|place| place.id != place_id(3)),
            9 => {
                caller.structural_places[1].kind = StructuralPlaceKind::OperationResult {
                    producer: operation_id(2),
                    structural_type: structural_type_id(1),
                }
            }
            _ => unreachable!(),
        }
        assert!(
            verify(&module).is_err(),
            "custody mutation {mutation} must reject"
        );
    }
}

#[test]
fn affine_identity_calls_reject_unproduced_and_reused_results() {
    let base = chained_module();
    verify(&base).unwrap();
    let mut module = base.clone();
    module.machines[0].blocks[0].operations.swap(0, 1);
    assert!(
        verify(&module).is_err(),
        "a later result is not a live source"
    );
    let mut module = base;
    let OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = place_id(1);
    assert!(
        verify(&module).is_err(),
        "an already moved parameter cannot be reused"
    );
}

#[test]
fn affine_identity_calls_reject_substituted_types_borrowed_fields_and_local_sources() {
    let base = identity_call_module(&[]);
    verify(&base).unwrap();
    let mut module = base.clone();
    let mut distinct_type = module.structural_types[0].clone();
    distinct_type.id = structural_type_id(2);
    distinct_type.identity = "test::Distinct".into();
    module.structural_types.push(distinct_type);
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(2);
    assert!(
        verify(&module).is_err(),
        "identical storage does not substitute type identity"
    );

    let mut module = base.clone();
    nested_shape(&mut module, false);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView);
    assert!(
        verify(&module).is_err(),
        "a nested borrowed field is not plain owned storage"
    );

    let mut module = base;
    module.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
    let caller = &mut module.machines[0];
    caller.structural_parameters.clear();
    caller.structural_places[0].kind = StructuralPlaceKind::TrivialAffineLocal {
        declaration_ordinal: 0,
        structural_type: structural_type_id(1),
        construction: None,
    };
    caller.blocks[0].operations.insert(
        0,
        Operation {
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal {
                destination: place_id(1),
            },
        },
    );
    assert!(
        verify(&module).is_err(),
        "even an established local needs its own source slice"
    );
}

#[test]
fn affine_identity_calls_require_an_exact_claim_free_identity_callee() {
    let base = identity_call_module(&[]);
    verify(&base).unwrap();
    for mutation in 0..5 {
        let mut module = base.clone();
        let callee = &mut module.machines[1];
        match mutation {
            0 => callee.entry_claims.push(EntryClaim {
                claim: claim_id(1),
                input: place_id(4),
                path: Vec::new(),
            }),
            1 => {
                let Terminator::ReturnStructural { source, .. } = &mut callee.blocks[0].terminator
                else {
                    unreachable!()
                };
                *source = place_id(5);
            }
            2 => callee.blocks[0].operations.push(Operation {
                id: operation_id(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value_id(1),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            }),
            3 => callee.contract.crash_routes.push(CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }),
            4 => callee.structural_parameters[0]
                .qualifications
                .push(structural_domain_id(1)),
            _ => unreachable!(),
        }
        assert!(
            verify(&module).is_err(),
            "callee mutation {mutation} must reject"
        );
    }
}
