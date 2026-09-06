use super::*;

fn borrowed_boundary_module(access: StructuralAccess) -> TerminalModule {
    let mut module = byte_sequence_literal_module(Vec::new());
    module.structural_types[0].identity = "test::Resource".into();
    module.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
    let parameter = &mut module.boundary_machines[0].structural_parameters[0];
    parameter.multiplicity = StructuralMultiplicity::Affine;
    parameter.access = access;
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    caller.structural_places[0].kind = semantic_vocabulary::StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    caller.blocks[0].operations = (1..=2)
        .map(|ordinal| Operation {
            id: operation_id(ordinal),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary: boundary_id(1),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(1),
                    path: Vec::new(),
                    access,
                }],
                completion_receipts: Vec::new(),
            },
        })
        .collect();
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: vec![place_id(1)],
    };
    module
}

#[test]
fn whole_boundary_borrows_preserve_the_value_until_normal_owner_cleanup() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let module = borrowed_boundary_module(access);
        assert_boundary_execution(&module, &[41], &[vec![41], vec![41]]);
    }
}

#[test]
fn a_boundary_consumes_its_owned_argument_without_consuming_its_borrowed_argument() {
    let mut module = borrowed_boundary_module(StructuralAccess::SharedBorrow);
    let mut mixed = module.boundary_machines[0].clone();
    mixed.id = boundary_id(2);
    mixed.identity = "test::observe_and_consume".into();
    let mut owned = mixed.structural_parameters[0].clone();
    owned.place = place_id(3);
    owned.position = 1;
    owned.access = StructuralAccess::Owned;
    mixed.structural_parameters.push(owned);
    module.boundary_machines.push(mixed);
    let caller = &mut module.machines[0];
    let mut owned = caller.structural_parameters[0].clone();
    owned.place = place_id(2);
    owned.position = 1;
    caller.structural_parameters.push(owned);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *boundary = boundary_id(2);
    structural_arguments.push(StructuralArgument {
        place: place_id(2),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    });
    assert_boundary_execution(&module, &[41, 42], &[vec![41, 42], vec![41]]);
}

fn assert_boundary_execution(module: &TerminalModule, identities: &[u64], expected: &[Vec<u64>]) {
    let proof = ProofBundle::default();
    verify_module(module, &proof, &AdmissionProfile::default())
        .expect("repeated loans preserve owner custody");
    let semantic = encode_module(module).unwrap();
    assert_eq!(&decode_module(&semantic).unwrap(), module);
    let evidence = encode_proof_bundle(&proof).unwrap();
    let values = identities
        .iter()
        .map(|identity| TerminalStructuralValue {
            opaque_identity: *identity,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &evidence,
            &AdmissionProfile::default(),
            &[],
            &values,
        )
        .unwrap();
        let mut handler = RecordingHandler::default();
        let mut fuel = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        for _ in 0..32 {
            match execution
                .resume_with_effect_handler(&mut fuel, &mut handler)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(incremental);
                    assert_eq!(
                        execution.live_affine_frontier().count(),
                        if handler.effects.is_empty() {
                            identities.len()
                        } else {
                            1
                        },
                        "borrowed value remains live before final owner cleanup"
                    );
                    fuel.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected borrow execution status: {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(handler.effects.len(), expected.len());
        for (effect, expected) in handler.effects.iter().zip(expected) {
            let TerminalEffect::BoundaryCall {
                structural_arguments,
                ..
            } = effect
            else {
                panic!("boundary effect")
            };
            assert_eq!(
                structural_arguments
                    .iter()
                    .map(|value| value.opaque_identity)
                    .collect::<Vec<_>>(),
                *expected
            );
        }
        assert!(execution.live_affine_frontier().next().is_none());
        assert_eq!(execution.effects(), handler.effects);
        if let Some(reference) = &reference {
            assert_eq!(execution.effects(), reference);
        } else {
            reference = Some(execution.effects().to_vec());
        }
    }
}
