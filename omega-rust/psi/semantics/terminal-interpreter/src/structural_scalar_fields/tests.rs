use super::*;
use crate::{StructuralAccess, StructuralMultiplicity, bind_structural_arguments};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
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
fn supplied_integer_boolean_and_nested_fields_keep_distinct_referents() {
    let (machine, types, roots) = fixture();
    let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
    let mut enabled = input(0, TerminalScalarValue::Boolean(true));
    enabled.field = StructuralFieldId::new(2).unwrap();
    let mut nested = input(0, unsigned(101));
    nested
        .path
        .push(StructuralPathSegment::Field("child".into()));
    let fields = bind(
        &machine,
        &types,
        &bindings,
        &[
            input(0, unsigned(42)),
            input(1, unsigned(99)),
            enabled,
            nested,
        ],
    )
    .unwrap();
    assert_eq!(fields.len(), 4);
    for (root, value) in roots.iter().zip([42, 99]) {
        assert_eq!(
            fields[&StructuralScalarRuntimeField {
                parent: StructuralRuntimePlace::from(root),
                field: StructuralFieldId::new(1).unwrap()
            }],
            unsigned(value)
        );
    }
    let mut parent = StructuralRuntimePlace::from(&roots[0]);
    parent
        .path
        .push(StructuralPathSegment::Field("child".into()));
    assert_eq!(
        fields[&StructuralScalarRuntimeField {
            parent,
            field: StructuralFieldId::new(1).unwrap()
        }],
        unsigned(101)
    );
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
fn duplicate_fields_reject_even_equal_values_or_aliased_parameter_positions() {
    let (machine, types, mut roots) = fixture();
    for alias in [false, true] {
        if alias {
            roots[1] = roots[0].clone();
        }
        let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
        for second_value in [42, 99] {
            let second = input(u32::from(alias), unsigned(second_value));
            assert_eq!(
                bind(
                    &machine,
                    &types,
                    &bindings,
                    &[input(0, unsigned(42)), second.clone()]
                ),
                Err(
                    TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                        argument_index: second.argument_index,
                        field: second.field
                    }
                )
            );
        }
    }
}

#[test]
fn overlapping_root_paths_cannot_supply_the_same_field_twice() {
    let (mut machine, types, mut roots) = fixture();
    roots[1].opaque_identity = roots[0].opaque_identity;
    roots[1]
        .path
        .push(StructuralPathSegment::Field("child".into()));
    roots[1].structural_type = StructuralTypeId::new(2).unwrap();
    machine.structural_parameters[1].structural_type = roots[1].structural_type;
    let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
    let mut nested = input(0, unsigned(42));
    nested
        .path
        .push(StructuralPathSegment::Field("child".into()));
    assert!(matches!(
        bind(
            &machine,
            &types,
            &bindings,
            &[nested, input(1, unsigned(99))]
        ),
        Err(
            TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                argument_index: 1,
                ..
            }
        )
    ));
}

#[test]
fn fixed_array_paths_accept_distinct_elements_and_reject_the_upper_bound() {
    let (mut machine, mut types, mut roots) = fixture();
    let array = StructuralTypeId::new(3).unwrap();
    types.insert(
        array,
        StructuralTypeDeclaration {
            id: array,
            identity: "Children".into(),
            shape: StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(2).unwrap(),
                length: 2,
            },
        },
    );
    roots[0].structural_type = array;
    machine.structural_parameters[0].structural_type = array;
    let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
    let fields = [0, 1].map(|element| TerminalStructuralScalarFieldValue {
        path: vec![StructuralPathSegment::FixedIndex(element)],
        ..input(0, unsigned(u128::from(element) + 42))
    });
    assert_eq!(bind(&machine, &types, &bindings, &fields).unwrap().len(), 2);
    let outside = TerminalStructuralScalarFieldValue {
        path: vec![StructuralPathSegment::FixedIndex(2)],
        ..input(0, unsigned(99))
    };
    assert!(matches!(
        bind(&machine, &types, &bindings, &[outside]),
        Err(TerminalInterpretError::StructuralScalarFieldArgumentInvalid { .. })
    ));
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
                    OperationKind::BooleanStructuralField { source, field }
                } else {
                    OperationKind::IntegerStructuralField { source, field }
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

#[test]
fn huge_bounded_arrays_stop_at_missing_contents_and_skip_erased_or_unbounded_children() {
    let (mut machine, mut types, mut roots) = fixture();
    restrict_child(&mut types);
    machine.structural_parameters.truncate(1);
    roots.truncate(1);
    let record = roots[0].structural_type;
    let child = StructuralTypeId::new(2).unwrap();
    types.get_mut(&record).unwrap().shape = StructuralTypeShape::FixedArray {
        element: child,
        length: u64::MAX,
    };
    let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
    let first = TerminalStructuralScalarFieldValue {
        path: vec![StructuralPathSegment::FixedIndex(0)],
        ..input(0, unsigned(3))
    };
    for supplied in [&[][..], std::slice::from_ref(&first)] {
        assert!(matches!(
            bind(&machine, &types, &bindings, supplied),
            Err(TerminalInterpretError::StructuralScalarFieldArgumentInvalid { .. })
        ));
    }
    for erased in [false, true] {
        let mut types = types.clone();
        let StructuralTypeShape::Record { fields } = &mut types.get_mut(&child).unwrap().shape
        else {
            panic!("child record")
        };
        if erased {
            fields[0].relevance = BindingRelevance::Erased;
        } else {
            fields[0].field_type = StructuralFieldType::Scalar(unsigned(0).scalar_type());
        }
        assert!(bind(&machine, &types, &bindings, &[]).unwrap().is_empty());
    }
    // A huge outer dimension does not create contents for empty inner arrays.
    let empty = StructuralTypeId::new(3).unwrap();
    let mut declaration = types[&child].clone();
    declaration.id = empty;
    declaration.shape = StructuralTypeShape::FixedArray {
        element: child,
        length: 0,
    };
    types.insert(empty, declaration);
    types.get_mut(&record).unwrap().shape = StructuralTypeShape::FixedArray {
        element: empty,
        length: u64::MAX,
    };
    assert!(bind(&machine, &types, &bindings, &[]).unwrap().is_empty());
}

#[test]
fn bounded_sum_and_mixed_entry_children_require_an_unavailable_discriminator() {
    for mixed in [false, true] {
        let (machine, mut types, roots) = fixture();
        restrict_child(&mut types);
        let child = types.get_mut(&StructuralTypeId::new(2).unwrap()).unwrap();
        let StructuralTypeShape::Record { fields } = &child.shape else {
            panic!("child record")
        };
        let cases = vec![terminal_psi::StructuralCaseDeclaration {
            id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
            identity: "Present".into(),
            fields: fields.clone(),
        }];
        child.shape = if mixed {
            StructuralTypeShape::Mixed {
                fields: Vec::new(),
                cases,
            }
        } else {
            StructuralTypeShape::Sum { cases }
        };
        let bindings = bind_structural_arguments(&machine.structural_parameters, &roots).unwrap();
        assert_eq!(
            bind(&machine, &types, &bindings, &[]),
            Err(TerminalInterpretError::VerifiedOperationMalformed)
        );
    }
}
