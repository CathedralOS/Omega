use super::expressions::project_boolean_expression;

#[test]
fn byte_length_observation_retains_its_structural_root_and_path() {
    let operand = checked_trees::CheckedScalarExpression::StructuralParameterByteLength {
        root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
        path: Vec::new(),
    };
    let expression = checked_trees::CheckedBooleanExpression::IntegerComparison {
        kind: checked_trees::CheckedIntegerComparisonKind::Equal,
        left: Box::new(operand.clone()),
        right: Box::new(operand),
    };
    let projected = project_boolean_expression(&expression).expect("live extent evidence");
    let PackageReviewBooleanExpression::IntegerComparison { left, right, .. } = projected else {
        panic!("comparison retains its operands");
    };
    let expected = PackageReviewScalarExpression::StructuralParameterByteLength {
        parameter_position: 0,
        path: Vec::new(),
    };
    assert_eq!(*left, expected);
    assert_eq!(*right, expected);
}

fn project_scalar(
    value: checked_trees::CheckedScalarExpression,
) -> Option<PackageReviewScalarExpression> {
    let expression = checked_trees::CheckedBooleanExpression::IntegerComparison {
        kind: checked_trees::CheckedIntegerComparisonKind::Equal,
        left: Box::new(value.clone()),
        right: Box::new(value),
    };
    let PackageReviewBooleanExpression::IntegerComparison { left, .. } =
        project_boolean_expression(&expression)?
    else {
        panic!("comparison retains its operands");
    };
    Some(*left)
}

#[test]
fn indexed_evidence_retains_ordered_recursive_selectors_and_carrier() {
    use crate::record::PackageReviewStructuralPredicatePathSegment as ReviewSegment;
    use checked_trees::{
        CheckedScalarExpression as Source, CheckedStorageRoot,
        CheckedStructuralPredicatePathSegment as Segment,
    };
    use typed_trees::types::PrimitiveType;
    let projected = project_scalar(Source::StructuralParameterIndexedRead {
        root: CheckedStorageRoot::Parameter { index: 3 },
        path: vec![Segment::Case("Ready".into()), Segment::Field("rows".into())],
        index: Box::new(Source::IntegerWrappingCast {
            primitive_type: PrimitiveType::U64,
            operand: Box::new(Source::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U32,
            }),
        }),
        inner_indexes: vec![
            Source::StructuralParameterByteLength {
                root: CheckedStorageRoot::Parameter { index: 2 },
                path: vec![Segment::Field("name".into())],
            },
            Source::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U64,
            },
        ],
        element_path: vec![Segment::Field("value".into())],
        primitive_type: PrimitiveType::U16,
    })
    .expect("recursive index evidence");
    let PackageReviewScalarExpression::StructuralParameterIndexedRead {
        parameter_position,
        path,
        index,
        inner_indexes,
        element_path,
        primitive_type,
    } = projected
    else {
        panic!("indexed read stays indexed");
    };
    assert_eq!(parameter_position, 3);
    assert_eq!(
        path,
        vec![
            ReviewSegment::Case("Ready".into()),
            ReviewSegment::Field("rows".into())
        ]
    );
    assert!(matches!(
        *index,
        PackageReviewScalarExpression::IntegerWrappingCast {
            primitive_type: PackageReviewPrimitiveType::U64,
            ..
        }
    ));
    assert_eq!(
        inner_indexes,
        vec![
            PackageReviewScalarExpression::StructuralParameterByteLength {
                parameter_position: 2,
                path: vec![ReviewSegment::Field("name".into())],
            },
            PackageReviewScalarExpression::Parameter {
                position: 0,
                primitive_type: PackageReviewPrimitiveType::U64
            }
        ]
    );
    assert_eq!(element_path, vec![ReviewSegment::Field("value".into())]);
    assert_eq!(primitive_type, PackageReviewPrimitiveType::U16);
}

#[test]
fn scalar_conversion_policies_remain_distinct_inside_evidence() {
    use checked_trees::CheckedScalarExpression as Source;
    use typed_trees::types::PrimitiveType;
    let operand = Box::new(Source::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U32,
    });
    let wrapping = project_scalar(Source::IntegerWrappingCast {
        primitive_type: PrimitiveType::U16,
        operand: operand.clone(),
    })
    .unwrap();
    let saturating = project_scalar(Source::IntegerSaturatingCast {
        primitive_type: PrimitiveType::U16,
        operand: operand.clone(),
    })
    .unwrap();
    let trapping = project_scalar(Source::IntegerTrappingCast {
        primitive_type: PrimitiveType::U16,
        operand: operand.clone(),
    })
    .unwrap();
    let view = project_scalar(Source::RepresentationView {
        primitive_type: PrimitiveType::F32,
        operand,
    })
    .unwrap();
    assert!(matches!(
        wrapping,
        PackageReviewScalarExpression::IntegerWrappingCast { .. }
    ));
    assert!(matches!(
        saturating,
        PackageReviewScalarExpression::IntegerSaturatingCast { .. }
    ));
    assert!(matches!(
        trapping,
        PackageReviewScalarExpression::IntegerTrappingCast { .. }
    ));
    assert!(
        matches!(view, PackageReviewScalarExpression::RepresentationView { primitive_type: PackageReviewPrimitiveType::F32, operand }
        if matches!(*operand, PackageReviewScalarExpression::Parameter { primitive_type: PackageReviewPrimitiveType::U32, .. }))
    );
}

#[test]
fn private_storage_and_erased_operands_cannot_become_public_entry_roots() {
    use checked_trees::{CheckedScalarExpression as Source, CheckedStorageRoot};
    use typed_trees::types::PrimitiveType;
    for root in [
        CheckedStorageRoot::ViewLocal {
            symbol: Default::default(),
        },
        CheckedStorageRoot::OwnedLocal {
            symbol: Default::default(),
        },
    ] {
        assert!(
            project_scalar(Source::StructuralParameterByteLength {
                root,
                path: Vec::new()
            })
            .is_none()
        );
        assert!(
            project_scalar(Source::StructuralParameterIndexedRead {
                root,
                path: Vec::new(),
                index: Box::new(Source::Parameter {
                    position: 0,
                    primitive_type: PrimitiveType::U64
                }),
                inner_indexes: Vec::new(),
                element_path: Vec::new(),
                primitive_type: PrimitiveType::U8,
            })
            .is_none()
        );
    }
    for operand in [
        Source::ErasedParameter {
            position: 0,
            primitive_type: PrimitiveType::U32,
        },
        Source::StorageRead {
            symbol: Default::default(),
            primitive_type: PrimitiveType::U32,
        },
    ] {
        assert!(
            project_scalar(Source::IntegerTrappingCast {
                primitive_type: PrimitiveType::U8,
                operand: Box::new(operand)
            })
            .is_none()
        );
    }
}

use crate::record::{
    PackageReviewArithmeticDomain, PackageReviewBooleanExpression,
    PackageReviewIntegerComparisonKind, PackageReviewPrimitiveType, PackageReviewScalarExpression,
};

#[test]
fn structural_runtime_requirement_crosses_as_closed_review_evidence() {
    let source = checked_trees::CheckedBooleanExpression::IntegerComparison {
        kind: checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
        left: Box::new(checked_trees::CheckedScalarExpression::IntegerLiteral {
            literal: numerics::literals::IntegerLiteral::from_value(7).with_landing(
                numerics::literals::IntegerLanding {
                    landed_type: numerics::literals::LandedIntegerType::U32,
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                },
            ),
        }),
        right: Box::new(checked_trees::CheckedScalarExpression::IntegerExactCast {
            primitive_type: typed_trees::types::PrimitiveType::U32,
            operand: Box::new(checked_trees::CheckedScalarExpression::Parameter {
                position: 3,
                primitive_type: typed_trees::types::PrimitiveType::U16,
            }),
            range: checked_trees::CheckedIntegerRange {
                minimum: numerics::bignum::BigInt::from_u64(0),
                maximum: numerics::bignum::BigInt::from_u64(u32::MAX as u64),
            },
        }),
    };

    let PackageReviewBooleanExpression::IntegerComparison { kind, left, right } =
        project_boolean_expression(&source).expect("exact scalar expressions remain projectable")
    else {
        panic!("integer comparison must retain its closed review shape")
    };
    assert_eq!(kind, PackageReviewIntegerComparisonKind::LessOrEqual);
    assert!(matches!(
        left.as_ref(),
        PackageReviewScalarExpression::IntegerLiteral(literal)
            if literal.canonical_text() == "7"
                && literal.landing().is_some_and(|landing|
                    landing.landed_type() == PackageReviewPrimitiveType::U32
                        && landing.arithmetic_domain() == PackageReviewArithmeticDomain::Exact)
    ));
    assert!(matches!(
        right.as_ref(),
        PackageReviewScalarExpression::IntegerExactCast {
            primitive_type: PackageReviewPrimitiveType::U32,
            operand,
            range,
        } if range.minimum() == "0"
            && range.maximum() == u32::MAX.to_string()
            && matches!(
                operand.as_ref(),
                PackageReviewScalarExpression::Parameter {
                    position: 3,
                    primitive_type: PackageReviewPrimitiveType::U16,
                }
            )
    ));
}

#[test]
fn mathematical_requirement_without_review_vocabulary_rejects_the_projection() {
    let mut terms = arena::Arena::new();
    let root = terms.insert(checked_trees::CheckedIntegerMathTerm::Embed {
        primitive_type: typed_trees::types::PrimitiveType::U64,
        operand: checked_trees::CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: typed_trees::types::PrimitiveType::U64,
        },
    });
    let term = checked_trees::CheckedIntegerMathExpression { terms, root };
    let comparison = checked_trees::CheckedBooleanExpression::IntegerMathComparison {
        kind: checked_trees::CheckedIntegerComparisonKind::Equal,
        left: term.clone(),
        right: term,
    };
    assert_eq!(project_boolean_expression(&comparison), None);
    // A supported conjunct cannot make an unrepresentable requirement vanish.
    let conjunction = checked_trees::CheckedBooleanExpression::And {
        left: Box::new(checked_trees::CheckedBooleanExpression::Constant(true)),
        right: Box::new(comparison),
    };
    assert_eq!(project_boolean_expression(&conjunction), None);
}
