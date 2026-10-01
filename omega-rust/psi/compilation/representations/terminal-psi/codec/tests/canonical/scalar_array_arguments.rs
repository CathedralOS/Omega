use super::{
    CodecError, Operation, OperationKind, OperationResult, ScalarType, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, Terminator, block_id, contract_id, decode_module, edge_id,
    encode_module, machine_id, operation_id, place_id, structural_type_id, unit_fixture,
};
fn array_argument_module(length: u64) -> TerminalModule {
    let mut module = unit_fixture();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(2),
                length,
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "boolean".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
    ];
    let mut callee = module.machines[0].clone();
    callee.id = machine_id(901);
    callee.entry = block_id(901);
    callee.contract.id = contract_id(901);
    callee.blocks[0].id = block_id(901);
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(901),
        trivial_affine_discards: vec![],
    };
    module.machines.push(callee);
    for machine in &mut module.machines {
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: place_id(machine.id.get()),
                position: 0,
                is_self: false,
                structural_type: structural_type_id(1),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![],
                projected_qualifications: vec![],
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(machine.id.get()),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    }
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: machine_id(901),
            arguments: vec![],
            structural_arguments: vec![StructuralArgument {
                place: place_id(900),
                path: vec![],
                access: StructuralAccess::Owned,
            }],
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    module
}

#[test]
fn owned_array_parameter_forwarding_roundtrips_without_a_new_vocabulary() {
    for length in [0, 2] {
        let module = array_argument_module(length);
        let bytes = encode_module(&module).expect("ordinary owned array argument");
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        terminal_verifier::validate_module(&decoded).unwrap();
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
    }
}

#[test]
fn owned_array_argument_encoding_rejects_borrow_and_signature_forgery() {
    let valid = array_argument_module(2);
    encode_module(&valid).expect("unmutated array argument encodes");
    terminal_verifier::validate_module(&valid).expect("unmutated array argument validates");
    let mut module = array_argument_module(2);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "primitive-array argument requires plain array transfer or loan custody"
        ))
    ));

    let mut module = array_argument_module(2);
    module.machines[1].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    assert!(encode_module(&module).is_err());

    let mut module = array_argument_module(2);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "different_length".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(2),
            length: 3,
        },
    });
    module.machines[1].structural_parameters[0].structural_type = structural_type_id(3);
    assert!(encode_module(&module).is_err());
}

fn boundary_array_argument_module(length: u64, copied: bool) -> TerminalModule {
    let mut module = array_argument_module(length);
    let mut boundary = super::unused_provider_attachment_fixture()
        .boundary_machines
        .remove(0);
    boundary.structural_parameters = module.machines[1].structural_parameters.clone();
    boundary.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    module.machines.truncate(1);
    let caller = &mut module.machines[0];
    let source = caller.structural_parameters[0].place;
    let actual = if copied { place_id(902) } else { source };
    caller.blocks[0].operations[0].kind = OperationKind::BoundaryCall {
        boundary: boundary.id,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: actual,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        completion_receipts: Vec::new(),
        requirement_obligations: Vec::new(),
    };
    if copied {
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: actual,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(2),
                structural_type: structural_type_id(1),
            },
        });
        caller.blocks[0].operations.insert(
            0,
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation_id(2),
                result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place: actual,
                    structural_type: structural_type_id(1),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::StructuralLeafCopy {
                    source,
                    path: Vec::new(),
                },
            },
        );
    }
    module.boundary_machines.push(boundary);
    module
}

#[test]
fn boundary_owned_array_parameters_and_copies_roundtrip_and_verify() {
    for length in [0, 2] {
        for copied in [false, true] {
            let module = boundary_array_argument_module(length, copied);
            let bytes = encode_module(&module).expect("whole owned boundary array encodes");
            let decoded = decode_module(&bytes).expect("boundary array decodes");
            assert_eq!(decoded, module);
            terminal_verifier::verify_module(
                &decoded,
                &terminal_verifier::ProofBundle::default(),
                &proof_admission::AdmissionProfile::default(),
            )
            .expect("reloaded array custody verifies independently");
            assert_eq!(encode_module(&decoded).unwrap(), bytes);
        }
    }
}

#[test]
fn boundary_array_encoding_rejects_access_mismatch_and_changed_shape() {
    let valid = boundary_array_argument_module(2, true);
    encode_module(&valid).expect("unmutated copied boundary array encodes");
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut changed = valid.clone();
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut changed.machines[0].blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        // Borrowing cannot satisfy an owned-by-value formal.
        structural_arguments[0].access = access;
        assert!(matches!(
            encode_module(&changed),
            Err(CodecError::MalformedStructuralFoundation(
                "primitive-array argument requires plain array transfer or loan custody"
            ))
        ));
    }

    let mut changed = valid.clone();
    changed.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "different_boundary_length".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(2),
            length: 3,
        },
    });
    changed.boundary_machines[0].structural_parameters[0].structural_type = structural_type_id(3);
    assert!(matches!(
        encode_module(&changed),
        Err(CodecError::MalformedStructuralFoundation(
            "structural argument has the wrong concrete type"
        ))
    ));

    let mut changed = valid;
    changed.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    assert!(matches!(
        encode_module(&changed),
        Err(CodecError::MalformedStructuralFoundation(
            "primitive-array argument requires plain array transfer or loan custody"
        ))
    ));
}

#[test]
fn boundary_array_result_loans_roundtrip_with_exact_presentation() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        for view in [false, true] {
            let mut module = boundary_array_argument_module(2, true);
            if view {
                module.structural_types.push(StructuralTypeDeclaration {
                    id: structural_type_id(3),
                    identity: "bool_view".into(),
                    shape: StructuralTypeShape::ElementView {
                        element: structural_type_id(2),
                    },
                });
                module.boundary_machines[0].structural_parameters[0].structural_type =
                    structural_type_id(3);
            }
            module.boundary_machines[0].structural_parameters[0].access = access;
            let OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } = &mut module.machines[0].blocks[0].operations[1].kind
            else {
                unreachable!()
            };
            structural_arguments[0].access = access;
            let bytes = encode_module(&module)
                .unwrap_or_else(|error| panic!("{access:?}, view={view}: {error:?}"));
            let decoded = decode_module(&bytes).expect("array result loan decodes");
            assert_eq!(decoded, module);
            terminal_verifier::verify_module(
                &decoded,
                &terminal_verifier::ProofBundle::default(),
                &proof_admission::AdmissionProfile::default(),
            )
            .expect("decoded backing and loan verify independently");
            assert_eq!(encode_module(&decoded).unwrap(), bytes);

            let mut wrong_type = module;
            wrong_type.boundary_machines[0].structural_parameters[0].structural_type =
                structural_type_id(2);
            assert!(encode_module(&wrong_type).is_err());
        }
    }
}

#[test]
fn array_element_loans_roundtrip_without_changing_backing_or_projection() {
    for copied in [false, true] {
        for boundary in [false, true] {
            for access in [
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
            ] {
                let mut module = boundary_array_argument_module(2, copied);
                let call_position = usize::from(copied);
                module.boundary_machines[0].structural_parameters[0].structural_type =
                    structural_type_id(2);
                module.boundary_machines[0].structural_parameters[0].access = access;
                if !boundary {
                    let mut callee = array_argument_module(2).machines.remove(1);
                    callee.structural_parameters =
                        module.boundary_machines[0].structural_parameters.clone();
                    let OperationKind::BoundaryCall {
                        structural_arguments,
                        ..
                    } = &module.machines[0].blocks[0].operations[call_position].kind
                    else {
                        unreachable!()
                    };
                    module.machines[0].blocks[0].operations[call_position].kind =
                        OperationKind::CallUnit {
                            erased_arguments: vec![],
                            erased_proof_arguments: vec![],
                            callee: callee.id,
                            arguments: vec![],
                            structural_arguments: structural_arguments.clone(),
                            claim_transfers: vec![],
                            requirement_obligations: vec![],
                            crash_continuations: vec![],
                        };
                    module.machines.push(callee);
                    module.boundary_machines.clear();
                }
                let arguments =
                    match &mut module.machines[0].blocks[0].operations[call_position].kind {
                        OperationKind::CallUnit {
                            structural_arguments,
                            ..
                        }
                        | OperationKind::BoundaryCall {
                            structural_arguments,
                            ..
                        } => structural_arguments,
                        _ => unreachable!(),
                    };
                arguments[0].access = access;
                arguments[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(1)];
                let bytes = encode_module(&module).expect("array element loan encodes");
                let decoded = decode_module(&bytes).expect("array element loan decodes");
                assert_eq!(decoded, module);
                terminal_verifier::verify_module(
                    &decoded,
                    &terminal_verifier::ProofBundle::default(),
                    &proof_admission::AdmissionProfile::default(),
                )
                .expect("decoded array element loan verifies independently");
                assert_eq!(encode_module(&decoded).unwrap(), bytes);

                let mut out_of_bounds = module.clone();
                let arguments =
                    match &mut out_of_bounds.machines[0].blocks[0].operations[call_position].kind {
                        OperationKind::CallUnit {
                            structural_arguments,
                            ..
                        }
                        | OperationKind::BoundaryCall {
                            structural_arguments,
                            ..
                        } => structural_arguments,
                        _ => unreachable!(),
                    };
                arguments[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(2)];
                assert!(encode_module(&out_of_bounds).is_err());

                let mut wrong_type = module;
                let formal = if boundary {
                    &mut wrong_type.boundary_machines[0].structural_parameters[0]
                } else {
                    &mut wrong_type.machines[1].structural_parameters[0]
                };
                formal.structural_type = structural_type_id(1);
                assert!(encode_module(&wrong_type).is_err());
            }
        }
    }
}
