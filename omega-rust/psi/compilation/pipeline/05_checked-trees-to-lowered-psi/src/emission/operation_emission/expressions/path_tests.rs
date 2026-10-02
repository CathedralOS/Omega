use super::*;

fn count_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn count(value: u128) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerLiteral {
        value: IntegerValue::Unsigned(value),
        scalar_type: count_type(),
    }
}

#[test]
fn byte_field_observation_evaluates_carrier_before_byte_once() {
    let source = PlaceId::new(7).unwrap();
    let field = StructuralFieldId::new(2).unwrap();
    let expression = LoweredDirectExpression::ByteSequenceFieldRead {
        source,
        path: vec![
            LoweredElementPathSegment::RuntimeIndex {
                selector: count(2),
                extent: None,
            },
            LoweredElementPathSegment::Static(terminal_psi::StructuralPathSegment::Field(
                "rows".into(),
            )),
            LoweredElementPathSegment::RuntimeIndex {
                selector: count(3),
                extent: None,
            },
        ],
        field,
        index: Box::new(count(5)),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    };
    let mut operations = OperationBuffer::new(0);
    let mut next_value = 1;
    emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
    let constants: Vec<_> = operations
        .operations
        .iter()
        .filter_map(|operation| {
            if let OperationKind::IntegerConstant { value } = operation.kind {
                Some(value)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        constants,
        [
            IntegerValue::Unsigned(2),
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5)
        ]
    );
    assert_eq!(operations.operations.len(), 5);
    let OperationKind::StructuralByteSequenceFieldLength {
        source: measured,
        path: length_path,
        field: measured_field,
    } = &operations.operations[3].kind
    else {
        panic!("field length follows selectors")
    };
    let OperationKind::StructuralByteSequenceFieldRead {
        source: read,
        path: read_path,
        field: read_field,
        index,
        length,
        obligation,
    } = &operations.operations[4].kind
    else {
        panic!("field read follows length")
    };
    assert_eq!((*measured, *read), (source, source));
    assert_eq!((*measured_field, *read_field), (field, field));
    assert_eq!(*index, operations.operations[2].result.scalar().unwrap().id);
    assert_eq!(
        *length,
        operations.operations[3].result.scalar().unwrap().id
    );
    let mut bounds = std::collections::BTreeSet::from([*obligation]);
    for (measured, read) in length_path.iter().zip(read_path) {
        match (measured, read) {
            (
                terminal_psi::StructuralPathSegment::RuntimeIndex {
                    index: first,
                    obligation: first_bound,
                },
                terminal_psi::StructuralPathSegment::RuntimeIndex {
                    index: second,
                    obligation: second_bound,
                },
            ) => {
                assert_eq!(first, second, "one evaluation supplies both observations");
                assert!(bounds.insert(*first_bound));
                assert!(bounds.insert(*second_bound));
            }
            _ => assert_eq!(measured, read),
        }
    }
}

#[test]
fn static_byte_field_read_reuses_exact_dominating_length() {
    let source = PlaceId::new(7).unwrap();
    let field = StructuralFieldId::new(2).unwrap();
    let path = vec![LoweredElementPathSegment::Static(
        terminal_psi::StructuralPathSegment::Field("room".into()),
    )];
    let mut operations = OperationBuffer::new(0);
    let mut next_value = 1;
    let length = emit_direct_expression(
        &LoweredDirectExpression::ByteSequenceFieldLength {
            source,
            path: path.clone(),
            field,
            scalar_type: count_type(),
        },
        &[],
        &mut next_value,
        &mut operations,
    );
    emit_direct_expression(
        &LoweredDirectExpression::ByteSequenceFieldRead {
            source,
            path,
            field,
            index: Box::new(count(0)),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        },
        &[],
        &mut next_value,
        &mut operations,
    );
    assert_eq!(operations.operations.len(), 3);
    assert!(
        matches!(operations.operations[2].kind, OperationKind::StructuralByteSequenceFieldRead { length: observed, .. } if observed == length)
    );
}

#[test]
fn byte_field_selectors_keep_parameter_type_and_namespace_checks() {
    use crate::emission::expression_validation::validate_direct_parameter_types;
    let expression = LoweredDirectExpression::ByteSequenceFieldLength {
        source: PlaceId::new(7).unwrap(),
        path: vec![LoweredElementPathSegment::RuntimeIndex {
            selector: LoweredDirectExpression::Parameter {
                position: 0,
                scalar_type: count_type(),
            },
            extent: None,
        }],
        field: StructuralFieldId::new(2).unwrap(),
        scalar_type: count_type(),
    };
    assert!(validate_direct_parameter_types(&expression, &[count_type()]).is_ok());
    assert!(validate_direct_parameter_types(&expression, &[]).is_err());
    assert!(validate_direct_parameter_types(&expression, &[ScalarType::Boolean]).is_err());
    let LoweredDirectExpression::ByteSequenceFieldLength { path, .. } = &expression else {
        unreachable!()
    };
    let mut too_many = expression.clone();
    let LoweredDirectExpression::ByteSequenceFieldLength {
        path: duplicated, ..
    } = &mut too_many
    else {
        unreachable!()
    };
    *duplicated = vec![path[0].clone(); TERMINAL_NESTED_SELECTOR_LIMIT + 1];
    assert!(validate_direct_parameter_types(&too_many, &[count_type()]).is_err());
}
