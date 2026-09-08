use super::*;

fn owned_field(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = unit_fixture();
    let record = structural_type_id(901);
    let place = place_id(901);
    let scalar_type = ScalarType::Integer(i32_type());
    module.structural_types.push(StructuralTypeDeclaration {
        id: record,
        identity: "Record".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: structural_field_id(1),
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(scalar_type),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type: record,
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(902),
        scalar_type,
    });
    machine.blocks[0].operations.push(Operation {
        id: operation_id(901),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(901),
            scalar_type,
        }),
        kind: OperationKind::IntegerStructuralField {
            source: place,
            field: structural_field_id(1),
        },
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(900),
        value: value_id(901),
        cleanup_actions: if multiplicity == StructuralMultiplicity::Affine {
            vec![TerminalAffineCleanupAction::DiscardRoot(place)]
        } else {
            Vec::new()
        },
    };
    module
}

#[test]
fn owned_affine_and_copy_integer_fields_round_trip_without_new_wire_vocabulary() {
    for multiplicity in [
        StructuralMultiplicity::Affine,
        StructuralMultiplicity::Unrestricted,
    ] {
        let module = owned_field(multiplicity);
        let bytes = encode_module(&module).expect("owned integer field encodes");
        assert_eq!(decode_module(&bytes).unwrap(), module);
        assert_eq!(
            encode_module(&decode_module(&bytes).unwrap()).unwrap(),
            bytes
        );
    }
}

#[test]
fn owned_integer_field_admission_retains_type_relevance_access_and_claim_checks() {
    let original = owned_field(StructuralMultiplicity::Affine);
    for mutation in [
        "write-only",
        "linear",
        "field",
        "source",
        "type",
        "erased",
        "claim",
    ] {
        let mut module = original.clone();
        match mutation {
            "write-only" => {
                module.machines[0].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }
            "linear" => {
                module.machines[0].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Linear
            }
            "field" => {
                module.machines[0].blocks[0].operations[0].kind =
                    OperationKind::IntegerStructuralField {
                        source: place_id(901),
                        field: structural_field_id(99),
                    }
            }
            "source" => {
                module.machines[0].blocks[0].operations[0].kind =
                    OperationKind::IntegerStructuralField {
                        source: place_id(999),
                        field: structural_field_id(1),
                    }
            }
            "claim" => module.machines[0].entry_claims.push(EntryClaim {
                claim: claim_id(901),
                input: place_id(901),
                path: Vec::new(),
            }),
            "type" | "erased" => {
                let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape
                else {
                    unreachable!();
                };
                if mutation == "type" {
                    fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
                } else {
                    fields[0].relevance = BindingRelevance::Erased;
                }
            }
            _ => unreachable!(),
        }
        assert!(encode_module(&module).is_err(), "{mutation}");
    }
}
