use super::*;
use crate::values::{StructuralRuntimePlace, StructuralScalarRuntimeField};
use semantic_vocabulary::{BoundedIntegerType, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
};

fn record_field(
    field: u32,
    identity: &str,
    field_type: StructuralFieldType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(field.into()).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn typed_collection(target_maximum: u128) -> (TerminalExecution, Operation) {
    let (mut execution, mut operation) = runtime_subslice();
    let source_type = StructuralTypeId::new(1).unwrap();
    let view_type = StructuralTypeId::new(2).unwrap();
    let array_type = StructuralTypeId::new(3).unwrap();
    let target_type = StructuralTypeId::new(4).unwrap();
    let signed = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let program = Arc::get_mut(&mut execution.program).unwrap();
    for (id, shape) in [
        (
            source_type,
            StructuralTypeShape::Record {
                fields: vec![
                    record_field(
                        1,
                        "signed_value",
                        StructuralFieldType::BoundedInteger(
                            BoundedIntegerType::new(
                                signed,
                                IntegerValue::Signed(0),
                                IntegerValue::Signed(100),
                            )
                            .unwrap(),
                        ),
                    ),
                    record_field(
                        2,
                        "enabled",
                        StructuralFieldType::Scalar(ScalarType::Boolean),
                    ),
                ],
            },
        ),
        (
            target_type,
            StructuralTypeShape::Record {
                fields: vec![
                    record_field(
                        3,
                        "unsigned_value",
                        StructuralFieldType::BoundedInteger(
                            BoundedIntegerType::new(
                                unsigned,
                                IntegerValue::Unsigned(0),
                                IntegerValue::Unsigned(target_maximum),
                            )
                            .unwrap(),
                        ),
                    ),
                    record_field(
                        4,
                        "active",
                        StructuralFieldType::Scalar(ScalarType::Boolean),
                    ),
                ],
            },
        ),
        (
            view_type,
            StructuralTypeShape::ElementView {
                element: target_type,
            },
        ),
        (
            array_type,
            StructuralTypeShape::FixedArray {
                element: source_type,
                length: 2,
            },
        ),
    ] {
        program.structural_types.insert(
            id,
            StructuralTypeDeclaration {
                id,
                identity: format!("typed-{}", id.get()).into(),
                shape,
            },
        );
    }
    let machine = program
        .machines
        .get_mut(&execution.current_machine)
        .unwrap();
    machine.structural_parameters[0].structural_type = array_type;
    operation.kind = OperationKind::EstablishMutableElementView {
        destination: PlaceId::new(2).unwrap(),
        source: terminal_psi::StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        },
        element: target_type,
        region: ElementViewRegion::WholeCollection,
    };
    machine
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations[0] = operation.clone();
    execution
        .structural_values
        .get_mut(&PlaceId::new(1).unwrap())
        .unwrap()
        .structural_type = array_type;
    execution.element_view_values.clear();
    for (index, number, enabled) in [(0, 7, false), (1, 11, true)] {
        let parent = StructuralRuntimePlace {
            opaque_identity: 100,
            path: vec![StructuralPathSegment::FixedIndex(index)],
        };
        execution.structural_scalar_fields.insert(
            StructuralScalarRuntimeField::member(
                parent.clone(),
                StructuralFieldId::new(1).unwrap(),
            ),
            TerminalScalarValue::Integer {
                scalar_type: signed,
                value: IntegerValue::Signed(number),
            },
        );
        execution.structural_scalar_fields.insert(
            StructuralScalarRuntimeField::member(parent, StructuralFieldId::new(2).unwrap()),
            TerminalScalarValue::Boolean(enabled),
        );
    }
    (execution, operation)
}

fn field(name: &str) -> Vec<StructuralPathSegment> {
    vec![StructuralPathSegment::Field(name.into())]
}
fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn typed_element_view_maps_fields_and_writes_original_typed_cells_after_transfer() {
    let (mut execution, operation) = typed_collection(100);
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    let source = PlaceId::new(2).unwrap();
    let view = execution.element_view_values[&source].clone();
    let ElementViewBacking::Typed(typed) = &view.backing else {
        panic!("typed backing");
    };
    assert_eq!(typed.relation.source_layout.size, 4);
    assert_eq!(typed.relation.target_layout.size, 4);
    assert_eq!(
        execution
            .read_typed_element(
                &view,
                1,
                &field("unsigned_value"),
                unsigned(0).scalar_type()
            )
            .unwrap(),
        unsigned(11)
    );
    let mut parameter =
        execution.program.machines[&execution.current_machine].structural_parameters[0].clone();
    parameter.place = PlaceId::new(3).unwrap();
    parameter.structural_type = StructuralTypeId::new(2).unwrap();
    let received = execution
        .bind_element_view_arguments(
            &[parameter],
            &[terminal_psi::StructuralArgument {
                place: source,
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            &[execution.structural_values[&source].clone()],
        )
        .unwrap();
    let transferred = &received[&PlaceId::new(3).unwrap()];
    execution
        .element_view_values
        .insert(PlaceId::new(3).unwrap(), transferred.clone());
    for (id, value) in [(1, 1), (3, 2)] {
        execution.values.insert(
            ValueId::new(id).unwrap(),
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(value),
            },
        );
    }
    execution
        .values
        .insert(ValueId::new(2).unwrap(), unsigned(99));
    let mut write = representation_write(3, 1, 2);
    let OperationKind::ElementViewWrite { path, .. } = &mut write.kind else {
        unreachable!();
    };
    *path = field("unsigned_value");
    execution.execute_element_view_write(&write).unwrap();
    execution
        .write_typed_element(
            transferred,
            0,
            &field("active"),
            TerminalScalarValue::Boolean(true),
        )
        .unwrap();
    let original_number = StructuralScalarRuntimeField::member(
        StructuralRuntimePlace {
            opaque_identity: 100,
            path: vec![StructuralPathSegment::FixedIndex(1)],
        },
        StructuralFieldId::new(1).unwrap(),
    );
    assert_eq!(
        execution.structural_scalar_fields[&original_number],
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 16).unwrap(),
            value: IntegerValue::Signed(99)
        }
    );
    assert_eq!(
        execution
            .read_typed_element(&view, 0, &field("active"), ScalarType::Boolean)
            .unwrap(),
        TerminalScalarValue::Boolean(true)
    );
    let nested = transferred
        .subslice(1, 2, StructuralAccess::SharedBorrow)
        .unwrap();
    assert_eq!(
        execution
            .read_typed_element(
                &nested,
                0,
                &field("unsigned_value"),
                unsigned(0).scalar_type()
            )
            .unwrap(),
        unsigned(99)
    );
    assert!(
        execution
            .write_typed_element(&nested, 0, &field("unsigned_value"), unsigned(12))
            .is_err()
    );
}

#[test]
fn typed_element_view_refuses_invalid_mutable_range_path_and_value() {
    let (mut execution, operation) = typed_collection(200);
    assert!(
        execution
            .execute_establish_element_view(&operation)
            .is_err(),
        "mutable widening would permit invalid source values"
    );
    assert!(
        !execution
            .element_view_values
            .contains_key(&PlaceId::new(2).unwrap())
    );
    let (mut execution, operation) = typed_collection(100);
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    let view = execution.element_view_values[&PlaceId::new(2).unwrap()].clone();
    let before = execution.structural_scalar_fields.clone();
    for (index, path, value) in [
        (0, field("unsigned_value"), unsigned(101)),
        (2, field("unsigned_value"), unsigned(99)),
        (0, field("signed_value"), unsigned(99)),
        (0, field("active"), unsigned(1)),
    ] {
        assert!(
            execution
                .write_typed_element(&view, index, &path, value)
                .is_err()
        );
    }
    assert_eq!(execution.structural_scalar_fields, before);
}

#[test]
fn typed_element_view_shared_widening_reads_source_without_mutating_it() {
    let (mut execution, mut operation) = typed_collection(200);
    let OperationKind::EstablishMutableElementView {
        destination,
        source,
        element,
        region,
    } = operation.kind
    else {
        unreachable!();
    };
    operation.kind = OperationKind::EstablishElementView {
        destination,
        source: terminal_psi::StructuralArgument {
            access: StructuralAccess::SharedBorrow,
            ..source
        },
        element,
        region,
    };
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!();
    };
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    Arc::get_mut(&mut execution.program)
        .unwrap()
        .machines
        .get_mut(&execution.current_machine)
        .unwrap()
        .blocks
        .get_mut(&execution.current)
        .unwrap()
        .operations[0] = operation.clone();
    execution
        .execute_establish_element_view(&operation)
        .unwrap();
    let view = execution.element_view_values[&PlaceId::new(2).unwrap()].clone();
    assert_eq!(
        execution
            .read_typed_element(
                &view,
                0,
                &field("unsigned_value"),
                unsigned(0).scalar_type()
            )
            .unwrap(),
        unsigned(7)
    );
    assert!(
        execution
            .write_typed_element(&view, 0, &field("unsigned_value"), unsigned(8))
            .is_err()
    );
    assert!(
        view.subslice(0, 2, StructuralAccess::MutableBorrow)
            .is_none()
    );
}
