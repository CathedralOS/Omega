use super::*;
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralParameterDeclaration};

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
            place: place_id(place),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            claims: vec![],
        })
    };
    caller.blocks[0].operations.push(Operation {
        id: operation_id(2),
        result: result(1),
        kind: OperationKind::EstablishScalarArray {
            elements: vec![value_id(1); dimensions.iter().product::<u64>() as usize],
        },
    });
    caller.blocks[0].operations.push(Operation {
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
