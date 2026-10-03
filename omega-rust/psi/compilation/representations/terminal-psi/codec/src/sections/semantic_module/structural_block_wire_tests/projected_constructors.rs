use super::{decode_module, encode_module, id, unit_module};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    BindingRelevance, Operation, OperationKind, OperationResult, RecordFieldInitializer,
    RecordFieldValue, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, Terminator,
};

fn projected_constructor(case: bool) -> TerminalModule {
    let mut module = unit_module();
    let field = |number, name: &str, target| StructuralFieldDeclaration {
        id: id(number),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(id(target)),
    };
    let payload = vec![field(3, "payload", 2)];
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "Root".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(1, "left", 2), field(2, "right", 4)],
            },
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "Token".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
        StructuralTypeDeclaration {
            id: id(3),
            identity: "Wrapper".into(),
            shape: if case {
                StructuralTypeShape::Sum {
                    cases: vec![StructuralCaseDeclaration {
                        id: id(1),
                        identity: "Present".into(),
                        fields: payload,
                    }],
                }
            } else {
                StructuralTypeShape::Record { fields: payload }
            },
        },
        StructuralTypeDeclaration {
            id: id(4),
            identity: "Tokens".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(2),
                length: 2,
            },
        },
    ];
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(1),
            position: 0,
            is_self: false,
            structural_type: id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(2),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(1),
                structural_type: id(3),
            },
        },
    ];
    let fields = vec![RecordFieldInitializer {
        field: id(3),
        value: RecordFieldValue::Structural(StructuralArgument {
            place: id(1),
            path: vec![
                StructuralPathSegment::Field("right".into()),
                StructuralPathSegment::FixedIndex(1),
            ],
            access: StructuralAccess::Owned,
        }),
    }];
    machine.blocks[0].operations.push(Operation {
        id: id(1),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: id(2),
            structural_type: id(3),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: if case {
            OperationKind::EstablishStructuralCase {
                result_case: id(1),
                fields,
            }
        } else {
            OperationKind::EstablishRecord { fields }
        },
    });
    machine.blocks[0].terminator = Terminator::ReturnUnitPartialAffine {
        edge: id(1),
        trivial_affine_discards: vec![id(2)],
        residual_affine_discards: vec![
            StructuralAffineDiscard {
                place: id(1),
                path: vec![
                    StructuralPathSegment::Field("right".into()),
                    StructuralPathSegment::FixedIndex(0),
                ],
                structural_type: id(2),
            },
            StructuralAffineDiscard {
                place: id(1),
                path: vec![StructuralPathSegment::Field("left".into())],
                structural_type: id(2),
            },
        ],
    };
    module
}

#[test]
fn projected_constructor_operands_preserve_canonical_paths_and_custody() {
    for case in [false, true] {
        let module = projected_constructor(case);
        let bytes = encode_module(&module).expect("finite projected constructor operand");
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
        for corruption in 0..5 {
            let mut wrong = module.clone();
            let operation = &mut wrong.machines[0].blocks[0].operations[0];
            let (OperationKind::EstablishRecord { fields }
            | OperationKind::EstablishStructuralCase { fields, .. }) = &mut operation.kind
            else {
                unreachable!()
            };
            let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
                unreachable!()
            };
            match corruption {
                0 => argument.path[1] = StructuralPathSegment::FixedIndex(2),
                1 => argument.access = StructuralAccess::SharedBorrow,
                2 => argument.path = vec![StructuralPathSegment::Field("right".into())],
                3 => argument.path.push(StructuralPathSegment::Referent),
                _ => argument.place = id(99),
            }
            assert!(
                encode_module(&wrong).is_err(),
                "case={case}, corruption={corruption}"
            );
        }
    }
}
