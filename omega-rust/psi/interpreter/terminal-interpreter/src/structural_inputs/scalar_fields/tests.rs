use crate::custody::bind_structural_arguments;
use crate::structural_inputs::scalar_fields::{
    BTreeMap, ExecutableMachine, OperationKind, PlaceId, StructuralFieldId, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralScalarFieldValue, TerminalStructuralValue, bind,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::{
    BindingRelevance, Block, Operation, OperationResult, StructuralFieldDeclaration,
    StructuralFieldType, StructuralParameterDeclaration, StructuralTypeShape,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn input(argument_index: u32, value: TerminalScalarValue) -> TerminalStructuralScalarFieldValue {
    TerminalStructuralScalarFieldValue {
        argument_index,
        path: Vec::new(),
        field: StructuralFieldId::new(1).unwrap(),
        value,
    }
}

fn fixture() -> (
    ExecutableMachine,
    BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    Vec<TerminalStructuralValue>,
) {
    let record = StructuralTypeId::new(1).unwrap();
    let child = StructuralTypeId::new(2).unwrap();
    let count = StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "count".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(unsigned(0).scalar_type()),
    };
    let types = BTreeMap::from([
        (
            record,
            StructuralTypeDeclaration {
                id: record,
                identity: "Record".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        count.clone(),
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "enabled".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(3).unwrap(),
                            identity: "child".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(child),
                        },
                    ],
                },
            },
        ),
        (
            child,
            StructuralTypeDeclaration {
                id: child,
                identity: "Child".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![count],
                },
            },
        ),
    ]);
    let block = BlockId::new(1).unwrap();
    let machine = ExecutableMachine {
        parameters: Vec::new(),
        structural_parameters: (0..2)
            .map(|position| StructuralParameterDeclaration {
                place: PlaceId::new(u64::from(position) + 1).unwrap(),
                position,
                is_self: false,
                structural_type: record,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            })
            .collect(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Unit,
        entry: block,
        blocks: BTreeMap::from([(
            block,
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: block,
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        )]),
    };
    let roots = [71, 83]
        .into_iter()
        .map(|opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: record,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect();
    (machine, types, roots)
}

#[test]
fn invalid_type_value_path_field_and_position_reject() {
    let (machine, types, roots) = fixture();
    let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
    for mutation in [
        "boolean",
        "width",
        "sign",
        "out of range",
        "missing path",
        "scalar path",
        "index path",
        "field",
        "position",
        "erased",
    ] {
        let mut field = input(0, unsigned(42));
        let mut types = types.clone();
        match mutation {
            "boolean" => field.value = TerminalScalarValue::Boolean(false),
            "width" => {
                field.value = TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                    value: IntegerValue::Unsigned(42),
                }
            }
            "sign" => {
                field.value = TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                    value: IntegerValue::Signed(42),
                }
            }
            "out of range" => field.value = unsigned(u128::from(u64::MAX) + 1),
            "missing path" => field
                .path
                .push(StructuralPathSegment::Field("missing".into())),
            "scalar path" => field
                .path
                .push(StructuralPathSegment::Field("count".into())),
            "index path" => field.path.push(StructuralPathSegment::FixedIndex(0)),
            "field" => field.field = StructuralFieldId::new(99).unwrap(),
            "position" => field.argument_index = 2,
            "erased" => {
                let StructuralTypeShape::Record { fields } =
                    &mut types.get_mut(&roots[0].structural_type).unwrap().shape
                else {
                    unreachable!();
                };
                fields[0].relevance = BindingRelevance::Erased;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            bind(&machine, &types, &bindings, &[field.clone()]),
            Err(
                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                    argument_index: field.argument_index,
                    field: field.field
                }
            ),
            "{mutation}"
        );
    }
}

#[test]
fn boolean_reads_require_entry_inputs_but_integer_reads_defer_until_execution() {
    for boolean in [false, true] {
        let (mut machine, types, roots) = fixture();
        let source = machine.structural_parameters[0].place;
        let field = StructuralFieldId::new(if boolean { 2 } else { 1 }).unwrap();
        machine
            .blocks
            .get_mut(&machine.entry)
            .unwrap()
            .operations
            .push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(1).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: ValueId::new(1).unwrap(),
                    scalar_type: if boolean {
                        ScalarType::Boolean
                    } else {
                        unsigned(0).scalar_type()
                    },
                }),
                kind: if boolean {
                    OperationKind::BooleanStructuralField {
                        source,
                        path: Vec::new(),
                        field,
                    }
                } else {
                    OperationKind::IntegerStructuralField {
                        source,
                        path: Vec::new(),
                        field,
                    }
                },
            });
        let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
        if boolean {
            assert_eq!(
                bind(&machine, &types, &bindings, &[]),
                Err(TerminalInterpretError::StructuralBooleanFieldMissing { source, field })
            );
        } else {
            assert!(
                bind(&machine, &types, &bindings, &[]).unwrap().is_empty(),
                "deferred integer observation creates no default contents"
            );
        }
        let supplied = TerminalStructuralScalarFieldValue {
            field,
            ..input(
                0,
                if boolean {
                    TerminalScalarValue::Boolean(false)
                } else {
                    unsigned(0)
                },
            )
        };
        assert!(bind(&machine, &types, &bindings, &[supplied]).is_ok());
    }
}

fn restrict_child(types: &mut BTreeMap<StructuralTypeId, StructuralTypeDeclaration>) {
    let child = StructuralTypeId::new(2).unwrap();
    let StructuralTypeShape::Record { fields } = &mut types.get_mut(&child).unwrap().shape else {
        panic!("child record")
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
        )
        .unwrap(),
    );
}

#[test]
fn nested_bounded_fields_require_complete_referent_contents_and_reject_alias_duplicates() {
    let (machine, mut types, mut roots) = fixture();
    restrict_child(&mut types);
    let fields = [0, 1].map(|position| TerminalStructuralScalarFieldValue {
        path: vec![StructuralPathSegment::Field("child".into())],
        ..input(position, unsigned(u128::from(position) + 3))
    });
    let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
    assert_eq!(bind(&machine, &types, &bindings, &fields).unwrap().len(), 2);
    for omitted in 0..2 {
        assert!(bind(&machine, &types, &bindings, &fields[omitted..omitted + 1]).is_err());
    }
    // Shared aliases observe one referent. One supplied value satisfies both
    // views; a second initialization of that same field remains invalid.
    roots[1] = roots[0].clone();
    let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
    assert_eq!(
        bind(&machine, &types, &bindings, &fields[..1])
            .unwrap()
            .len(),
        1
    );
    for value in [3, 4] {
        let mut supplied = fields.clone();
        supplied[1].value = unsigned(value);
        assert!(matches!(
            bind(&machine, &types, &bindings, &supplied),
            Err(
                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                    argument_index: 1,
                    ..
                }
            )
        ));
    }
}
