use super::*;
use crate::encoding::{
    PackagePolicyRecoveryLimits,
    encode::{encode_boolean_expression, encode_scalar_expression, encoder::Encoder},
};
use PackageReviewBooleanExpression as Boolean;
use PackageReviewPrimitiveType as Primitive;
use PackageReviewScalarExpression as Scalar;

const PRIMITIVES: [Primitive; 12] = [
    Primitive::Bool,
    Primitive::F32,
    Primitive::F64,
    Primitive::I8,
    Primitive::I16,
    Primitive::I32,
    Primitive::I64,
    Primitive::U8,
    Primitive::U16,
    Primitive::U32,
    Primitive::U64,
    Primitive::Addr,
];

fn field() -> PackageReviewStructuralParameterField {
    PackageReviewStructuralParameterField {
        parameter_position: 2,
        path: vec![
            PackageReviewStructuralPredicatePathSegment::Case("Some".into()),
            PackageReviewStructuralPredicatePathSegment::Field("value".into()),
        ],
    }
}
fn literal() -> Scalar {
    Scalar::IntegerLiteral(PackageReviewIntegerLiteral {
        canonical_text: "9".into(),
        landing: None,
    })
}
fn boolean_bytes(value: &Boolean) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
    encode_boolean_expression(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}
fn scalar_bytes(value: &Scalar) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
    encode_scalar_expression(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}
fn recover_boolean(bytes: &[u8], limits: PackagePolicyRecoveryLimits) -> Result<Boolean, Error> {
    let mut reader = Reader::new(bytes, limits)?;
    let value = boolean_expression(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
fn recover_scalar(bytes: &[u8]) -> Result<Scalar, Error> {
    let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default())?;
    let value = scalar_expression(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
fn boolean_roundtrip(value: Boolean) {
    crate::encoding::encode::text_test_support::meaning(|encoder| {
        encode_boolean_expression(encoder, &value)
    });
    let bytes = boolean_bytes(&value);
    assert_eq!(
        recover_boolean(&bytes, PackagePolicyRecoveryLimits::default()).unwrap(),
        value
    );
    for end in 0..bytes.len() {
        assert!(
            recover_boolean(&bytes[..end], PackagePolicyRecoveryLimits::default()).is_err(),
            "Boolean prefix {end}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        recover_boolean(&trailing, PackagePolicyRecoveryLimits::default()),
        Err(Error::TrailingBytes)
    );
}
fn scalar_roundtrip(value: Scalar) {
    crate::encoding::encode::text_test_support::meaning(|encoder| {
        encode_scalar_expression(encoder, &value)
    });
    let bytes = scalar_bytes(&value);
    assert_eq!(recover_scalar(&bytes).unwrap(), value);
    for end in 0..bytes.len() {
        assert!(
            recover_scalar(&bytes[..end]).is_err(),
            "scalar prefix {end}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(recover_scalar(&trailing), Err(Error::TrailingBytes));
}

#[test]
fn every_boolean_variant_comparison_kind_and_structural_path_roundtrips() {
    let values = [
        Boolean::Constant(true),
        Boolean::Parameter { position: 3 },
        Boolean::Local { position: 9 },
        Boolean::StructuralParameterField {
            parameter_position: 2,
            path: field().path,
        },
        Boolean::Not(Box::new(Boolean::Constant(false))),
        Boolean::Equal {
            left: Box::new(Boolean::Constant(true)),
            right: Box::new(Boolean::Constant(false)),
        },
        Boolean::IntegerComparison {
            kind: PackageReviewIntegerComparisonKind::Equal,
            left: Box::new(literal()),
            right: Box::new(literal()),
        },
        Boolean::IeeeFloatComparison {
            kind: PackageReviewIeeeFloatComparisonKind::Equal,
            primitive_type: Primitive::F64,
            left: field(),
            right: field(),
        },
        Boolean::ByteSequenceEqual {
            left: field(),
            right: field(),
        },
        Boolean::PayloadlessSumEqual {
            left: field(),
            right: field(),
            cases: vec!["First".into(), "Second".into()],
        },
        Boolean::StructuralCaseMembership {
            subject: field(),
            case: "Some".into(),
        },
        Boolean::And {
            left: Box::new(Boolean::Constant(true)),
            right: Box::new(Boolean::Constant(false)),
        },
        Boolean::Or {
            left: Box::new(Boolean::Constant(true)),
            right: Box::new(Boolean::Constant(false)),
        },
    ];
    for (tag, value) in values.into_iter().enumerate() {
        assert_eq!(boolean_bytes(&value)[0], tag as u8);
        boolean_roundtrip(value);
    }
    for kind in [
        PackageReviewIntegerComparisonKind::Equal,
        PackageReviewIntegerComparisonKind::LessThan,
        PackageReviewIntegerComparisonKind::LessOrEqual,
    ] {
        boolean_roundtrip(Boolean::IntegerComparison {
            kind,
            left: Box::new(literal()),
            right: Box::new(literal()),
        });
    }
    for kind in [
        PackageReviewIeeeFloatComparisonKind::Equal,
        PackageReviewIeeeFloatComparisonKind::NotEqual,
    ] {
        for primitive_type in [Primitive::F32, Primitive::F64] {
            boolean_roundtrip(Boolean::IeeeFloatComparison {
                kind,
                primitive_type,
                left: field(),
                right: field(),
            });
        }
    }
}

#[test]
fn every_scalar_variant_and_primitive_tag_roundtrips() {
    let values = [
        Scalar::Parameter {
            position: 0,
            primitive_type: Primitive::U64,
        },
        Scalar::Local {
            position: 4,
            primitive_type: Primitive::I32,
        },
        Scalar::StructuralParameterField {
            parameter_position: 2,
            path: field().path,
            primitive_type: Primitive::U16,
        },
        literal(),
        Scalar::IntegerBinary {
            kind: PackageReviewIntegerBinaryKind::ExactAdd,
            primitive_type: Primitive::U64,
            left: Box::new(literal()),
            right: Box::new(literal()),
        },
        Scalar::IntegerBitwiseNot {
            primitive_type: Primitive::U64,
            operand: Box::new(literal()),
        },
        Scalar::IntegerWiden {
            primitive_type: Primitive::U64,
            operand: Box::new(literal()),
        },
        Scalar::IntegerExactCast {
            primitive_type: Primitive::I64,
            operand: Box::new(literal()),
            range: PackageReviewIntegerRange {
                minimum: "-9".into(),
                maximum: "9".into(),
            },
        },
        Scalar::Boolean(Box::new(Boolean::Constant(true))),
    ];
    for (tag, value) in values.into_iter().enumerate() {
        assert_eq!(scalar_bytes(&value)[0], tag as u8);
        scalar_roundtrip(value);
    }
    for (tag, primitive_type) in PRIMITIVES.into_iter().enumerate() {
        let value = Scalar::Parameter {
            position: 0,
            primitive_type,
        };
        assert_eq!(*scalar_bytes(&value).last().unwrap(), tag as u8);
        scalar_roundtrip(value);
    }
}

#[test]
fn all_integer_binary_kinds_and_literal_landing_domains_roundtrip() {
    use PackageReviewIntegerBinaryKind as Kind;
    let kinds = [
        Kind::ExactAdd,
        Kind::ExactSubtract,
        Kind::ExactMultiply,
        Kind::ExactDivide,
        Kind::ExactRemainder,
        Kind::WrappingDivide,
        Kind::WrappingRemainder,
        Kind::SaturatingDivide,
        Kind::SaturatingRemainder,
        Kind::WrappingAdd,
        Kind::SaturatingAdd,
        Kind::WrappingSubtract,
        Kind::SaturatingSubtract,
        Kind::WrappingMultiply,
        Kind::SaturatingMultiply,
        Kind::BitwiseAnd,
        Kind::BitwiseOr,
        Kind::BitwiseXor,
        Kind::WrappingShiftLeft,
        Kind::WrappingShiftRight,
        Kind::ExactShiftLeft,
        Kind::ExactShiftRight,
    ];
    for (tag, kind) in kinds.into_iter().enumerate() {
        let value = Scalar::IntegerBinary {
            kind,
            primitive_type: Primitive::U64,
            left: Box::new(literal()),
            right: Box::new(literal()),
        };
        assert_eq!(scalar_bytes(&value)[1], tag as u8);
        scalar_roundtrip(value);
    }
    for landed_type in PRIMITIVES {
        for arithmetic_domain in [
            PackageReviewArithmeticDomain::Exact,
            PackageReviewArithmeticDomain::Wrapping,
            PackageReviewArithmeticDomain::Saturating,
            PackageReviewArithmeticDomain::Trapping,
        ] {
            scalar_roundtrip(Scalar::IntegerLiteral(PackageReviewIntegerLiteral {
                canonical_text: "-18446744073709551616".into(),
                landing: Some(PackageReviewIntegerLiteralLanding {
                    landed_type,
                    arithmetic_domain,
                }),
            }));
        }
    }
}

#[test]
fn closed_tags_reject_unknown_nodes_kinds_paths_primitive_names_and_domains() {
    for bytes in [vec![13], vec![0, 2], vec![6, 3], vec![7, 2]] {
        assert_eq!(
            recover_boolean(&bytes, PackagePolicyRecoveryLimits::default()),
            Err(Error::InvalidTag)
        );
    }
    for bytes in [vec![9], vec![4, 22], vec![5, 12]] {
        assert_eq!(recover_scalar(&bytes), Err(Error::InvalidTag));
    }
    let mut path = boolean_bytes(&Boolean::StructuralParameterField {
        parameter_position: 0,
        path: field().path,
    });
    path[1 + 4 + 8] = 2;
    assert_eq!(
        recover_boolean(&path, PackagePolicyRecoveryLimits::default()),
        Err(Error::InvalidTag)
    );
    let value = Scalar::IntegerLiteral(PackageReviewIntegerLiteral {
        canonical_text: "1".into(),
        landing: Some(PackageReviewIntegerLiteralLanding {
            landed_type: Primitive::U64,
            arithmetic_domain: PackageReviewArithmeticDomain::Exact,
        }),
    });
    let bytes = scalar_bytes(&value);
    for marker in [b"u64".as_slice(), b"Exact".as_slice()] {
        let mut changed = bytes.clone();
        let position = changed
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        changed[position] = b'?';
        assert_eq!(recover_scalar(&changed), Err(Error::InvalidTag));
    }
    let mut landing = bytes;
    landing[1 + 8 + 1] = 2;
    assert_eq!(recover_scalar(&landing), Err(Error::InvalidTag));
}

#[test]
fn mixed_boolean_scalar_recursion_shares_exact_depth_element_and_owned_limits() {
    let expression = Boolean::IntegerComparison {
        kind: PackageReviewIntegerComparisonKind::Equal,
        left: Box::new(Scalar::Boolean(Box::new(Boolean::Not(Box::new(
            Boolean::Constant(true),
        ))))),
        right: Box::new(Scalar::IntegerExactCast {
            primitive_type: Primitive::I64,
            operand: Box::new(literal()),
            range: PackageReviewIntegerRange {
                minimum: "-9".into(),
                maximum: "9".into(),
            },
        }),
    };
    let bytes = boolean_bytes(&expression);
    let owned = 3 * std::mem::size_of::<Scalar>() + 2 * std::mem::size_of::<Boolean>() + 4;
    let limits = |elements, owned, depth| {
        PackagePolicyRecoveryLimits::new(bytes.len(), 2, elements, owned, depth)
    };
    assert_eq!(
        recover_boolean(&bytes, limits(6, owned, 4)).unwrap(),
        expression
    );
    for (limits, error) in [
        (limits(5, owned, 4), Error::ElementLimitExceeded),
        (limits(6, owned - 1, 4), Error::AllocationLimitExceeded),
        (limits(6, owned, 3), Error::NestingLimitExceeded),
    ] {
        assert_eq!(recover_boolean(&bytes, limits), Err(error));
    }
    assert_eq!(
        recover_boolean(
            &bytes,
            PackagePolicyRecoveryLimits::new(bytes.len() - 1, 2, 6, owned, 4)
        ),
        Err(Error::InputTooLarge)
    );
    assert_eq!(
        recover_boolean(
            &bytes,
            PackagePolicyRecoveryLimits::new(bytes.len(), 1, 6, owned, 4)
        ),
        Err(Error::FieldTooLarge)
    );
    let mut encoder = Encoder::policy_bounded(bytes.len());
    encode_boolean_expression(&mut encoder, &expression).unwrap();
    assert_eq!(encoder.finish().unwrap(), bytes);
    let mut together = bytes.clone();
    together.extend_from_slice(&bytes);
    let mut reader = Reader::new(
        &together,
        PackagePolicyRecoveryLimits::new(together.len(), 2, 11, 2 * owned, 4),
    )
    .unwrap();
    assert_eq!(boolean_expression(&mut reader).unwrap(), expression);
    assert_eq!(
        boolean_expression(&mut reader),
        Err(Error::ElementLimitExceeded)
    );
}
