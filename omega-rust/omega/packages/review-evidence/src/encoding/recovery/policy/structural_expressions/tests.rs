use super::Error;
use crate::encoding::recovery::policy::structural_expressions::Reader;
use crate::encoding::recovery::policy::structural_expressions::boolean_expression;
use crate::encoding::recovery::policy::structural_expressions::scalar_expression;
use crate::encoding::{
    PackagePolicyRecoveryLimits,
    encode::{encode_boolean_expression, encode_scalar_expression, encoder::Encoder},
};
use crate::record::PackageReviewArithmeticDomain;
use crate::record::PackageReviewBooleanExpression as Boolean;
use crate::record::PackageReviewIeeeFloatComparisonKind;
use crate::record::PackageReviewIntegerBinaryKind;
use crate::record::PackageReviewIntegerComparisonKind;
use crate::record::PackageReviewIntegerLiteral;
use crate::record::PackageReviewIntegerLiteralLanding;
use crate::record::PackageReviewIntegerRange;
use crate::record::PackageReviewPrimitiveType as Primitive;
use crate::record::PackageReviewScalarExpression as Scalar;
use crate::record::PackageReviewStructuralParameterField;
use crate::record::PackageReviewStructuralPredicatePathSegment;

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

pub(in crate::encoding::recovery) fn field() -> PackageReviewStructuralParameterField {
    PackageReviewStructuralParameterField {
        parameter_position: 2,
        path: vec![
            PackageReviewStructuralPredicatePathSegment::Case("Some".into()),
            PackageReviewStructuralPredicatePathSegment::Field("value".into()),
            PackageReviewStructuralPredicatePathSegment::FixedIndex(7),
        ],
    }
}

#[test]
pub(in crate::encoding::recovery) fn fixed_index_paths_retain_numeric_identity_and_reject_malformed_payloads()
 {
    use PackageReviewStructuralPredicatePathSegment as Segment;
    let indexed = |element_index| Boolean::StructuralParameterField {
        parameter_position: 0,
        path: vec![Segment::FixedIndex(element_index)],
    };
    for element_index in [0, 1, u64::from(u32::MAX) + 1, u64::MAX] {
        let value = indexed(element_index);
        let bytes = boolean_bytes(&value);
        let mut expected = vec![3];
        expected.extend_from_slice(&0_u32.to_le_bytes());
        expected.extend_from_slice(&1_u64.to_le_bytes());
        expected.push(2);
        expected.extend_from_slice(&element_index.to_le_bytes());
        assert_eq!(bytes, expected, "fixed-width numeric path encoding");
        boolean_roundtrip(value);
    }
    let first = boolean_bytes(&indexed(0));
    let second = boolean_bytes(&indexed(1));
    assert_ne!(first, second);
    let named = boolean_bytes(&Boolean::StructuralParameterField {
        parameter_position: 0,
        path: vec![Segment::Field("1".into())],
    });
    assert_ne!(second, named, "an index is not a field spelling");
    // Segment tags 0..=3 name a field, case, fixed index and runtime index;
    // 4 is the first tag no path segment declares.
    let mut unknown = second.clone();
    unknown[13] = 4;
    assert_eq!(
        recover_boolean(&unknown, PackagePolicyRecoveryLimits::default()),
        Err(Error::InvalidTag)
    );
    for retained_bytes in 0..8 {
        assert!(
            recover_boolean(
                &second[..14 + retained_bytes],
                PackagePolicyRecoveryLimits::default()
            )
            .is_err()
        );
    }
}
pub(in crate::encoding::recovery) fn literal() -> Scalar {
    Scalar::IntegerLiteral(PackageReviewIntegerLiteral {
        canonical_text: "9".into(),
        landing: None,
    })
}
pub(in crate::encoding::recovery) fn boolean_bytes(value: &Boolean) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
    encode_boolean_expression(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}
pub(in crate::encoding::recovery) fn scalar_bytes(value: &Scalar) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
    encode_scalar_expression(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}
pub(in crate::encoding::recovery) fn recover_boolean(
    bytes: &[u8],
    limits: PackagePolicyRecoveryLimits,
) -> Result<Boolean, Error> {
    let mut reader = Reader::new(bytes, limits)?;
    let value = boolean_expression(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
pub(in crate::encoding::recovery) fn recover_scalar(bytes: &[u8]) -> Result<Scalar, Error> {
    let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default())?;
    let value = scalar_expression(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
pub(in crate::encoding::recovery) fn boolean_roundtrip(value: Boolean) {
    crate::encoding::encode::text_test_support::meaning(|encoder| {
        encode_boolean_expression(encoder, &value)
    });
    let bytes = boolean_bytes(&value);
    assert_eq!(
        recover_boolean(&bytes, PackagePolicyRecoveryLimits::default()).unwrap(),
        value
    );
    for end in [0, bytes.len() / 2, bytes.len() - 1] {
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
pub(in crate::encoding::recovery) fn scalar_roundtrip(value: Scalar) {
    crate::encoding::encode::text_test_support::meaning(|encoder| {
        encode_scalar_expression(encoder, &value)
    });
    let bytes = scalar_bytes(&value);
    assert_eq!(recover_scalar(&bytes).unwrap(), value);
    for end in [0, bytes.len() / 2, bytes.len() - 1] {
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
pub(in crate::encoding::recovery) fn every_boolean_variant_comparison_kind_and_structural_path_roundtrips()
 {
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
        PackageReviewIeeeFloatComparisonKind::LessThan,
        PackageReviewIeeeFloatComparisonKind::LessOrEqual,
        PackageReviewIeeeFloatComparisonKind::NotLessThan,
        PackageReviewIeeeFloatComparisonKind::NotLessOrEqual,
    ] {
        for primitive_type in [Primitive::F32, Primitive::F64] {
            boolean_roundtrip(Boolean::IeeeFloatComparison {
                kind,
                primitive_type,
                left: field(),
                right: field(),
            });
        }
        boolean_roundtrip(Boolean::ScalarIeeeFloatComparison {
            kind,
            left: Box::new(Scalar::Parameter {
                position: 0,
                primitive_type: Primitive::F32,
            }),
            right: Box::new(Scalar::Float32Literal(0x4300_0000)),
        });
        boolean_roundtrip(Boolean::ScalarIeeeFloatComparison {
            kind,
            left: Box::new(Scalar::Float64Literal(0x8000_0000_0000_0000)),
            right: Box::new(Scalar::Float64Literal(0x7ff8_0000_0000_002a)),
        });
    }
}

#[test]
pub(in crate::encoding::recovery) fn every_scalar_variant_and_primitive_tag_roundtrips() {
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

fn indexed_scalar() -> Scalar {
    use PackageReviewStructuralPredicatePathSegment as Segment;
    Scalar::StructuralParameterIndexedRead {
        parameter_position: 0,
        path: vec![Segment::Field("items".into())],
        index: Box::new(Scalar::IntegerWrappingCast {
            primitive_type: Primitive::U64,
            operand: Box::new(Scalar::Parameter {
                position: 1,
                primitive_type: Primitive::U32,
            }),
        }),
        inner_indexes: vec![
            Scalar::Parameter {
                position: 2,
                primitive_type: Primitive::U64,
            },
            Scalar::Parameter {
                position: 3,
                primitive_type: Primitive::U64,
            },
        ],
        element_path: vec![Segment::Case("Some".into()), Segment::Field("value".into())],
        primitive_type: Primitive::U8,
    }
}

#[test]
pub(in crate::encoding::recovery) fn scalar_observations_and_conversion_policies_roundtrip_with_distinct_tags()
 {
    let operand = || Box::new(indexed_scalar());
    let values = [
        Scalar::IntegerWrappingCast {
            primitive_type: Primitive::U16,
            operand: operand(),
        },
        Scalar::IntegerSaturatingCast {
            primitive_type: Primitive::U16,
            operand: operand(),
        },
        Scalar::IntegerTrappingCast {
            primitive_type: Primitive::U16,
            operand: operand(),
        },
        Scalar::RepresentationView {
            primitive_type: Primitive::U16,
            operand: operand(),
        },
        Scalar::StructuralParameterByteLength {
            parameter_position: 2,
            path: field().path,
        },
        indexed_scalar(),
    ];
    let mut encodings = Vec::new();
    for (offset, value) in values.into_iter().enumerate() {
        let bytes = scalar_bytes(&value);
        assert_eq!(bytes[0], 11 + offset as u8);
        assert!(!encodings.contains(&bytes));
        encodings.push(bytes);
        scalar_roundtrip(value);
    }
    scalar_roundtrip(Scalar::StructuralParameterByteLength {
        parameter_position: 0,
        path: Vec::new(),
    });
    boolean_roundtrip(Boolean::IntegerComparison {
        kind: PackageReviewIntegerComparisonKind::LessThan,
        left: Box::new(indexed_scalar()),
        right: Box::new(Scalar::StructuralParameterByteLength {
            parameter_position: 0,
            path: Vec::new(),
        }),
    });
}

#[test]
pub(in crate::encoding::recovery) fn indexed_scalar_coordinates_and_index_order_are_canonical_identity()
 {
    use PackageReviewStructuralPredicatePathSegment as Segment;
    let original = indexed_scalar();
    let bytes = scalar_bytes(&original);
    let mut changed_values = vec![original.clone(); 6];
    for (coordinate, changed) in changed_values.iter_mut().enumerate() {
        let Scalar::StructuralParameterIndexedRead {
            parameter_position,
            path,
            index,
            inner_indexes,
            element_path,
            primitive_type,
        } = changed
        else {
            panic!("indexed scalar fixture");
        };
        match coordinate {
            0 => *parameter_position = 1,
            1 => *path = vec![Segment::Field("other".into())],
            2 => *index = Box::new(literal()),
            3 => inner_indexes.swap(0, 1),
            4 => *element_path = vec![Segment::Field("other".into())],
            5 => *primitive_type = Primitive::I8,
            _ => panic!("one control per retained coordinate"),
        }
        assert_ne!(scalar_bytes(changed), bytes);
        scalar_roundtrip(changed.clone());
    }
    let length = |parameter_position, path| Scalar::StructuralParameterByteLength {
        parameter_position,
        path,
    };
    let whole = scalar_bytes(&length(0, Vec::new()));
    assert_ne!(whole, scalar_bytes(&length(1, Vec::new())));
    assert_ne!(
        whole,
        scalar_bytes(&length(0, vec![Segment::Field("bytes".into())]))
    );
    assert_ne!(
        whole,
        scalar_bytes(&Scalar::Parameter {
            position: 0,
            primitive_type: Primitive::U64,
        })
    );
}

#[test]
pub(in crate::encoding::recovery) fn scalar_observation_recovery_rejects_malformed_payloads() {
    for tag in 11..=14 {
        assert_eq!(recover_scalar(&[tag, 12]), Err(Error::InvalidTag));
        assert_eq!(recover_scalar(&[tag, 10]), Err(Error::UnexpectedEnd));
        assert_eq!(recover_scalar(&[tag, 10, 17]), Err(Error::InvalidTag));
    }
    let simple = Scalar::StructuralParameterIndexedRead {
        parameter_position: 0,
        path: Vec::new(),
        index: Box::new(Scalar::Parameter {
            position: 1,
            primitive_type: Primitive::U64,
        }),
        inner_indexes: Vec::new(),
        element_path: Vec::new(),
        primitive_type: Primitive::U8,
    };
    let bytes = scalar_bytes(&simple);
    for end in 0..bytes.len() {
        assert!(
            recover_scalar(&bytes[..end]).is_err(),
            "indexed prefix {end}"
        );
    }
    let mut unknown_index = bytes.clone();
    unknown_index[13] = 17;
    assert_eq!(recover_scalar(&unknown_index), Err(Error::InvalidTag));
    let mut unknown_carrier = bytes.clone();
    *unknown_carrier.last_mut().unwrap() = 12;
    assert_eq!(recover_scalar(&unknown_carrier), Err(Error::InvalidTag));
    let mut excessive_inner_indexes = bytes;
    excessive_inner_indexes[23..31].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        recover_scalar(&excessive_inner_indexes),
        Err(Error::ElementLimitExceeded)
    );

    let mut unknown_path = vec![15];
    unknown_path.extend_from_slice(&0_u32.to_le_bytes());
    unknown_path.extend_from_slice(&1_u64.to_le_bytes());
    unknown_path.push(4);
    unknown_path.extend_from_slice(&0_u64.to_le_bytes());
    assert_eq!(recover_scalar(&unknown_path), Err(Error::InvalidTag));
}

#[test]
pub(in crate::encoding::recovery) fn indexed_scalar_recovery_charges_nested_indices_paths_and_storage()
 {
    let value = indexed_scalar();
    let bytes = scalar_bytes(&value);
    let owned = 4 * std::mem::size_of::<Scalar>()
        + 3 * std::mem::size_of::<PackageReviewStructuralPredicatePathSegment>()
        + 14;
    let limits = |elements, owned, depth| {
        PackagePolicyRecoveryLimits::new(bytes.len(), 5, elements, owned, depth)
    };
    let recover = |limits| {
        let mut reader = Reader::new(&bytes, limits)?;
        let value = scalar_expression(&mut reader)?;
        reader.finish()?;
        Ok(value)
    };
    assert_eq!(recover(limits(10, owned, 3)), Ok(value.clone()));
    for (limits, error) in [
        (limits(9, owned, 3), Error::ElementLimitExceeded),
        (limits(10, owned - 1, 3), Error::AllocationLimitExceeded),
        (limits(10, owned, 2), Error::NestingLimitExceeded),
        (
            PackagePolicyRecoveryLimits::new(bytes.len(), 4, 10, owned, 3),
            Error::FieldTooLarge,
        ),
        (
            PackagePolicyRecoveryLimits::new(bytes.len() - 1, 5, 10, owned, 3),
            Error::InputTooLarge,
        ),
    ] {
        assert_eq!(recover(limits), Err(error));
    }
    let mut encoder = Encoder::policy_bounded(bytes.len());
    encode_scalar_expression(&mut encoder, &value).unwrap();
    assert_eq!(encoder.finish().unwrap(), bytes);
}

#[test]
pub(in crate::encoding::recovery) fn all_integer_binary_kinds_and_literal_landing_domains_roundtrip()
 {
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
        Kind::TrappingShiftLeft,
        Kind::TrappingShiftRight,
        Kind::TrappingAdd,
        Kind::TrappingSubtract,
        Kind::TrappingMultiply,
        Kind::TrappingDivide,
        Kind::TrappingRemainder,
        Kind::SaturatingShiftLeft,
        Kind::SaturatingShiftRight,
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
pub(in crate::encoding::recovery) fn closed_tags_reject_unknown_nodes_kinds_paths_primitive_names_and_domains()
 {
    // The first unknown node tag, an unknown Constant byte, integer
    // comparison kind, and IEEE comparison kind under both comparison tags.
    for bytes in [vec![14], vec![0, 2], vec![6, 3], vec![7, 6], vec![13, 6]] {
        assert_eq!(
            recover_boolean(&bytes, PackagePolicyRecoveryLimits::default()),
            Err(Error::InvalidTag)
        );
    }
    for bytes in [vec![17], vec![4, 31], vec![5, 12]] {
        assert_eq!(recover_scalar(&bytes), Err(Error::InvalidTag));
    }
    for kind in [29, 30] {
        assert_eq!(recover_scalar(&[4, kind]), Err(Error::UnexpectedEnd));
    }
    let mut path = boolean_bytes(&Boolean::StructuralParameterField {
        parameter_position: 0,
        path: field().path,
    });
    path[1 + 4 + 8] = 3;
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
pub(in crate::encoding::recovery) fn mixed_boolean_scalar_recursion_shares_exact_depth_element_and_owned_limits()
 {
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
