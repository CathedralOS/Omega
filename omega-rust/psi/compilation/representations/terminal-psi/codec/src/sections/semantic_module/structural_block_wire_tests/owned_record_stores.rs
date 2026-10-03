use super::{
    Block, Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
    Terminator, ValueDeclaration, borrowed_parameter, decode_module, encode_module, id,
    unit_module,
};
use semantic_vocabulary::ScalarType;
use terminal_psi::{
    BindingRelevance, RecordFieldInitializer, RecordFieldValue, StructuralFieldDeclaration,
    StructuralFieldType,
};

fn store_module(block_home: bool) -> TerminalModule {
    let mut module = unit_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(1),
        identity: "Record".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: id(1),
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        id: id(1),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(1),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(1),
            structural_type: id(1),
        },
    });
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(1),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: id(1),
            structural_type: id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishRecord {
            fields: vec![RecordFieldInitializer {
                field: id(1),
                value: RecordFieldValue::Scalar {
                    value: id(1),
                    range_obligation: None,
                },
            }],
        },
    });
    if block_home {
        let mut parameter = borrowed_parameter(2, 0);
        parameter.access = StructuralAccess::Owned;
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(2),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(2),
                position: 0,
            },
        });
        let terminator = machine.blocks[0].terminator.clone();
        machine.blocks[0].terminator = Terminator::Jump {
            edge: id(2),
            target: id(2),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(1),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: id(2),
            parameters: Vec::new(),
            structural_parameters: vec![parameter],
            operations: Vec::new(),
            terminator,
        });
    }
    machine
        .blocks
        .last_mut()
        .unwrap()
        .operations
        .push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(2),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: id(if block_home { 2 } else { 1 }),
                path: Vec::new(),
                field: id(1),
                value: id(1),
                range_obligation: None,
            },
        });
    module
}

#[test]
fn owned_record_store_codec_rejects_forged_home_field_and_custody() {
    for block_home in [false, true] {
        for corruption in 0..3 {
            let mut module = store_module(block_home);
            let machine = &mut module.machines[0];
            match corruption {
                0 => {
                    let OperationKind::StructuralScalarFieldStore { field, .. } = &mut machine
                        .blocks
                        .last_mut()
                        .unwrap()
                        .operations
                        .last_mut()
                        .unwrap()
                        .kind
                    else {
                        unreachable!()
                    };
                    *field = id(999);
                }
                1 => {
                    let StructuralPlaceKind::OperationResult { producer, .. } =
                        &mut machine.structural_places[0].kind
                    else {
                        unreachable!()
                    };
                    *producer = id(999);
                }
                _ if block_home => {
                    machine.blocks[1].structural_parameters[0].access =
                        StructuralAccess::SharedBorrow
                }
                _ => {
                    let OperationResult::Structural(result) =
                        &mut machine.blocks[0].operations[0].result
                    else {
                        unreachable!()
                    };
                    result.multiplicity = StructuralMultiplicity::Linear;
                }
            }
            assert!(
                encode_module(&module).is_err(),
                "block_home={block_home}, corruption={corruption}"
            );
        }
    }
}

#[test]
fn bounded_owned_record_store_round_trips_and_requires_exact_obligation_presence() {
    let mut module = store_module(false);
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap();
    let bounds = semantic_vocabulary::BoundedIntegerType::new(
        integer,
        semantic_vocabulary::IntegerValue::Signed(0),
        semantic_vocabulary::IntegerValue::Signed(16),
    )
    .unwrap();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("record");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(bounds);
    module.machines[0].parameters[0].scalar_type = ScalarType::Integer(integer);
    let OperationKind::EstablishRecord { fields } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("record");
    };
    let RecordFieldValue::Scalar {
        range_obligation, ..
    } = &mut fields[0].value
    else {
        panic!("scalar");
    };
    *range_obligation = Some(id(1));
    let OperationKind::StructuralScalarFieldStore {
        range_obligation, ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        panic!("store");
    };
    *range_obligation = Some(id(2));
    let bytes = encode_module(&module).expect("bounded store encodes");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let OperationKind::StructuralScalarFieldStore {
        range_obligation, ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        panic!("store");
    };
    *range_obligation = None;
    assert!(encode_module(&module).is_err());
}

fn erased_member_record_module() -> TerminalModule {
    let mut module = store_module(false);
    module.machines[0].blocks[0].operations.truncate(1);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!();
    };
    let mut last = fields[0].clone();
    last.id = id(3);
    last.identity = "last".into();
    fields.push(StructuralFieldDeclaration {
        id: id(2),
        identity: "evidence".into(),
        relevance: BindingRelevance::Erased,
        field_type: StructuralFieldType::Erased {
            type_identity: "EvidenceWithNominalCleanup".into(),
        },
    });
    fields.push(last);
    let initializers = record_initializers(&mut module);
    let mut last = initializers[0].clone();
    last.field = id(3);
    initializers.push(last);
    module
}

fn record_initializers(module: &mut TerminalModule) -> &mut Vec<RecordFieldInitializer> {
    let OperationKind::EstablishRecord { fields } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!();
    };
    fields
}

#[test]
fn erased_record_fields_round_trip_semantic_identity_and_runtime_roster() {
    let module = erased_member_record_module();
    let bytes = encode_module(&module).expect("only runtime fields need initializers");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let mut changed_identity = module.clone();
    let StructuralTypeShape::Record { fields } = &mut changed_identity.structural_types[0].shape
    else {
        unreachable!();
    };
    let StructuralFieldType::Erased { type_identity } = &mut fields[1].field_type else {
        unreachable!();
    };
    *type_identity = "DistinctEvidence".into();
    assert_ne!(
        encode_module(&changed_identity).unwrap(),
        bytes,
        "erased type identity remains in the canonical semantic bytes"
    );

    let mut entirely_erased = module;
    let StructuralTypeShape::Record { fields } = &mut entirely_erased.structural_types[0].shape
    else {
        unreachable!();
    };
    fields.retain(|field| field.relevance.is_erased());
    record_initializers(&mut entirely_erased).clear();
    let bytes = encode_module(&entirely_erased).expect("erased-only record has no runtime fields");
    assert_eq!(decode_module(&bytes).unwrap(), entirely_erased);
}

#[test]
fn erased_record_fields_reject_malformed_runtime_rosters_on_encode_and_decode() {
    let original = erased_member_record_module();
    for mutation in [
        "missing",
        "duplicate",
        "reordered",
        "erased initializer",
        "renumbered",
        "relevant erased carrier",
        "erased runtime carrier",
    ] {
        let mut module = original.clone();
        let initializers = record_initializers(&mut module);
        match mutation {
            "missing" => {
                initializers.pop();
            }
            "duplicate" => initializers.push(initializers[0].clone()),
            "reordered" => initializers.reverse(),
            "erased initializer" => {
                let mut erased = initializers[0].clone();
                erased.field = id(2);
                initializers.insert(1, erased);
            }
            "renumbered" => initializers[1].field = id(2),
            "relevant erased carrier" | "erased runtime carrier" => {
                let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape
                else {
                    unreachable!();
                };
                if mutation == "relevant erased carrier" {
                    fields[1].relevance = BindingRelevance::Relevant;
                } else {
                    fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
                }
            }
            _ => unreachable!(),
        }
        assert!(
            encode_module(&module).is_err(),
            "writer accepted {mutation}"
        );
        let hostile = crate::sections::semantic_module::module_wire::encode_raw(&module)
            .expect("untrusted wire can encode the malformed roster");
        assert!(
            decode_module(&hostile).is_err(),
            "decoder accepted {mutation}"
        );
    }
}
