use super::{
    AdmissionProfile, ModuleError, Operation, OperationKind, OperationResult, ProofBundle,
    ScalarType, StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachineResult, TerminalModule, Terminator, block_id,
    boolean_declaration, call_module, edge_id, machine_id, operation_id, place_id,
    structural_type_id, validate_module, value_id, verify_module,
};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralParameterDeclaration};

#[path = "scalar_array_arguments/selection.rs"]
mod selection;

#[path = "scalar_array_arguments/boundary.rs"]
mod boundary;

fn array_call(dimensions: &[u64]) -> TerminalModule {
    let mut module = call_module();
    for (position, length) in dimensions.iter().enumerate() {
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(position as u64 + 1),
            identity: format!("array{position}"),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(position as u64 + 2),
                length: *length,
            },
        });
    }
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(dimensions.len() as u64 + 1),
        identity: "leaf".into(),
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
    });
    for machine in &mut module.machines {
        machine.parameters.clear();
        machine.contract.requires.clear();
        machine.contract.ensures.clear();
        machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: place_id(9 + machine.id.get()),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
        });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(9 + machine.id.get()),
            kind: StructuralPlaceKind::Result,
        });
        machine.blocks[0].operations.clear();
        machine.blocks[0].terminator = Terminator::ReturnStructural {
            edge: edge_id(machine.id.get()),
            source: place_id(if machine.id == machine_id(1) { 2 } else { 4 }),
            returned_claims: vec![],
            trivial_affine_discards: vec![],
        };
    }
    let caller = &mut module.machines[0];
    caller.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(1),
        result: OperationResult::Scalar(boolean_declaration(value_id(1))),
        kind: OperationKind::BooleanConstant { value: true },
    });
    for (place, producer) in [(1, 2), (2, 3)] {
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(place),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(producer),
                structural_type: structural_type_id(1),
            },
        });
    }
    let result = |place| {
        OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: place_id(place),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            claims: vec![],
        })
    };
    caller.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(2),
        result: result(1),
        kind: OperationKind::EstablishScalarArray {
            elements: vec![value_id(1); dimensions.iter().product::<u64>() as usize],
        },
    });
    caller.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(3),
        result: result(2),
        kind: OperationKind::CallStructural {
            callee: machine_id(2),
            structural_arguments: vec![
                StructuralArgument {
                    place: place_id(1),
                    path: vec![],
                    access: StructuralAccess::Owned,
                };
                2
            ],
            claim_transfers: vec![],
            returned_claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
            selected_evidence: vec![],
        },
    });
    let callee = &mut module.machines[1];
    for position in 0..2 {
        let place = place_id(u64::from(position) + 3);
        callee
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                is_self: false,
                structural_type: structural_type_id(1),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![],
                projected_qualifications: vec![],
            });
        callee.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
    }
    module
}

#[test]
fn repeated_owned_arrays_and_nonfirst_parameter_returns_are_verified() {
    for dimensions in [&[2][..], &[2, 2], &[2, 0], &[0, 2, 0]] {
        verify_module(
            &array_call(dimensions),
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("source-free owned array call and parameter return");
    }
}

fn array_element_loan(access: StructuralAccess) -> TerminalModule {
    let mut module = array_call(&[2, 2]);
    for machine in &mut module.machines {
        machine.result = TerminalMachineResult::Unit;
        machine.structural_places.retain(|place| {
            !matches!(place.kind, StructuralPlaceKind::Result)
                && place.id != place_id(2)
                && place.id != place_id(4)
        });
        machine.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(machine.id.get()),
            trivial_affine_discards: Vec::new(),
        };
    }
    let call = &mut module.machines[0].blocks[0].operations[2];
    call.result = OperationResult::Unit;
    call.kind = OperationKind::CallUnit {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        callee: machine_id(2),
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: place_id(1),
            path: vec![terminal_psi::StructuralPathSegment::FixedIndex(1); 2],
            access,
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let callee = &mut module.machines[1];
    callee.structural_parameters.truncate(1);
    callee.structural_parameters[0].structural_type = structural_type_id(3);
    callee.structural_parameters[0].access = access;
    module
}

#[test]
fn initialized_array_element_loans_preserve_projection_access_and_availability() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let valid = array_element_loan(access);
        verify_module(
            &valid,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("a checked nested element loan keeps its initialized backing");

        let mut early = valid.clone();
        early.machines[0].blocks[0].operations.swap(1, 2);
        assert!(
            validate_module(&early).is_err(),
            "a future producer is not backing"
        );

        let mut wrong_type = valid.clone();
        wrong_type.machines[1].structural_parameters[0].structural_type = structural_type_id(2);
        assert!(
            validate_module(&wrong_type).is_err(),
            "the formal must name the selected leaf"
        );

        let mut outside = valid.clone();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut outside.machines[0].blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        structural_arguments[0].path[0] = terminal_psi::StructuralPathSegment::FixedIndex(2);
        assert!(
            validate_module(&outside).is_err(),
            "the projection stays inside the array"
        );

        let mut owned_projection = valid.clone();
        owned_projection.machines[1].structural_parameters[0].access = StructuralAccess::Owned;
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut owned_projection.machines[0].blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        structural_arguments[0].access = StructuralAccess::Owned;
        assert!(
            validate_module(&owned_projection).is_err(),
            "loans do not admit projected owned transfers"
        );

        let mut overlap = valid.clone();
        let mut parameter = overlap.machines[1].structural_parameters[0].clone();
        parameter.place = place_id(4);
        parameter.position = 1;
        overlap.machines[1].structural_parameters.push(parameter);
        overlap.machines[1]
            .structural_places
            .push(StructuralPlaceDeclaration {
                id: place_id(4),
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            });
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut overlap.machines[0].blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        structural_arguments.push(structural_arguments[0].clone());
        if access == StructuralAccess::MutableBorrow {
            assert!(matches!(
                validate_module(&overlap),
                Err(ModuleError::OverlappingExclusiveStructuralArguments { .. })
            ));
        } else {
            validate_module(&overlap).expect("shared loans may observe the same element");
        }

        let mut parameter_backing = valid;
        let caller = &mut parameter_backing.machines[0];
        caller.blocks[0].operations.remove(1);
        caller.structural_places[0].kind = StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        };
        caller
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: place_id(1),
                position: 0,
                is_self: false,
                structural_type: structural_type_id(1),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        verify_module(
            &parameter_backing,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("an owned parameter lends initialized storage by the same rule");
    }
}

/// A record that stores a plain array owns a copy, so the ownership
/// frontier does not track the array. The record still may not consume it
/// before the array's establishment has run.
#[test]
fn record_fields_require_an_established_plain_array() {
    let mut module = array_call(&[2]);
    module.machines.truncate(1);
    let holder = structural_type_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: holder,
        identity: "holder".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![terminal_psi::StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "items".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Structural(structural_type_id(1)),
            }],
        },
    });
    let caller = &mut module.machines[0];
    let TerminalMachineResult::Structural(result) = &mut caller.result else {
        unreachable!()
    };
    result.structural_type = holder;
    for place in &mut caller.structural_places {
        if place.id == place_id(2) {
            place.kind = StructuralPlaceKind::OperationResult {
                producer: operation_id(3),
                structural_type: holder,
            };
        }
    }
    let record = &mut caller.blocks[0].operations[2];
    let OperationResult::Structural(record_result) = &mut record.result else {
        unreachable!()
    };
    record_result.structural_type = holder;
    record.kind = OperationKind::EstablishRecord {
        fields: vec![terminal_psi::RecordFieldInitializer {
            field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            value: terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                place: place_id(1),
                path: vec![],
                access: StructuralAccess::Owned,
            }),
        }],
    };
    validate_module(&module).expect("a record stores an established plain array");
    let mut early = module;
    early.machines[0].blocks[0].operations.swap(1, 2);
    assert!(
        matches!(
            validate_module(&early),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
        ),
        "{:?}",
        validate_module(&early)
    );
}

#[test]
fn array_arguments_require_live_exact_whole_owned_sources() {
    let valid = array_call(&[2]);
    validate_module(&valid).expect("unmutated array call");
    let mut forward = valid.clone();
    forward.machines[0].blocks[0].operations.swap(1, 2);
    assert!(matches!(
        validate_module(&forward),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
    ));

    let mut borrowed = valid.clone();
    let OperationKind::CallStructural {
        structural_arguments,
        ..
    } = &mut borrowed.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert!(validate_module(&borrowed).is_err());

    let mut wrong_shape = valid.clone();
    wrong_shape.machines[1].structural_parameters[0].structural_type = structural_type_id(2);
    assert!(validate_module(&wrong_shape).is_err());

    let mut wrong_source = valid.clone();
    let Terminator::ReturnStructural { source, .. } =
        &mut wrong_source.machines[1].blocks[0].terminator
    else {
        unreachable!()
    };
    *source = place_id(11);
    assert!(validate_module(&wrong_source).is_err());

    let mut wrong_multiplicity = valid;
    wrong_multiplicity.machines[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    assert!(validate_module(&wrong_multiplicity).is_err());
}

#[test]
fn array_parameters_do_not_admit_block_payload_transport() {
    let mut module = array_call(&[0, 2]);
    validate_module(&module).expect("unmutated empty nested array call");
    let callee = &mut module.machines[1];
    let mut successor = callee.blocks[0].clone();
    successor.id = block_id(3);
    let mut parameter = callee.structural_parameters[0].clone();
    parameter.place = place_id(5);
    successor.structural_parameters.push(parameter);
    successor.terminator = Terminator::ReturnStructural {
        edge: edge_id(3),
        source: place_id(5),
        returned_claims: vec![],
        trivial_affine_discards: vec![],
    };
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(5),
        kind: StructuralPlaceKind::BlockParameter {
            block: block_id(3),
            position: 0,
        },
    });
    callee.blocks.push(successor);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidBlockStructuralParameter { .. })
    ));
}
