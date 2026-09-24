use super::super::{
    CheckedBooleanExpression, CheckedScalarExpression, ScalarBindings, StructuralMultiplicity,
    terminal_scalar_type,
};
use super::{
    PlaceId, ScalarType, StructuralAccess, StructuralFieldId, StructuralFieldType,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::emit_direct_expression;
use checked_trees::CheckedStructuralPredicatePathSegment;
use checked_trees::types::PrimitiveType;
use terminal_psi::OperationKind;
use terminal_psi::{BindingRelevance, StructuralFieldDeclaration};

fn bindings(access: StructuralAccess, relevance: BindingRelevance) -> ScalarBindings {
    let scalar_type = terminal_scalar_type(PrimitiveType::U64).unwrap();
    let declaration = StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "Counter".to_owned(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(7).unwrap(),
                identity: "value".to_owned(),
                relevance,
                field_type: StructuralFieldType::Scalar(scalar_type),
            }],
        },
    };
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(11).unwrap(),
        position: 0,
        is_self: false,
        structural_type: declaration.id,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    ScalarBindings::new(0)
        .with_structural_parameters(&[(3, parameter)])
        .with_structural_observations(&[declaration])
}

fn observation(
    position: u32,
    primitive_type: PrimitiveType,
    identity: &str,
) -> CheckedScalarExpression {
    CheckedScalarExpression::StructuralParameterField {
        parameter_position: position,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            identity.to_owned(),
        )],
        primitive_type,
    }
}

#[test]
fn repeated_field_reads_emit_distinct_current_observations() {
    let bindings = bindings(StructuralAccess::MutableBorrow, BindingRelevance::Relevant);
    let expression = bindings
        .expression(&observation(3, PrimitiveType::U64, "value"))
        .unwrap();
    let mut operations = OperationBuffer::new(0);
    let mut next_value = 1;
    let first = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
    let second = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
    assert_ne!(
        first, second,
        "field reads are not cached as descriptor metadata"
    );
    assert_eq!(operations.len(), 2);
    for operation in operations.iter() {
        assert!(matches!(operation.kind,
            OperationKind::IntegerStructuralField { source, field, ref path }
                if path.is_empty() && source == PlaceId::new(11).unwrap() && field == StructuralFieldId::new(7).unwrap()));
    }
}

#[test]
fn field_reads_require_exact_source_position_type_name_and_readable_relevance() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let bindings = bindings(access, BindingRelevance::Relevant);
        assert!(
            bindings
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_ok()
        );
        for expression in [
            observation(0, PrimitiveType::U64, "value"),
            observation(3, PrimitiveType::U8, "value"),
            observation(3, PrimitiveType::U64, "missing"),
        ] {
            assert!(bindings.expression(&expression).is_err());
        }
        let mut nested = observation(3, PrimitiveType::U64, "value");
        if let CheckedScalarExpression::StructuralParameterField { path, .. } = &mut nested {
            path.push(CheckedStructuralPredicatePathSegment::Field(
                "value".to_owned(),
            ));
        }
        assert!(bindings.expression(&nested).is_err());
        assert!(
            bindings
                .with_structural_parameters(&[])
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_err()
        );
    }
    for (access, relevance) in [
        (
            StructuralAccess::WriteOnlyBorrow,
            BindingRelevance::Relevant,
        ),
        (StructuralAccess::MutableBorrow, BindingRelevance::Erased),
    ] {
        assert!(
            bindings(access, relevance)
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_err()
        );
    }
}

fn nested_declarations(scalar: ScalarType) -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "Root".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(7).unwrap(),
                    identity: "child".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(StructuralTypeId::new(2).unwrap()),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(2).unwrap(),
            identity: "Child".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(7).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(scalar),
                }],
            },
        },
    ]
}

fn nested_observation(boolean: bool) -> CheckedScalarExpression {
    let path = ["child", "value"]
        .map(|identity| CheckedStructuralPredicatePathSegment::Field(identity.into()))
        .to_vec();
    if boolean {
        CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 3,
                path,
            },
        ))
    } else {
        CheckedScalarExpression::StructuralParameterField {
            parameter_position: 3,
            path,
            primitive_type: PrimitiveType::U64,
        }
    }
}

#[test]
fn nested_scalar_reads_emit_original_root_and_declaration_local_carrier_ids() {
    for boolean in [false, true] {
        let scalar = if boolean {
            ScalarType::Boolean
        } else {
            terminal_scalar_type(PrimitiveType::U64).unwrap()
        };
        let declarations = nested_declarations(scalar);
        let bindings = bindings(StructuralAccess::MutableBorrow, BindingRelevance::Relevant)
            .with_structural_observations(&declarations);
        let expression = bindings.expression(&nested_observation(boolean)).unwrap();
        let mut operations = OperationBuffer::new(0);
        let mut next_value = 1;
        let first = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
        let second = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
        assert_ne!(first, second);
        assert_eq!(operations.len(), 2);
        for operation in operations.iter() {
            let (source, path, field) = match &operation.kind {
                OperationKind::IntegerStructuralField {
                    source,
                    path,
                    field,
                }
                | OperationKind::BooleanStructuralField {
                    source,
                    path,
                    field,
                } => (source, path, field),
                other => panic!("expected nested read, got {other:?}"),
            };
            assert_eq!(*source, PlaceId::new(11).unwrap());
            assert_eq!(*field, StructuralFieldId::new(7).unwrap());
            assert_eq!(
                path,
                &[semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    *field
                )]
            );
        }
        assert!(
            bindings
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_err()
        );
        for mutation in [
            "erased carrier",
            "scalar carrier",
            "missing child",
            "duplicate type",
            "duplicate name",
            "duplicate id",
            "cycle",
            "wrong leaf",
        ] {
            let mut changed = declarations.clone();
            match mutation {
                "missing child" => {
                    changed.pop();
                }
                "duplicate type" => changed.push(changed[1].clone()),
                "wrong leaf" => {
                    let StructuralTypeShape::Record { fields } = &mut changed[1].shape else {
                        unreachable!()
                    };
                    fields[0].field_type = StructuralFieldType::Scalar(
                        terminal_scalar_type(PrimitiveType::U8).unwrap(),
                    );
                }
                _ => {
                    let StructuralTypeShape::Record { fields } = &mut changed[0].shape else {
                        unreachable!()
                    };
                    match mutation {
                        "erased carrier" => fields[0].relevance = BindingRelevance::Erased,
                        "scalar carrier" => {
                            fields[0].field_type = StructuralFieldType::Scalar(scalar)
                        }
                        "cycle" => {
                            fields[0].field_type =
                                StructuralFieldType::Structural(StructuralTypeId::new(1).unwrap())
                        }
                        "duplicate name" => {
                            let mut duplicate = fields[0].clone();
                            duplicate.id = StructuralFieldId::new(8).unwrap();
                            fields.push(duplicate);
                        }
                        "duplicate id" => {
                            let mut duplicate = fields[0].clone();
                            duplicate.identity = "other".into();
                            fields.push(duplicate);
                        }
                        _ => unreachable!(),
                    }
                }
            }
            let changed = bindings.clone().with_structural_observations(&changed);
            assert!(
                changed.expression(&nested_observation(boolean)).is_err(),
                "{boolean} {mutation}"
            );
        }
    }
}
