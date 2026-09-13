use super::*;

fn store_module(multiplicity: StructuralMultiplicity, home: u8) -> TerminalModule {
    let mut module = if home == 1 {
        block_reader(integer_type(), multiplicity)
    } else {
        owned_reader(multiplicity, integer_type())
    };
    let machine = &mut module.machines[0];
    let block_position = usize::from(home == 1);
    let parameter = if home == 1 {
        machine.blocks[1].structural_parameters[0].clone()
    } else {
        machine.structural_parameters[0].clone()
    };
    if home == 2 {
        machine.structural_parameters.clear();
        machine.structural_places[0].kind = StructuralPlaceKind::OperationResult {
            producer: id(20),
            structural_type: parameter.structural_type,
        };
        machine.blocks[0].operations.splice(
            0..0,
            [
                Operation {
                    static_reach_binding: None,
                    id: id(19),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: id(19),
                        scalar_type: integer_type(),
                        qualifications: Default::default(),
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(41),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: id(20),
                    result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishRecord {
                        fields: vec![terminal_psi::RecordFieldInitializer {
                            field: id(1),
                            value: terminal_psi::RecordFieldValue::Scalar {
                                value: id(19),
                                range_obligation: None,
                            },
                        }],
                    },
                },
            ],
        );
    }
    machine.blocks[block_position].operations.push(Operation {
        static_reach_binding: None,
        id: id(21),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: parameter.place,
            path: Vec::new(),
            field: id(1),
            value: id(4),
        },
    });
    module
}

#[test]
fn owned_entry_block_and_established_record_stores_validate() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        for home in 0..3 {
            validate_module(&store_module(multiplicity, home))
                .expect("exact whole owned record store");
        }
    }
}

#[test]
fn owned_record_store_requires_exact_field_value_and_unbounded_invariant() {
    for home in 0..3 {
        let module = store_module(StructuralMultiplicity::Unrestricted, home);
        validate_module(&module).unwrap();
        for corruption in 0..4 {
            let mut invalid = module.clone();
            let operation = invalid.machines[0].blocks[usize::from(home == 1)]
                .operations
                .last_mut()
                .unwrap();
            let OperationKind::StructuralScalarFieldStore {
                field, path, value, ..
            } = &mut operation.kind
            else {
                unreachable!()
            };
            match corruption {
                0 => *field = id(999),
                1 => path.push(StructuralPathSegment::FixedIndex(0)),
                2 => *value = id(999),
                _ => {
                    let StructuralTypeShape::Record { fields } =
                        &mut invalid.structural_types[1].shape
                    else {
                        unreachable!()
                    };
                    let ScalarType::Integer(integer) = integer_type() else {
                        unreachable!()
                    };
                    fields[0].field_type = StructuralFieldType::BoundedInteger(
                        semantic_vocabulary::BoundedIntegerType::new(
                            integer,
                            IntegerValue::Signed(0),
                            IntegerValue::Signed(100),
                        )
                        .unwrap(),
                    );
                }
            }
            assert!(
                validate_module(&invalid).is_err(),
                "home {home}, corruption {corruption}"
            );
        }
    }
}

#[test]
fn store_cannot_use_a_record_home_before_establishment_or_block_binding() {
    for home in [1, 2] {
        let mut module = store_module(StructuralMultiplicity::Unrestricted, home);
        validate_module(&module).unwrap();
        let machine = &mut module.machines[0];
        let mut store = machine.blocks[usize::from(home == 1)]
            .operations
            .pop()
            .unwrap();
        // Use an independently dominating scalar so the failure is structural.
        let constant = Operation {
            static_reach_binding: None,
            id: id(30),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(30),
                scalar_type: integer_type(),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(2),
            },
        };
        let OperationKind::StructuralScalarFieldStore { value, .. } = &mut store.kind else {
            unreachable!()
        };
        *value = id(30);
        machine.blocks[0].operations.splice(0..0, [constant, store]);
        assert!(validate_module(&module).is_err());
    }
}

#[test]
fn store_cannot_access_affine_source_after_owned_edge_transfer() {
    let mut module = store_module(StructuralMultiplicity::Affine, 1);
    validate_module(&module).unwrap();
    let machine = &mut module.machines[0];
    let source = machine.structural_parameters[0].place;
    let operation = machine.blocks[1].operations.last_mut().unwrap();
    let OperationKind::StructuralScalarFieldStore { destination, .. } = &mut operation.kind else {
        unreachable!()
    };
    *destination = source;
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id(21),
            place: source
        })
    );
}

#[test]
fn store_rejects_read_only_authority_and_forged_local_producer() {
    let mut shared = store_module(StructuralMultiplicity::Unrestricted, 0);
    shared.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        validate_module(&shared),
        Err(ModuleError::InvalidStructuralScalarFieldStore { .. })
    ));

    let mut local = store_module(StructuralMultiplicity::Unrestricted, 2);
    let StructuralPlaceKind::OperationResult { producer, .. } =
        &mut local.machines[0].structural_places[0].kind
    else {
        unreachable!()
    };
    *producer = id(999);
    assert!(validate_module(&local).is_err());
}

#[test]
fn owned_store_rejects_a_dominating_wrong_scalar_carrier() {
    for home in 0..3 {
        let mut module = store_module(StructuralMultiplicity::Unrestricted, home);
        let block = &mut module.machines[0].blocks[usize::from(home == 1)];
        let store_position = block.operations.len() - 1;
        let OperationKind::StructuralScalarFieldStore { value, .. } =
            &mut block.operations[store_position].kind
        else {
            unreachable!()
        };
        *value = id(30);
        block.operations.insert(
            store_position,
            Operation {
                static_reach_binding: None,
                id: id(30),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: id(30),
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                }),
                kind: OperationKind::BooleanConstant { value: true },
            },
        );
        assert!(matches!(
            validate_module(&module),
            Err(ModuleError::StructuralScalarFieldStoreValueTypeMismatch { .. })
        ));
    }
}
