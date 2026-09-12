use super::*;
use terminal_psi::ScalarCaseField;

fn fixture() -> TerminalModule {
    let mut module = unit_fixture();
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let fields = vec![
        StructuralFieldDeclaration {
            id: structural_field_id(1),
            identity: "count".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(integer),
        },
        StructuralFieldDeclaration {
            id: structural_field_id(2),
            identity: "limit".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(integer),
        },
    ];
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Outcome".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: structural_case_id(1),
                    identity: "Complete".into(),
                    fields: fields.clone(),
                },
                StructuralCaseDeclaration {
                    id: structural_case_id(2),
                    identity: "Full".into(),
                    fields,
                },
            ],
        },
    });
    let machine = &mut module.machines[0];
    machine.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type: integer,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(2),
            scalar_type: integer,
        },
    ];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(2),
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(1),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(1),
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(2),
            kind: StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Structural(StructuralOperationResult {
            place: place_id(1),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case: structural_case_id(1),
            fields: vec![
                ScalarCaseField {
                    field: structural_field_id(1),
                    value: value_id(2),
                    range_obligation: None,
                },
                ScalarCaseField {
                    field: structural_field_id(2),
                    value: value_id(1),
                    range_obligation: None,
                },
            ],
        },
    }];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(900),
        source: place_id(1),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

#[test]
fn scalar_case_field_bindings_round_trip_and_bind_case_and_operand_identity() {
    let module = fixture();
    let encoded = encode_module(&module).unwrap();
    assert_eq!(decode_module(&encoded).unwrap(), module);
    let identity = semantic_fingerprint(&module).unwrap();
    for mutation in 0..2 {
        let mut changed = module.clone();
        let OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } = &mut changed.machines[0].blocks[0].operations[0].kind
        else {
            panic!("constructor");
        };
        if mutation == 0 {
            *result_case = structural_case_id(2);
        } else {
            fields[0].value = value_id(1);
        }
        assert_ne!(semantic_fingerprint(&changed).unwrap(), identity);
    }
    for length in [12, encoded.len() - 1] {
        assert!(decode_module(&encoded[..length]).is_err());
    }
}

#[test]
fn scalar_case_field_roster_and_proof_roles_reject_tampering() {
    for mutation in 0..6 {
        let mut changed = fixture();
        let OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } = &mut changed.machines[0].blocks[0].operations[0].kind
        else {
            panic!("constructor");
        };
        match mutation {
            0 => {
                fields.pop();
            }
            1 => fields.push(fields[0].clone()),
            2 => fields.reverse(),
            3 => fields[0].value = value_id(999),
            4 => fields[0].range_obligation = Some(obligation_id(1)),
            _ => *result_case = structural_case_id(999),
        }
        assert!(encode_module(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn scalar_case_range_obligation_identity_round_trips_without_claiming_a_proof() {
    let mut module = fixture();
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum");
    };
    cases[0].fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(255),
        )
        .unwrap(),
    );
    let OperationKind::EstablishScalarCase { fields, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("constructor");
    };
    fields[0].range_obligation = Some(obligation_id(1));
    let encoded = encode_module(&module).unwrap();
    assert_eq!(decode_module(&encoded).unwrap(), module);
    let identity = semantic_fingerprint(&module).unwrap();
    let OperationKind::EstablishScalarCase { fields, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("constructor");
    };
    fields[0].range_obligation = Some(obligation_id(2));
    assert_ne!(semantic_fingerprint(&module).unwrap(), identity);
}
