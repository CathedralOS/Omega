use super::*;

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
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
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
            "primitive-array argument requires whole plain owned internal-call custody"
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
