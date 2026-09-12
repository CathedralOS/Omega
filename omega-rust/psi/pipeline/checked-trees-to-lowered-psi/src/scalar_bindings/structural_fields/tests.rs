use super::*;
use checked_trees::CheckedStructuralPredicatePathSegment;
use terminal_psi::BindingRelevance;

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
            OperationKind::IntegerStructuralField { source, field }
                if source == PlaceId::new(11).unwrap() && field == StructuralFieldId::new(7).unwrap()));
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
