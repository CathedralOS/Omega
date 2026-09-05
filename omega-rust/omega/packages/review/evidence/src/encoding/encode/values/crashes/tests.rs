use super::*;

fn encoded_boolean(expression: &PackageReviewBooleanExpression) -> Vec<u8> {
    let mut encoder = Encoder::bounded(1024);
    encode_boolean_expression(&mut encoder, expression).expect("bounded Boolean encoding");
    encoder.finish().expect("complete Boolean encoding")
}

fn encoded_scalar(expression: &PackageReviewScalarExpression) -> Vec<u8> {
    let mut encoder = Encoder::bounded(1024);
    encode_scalar_expression(&mut encoder, expression).expect("bounded scalar encoding");
    encoder.finish().expect("complete scalar encoding")
}

#[test]
fn closed_boolean_nodes_retain_the_existing_canonical_tags() {
    let expression = PackageReviewBooleanExpression::And {
        left: Box::new(PackageReviewBooleanExpression::Parameter { position: 7 }),
        right: Box::new(PackageReviewBooleanExpression::Not(Box::new(
            PackageReviewBooleanExpression::Local { position: 9 },
        ))),
    };
    let mut expected = vec![11, 1];
    expected.extend_from_slice(&7u64.to_le_bytes());
    expected.extend_from_slice(&[4, 2]);
    expected.extend_from_slice(&9u64.to_le_bytes());
    assert_eq!(encoded_boolean(&expression), expected);
}

#[test]
fn closed_integer_literal_retains_the_existing_canonical_bytes() {
    let expression =
        PackageReviewScalarExpression::IntegerLiteral(crate::record::PackageReviewIntegerLiteral {
            canonical_text: "7".to_owned(),
            landing: Some(crate::record::PackageReviewIntegerLiteralLanding {
                landed_type: PackageReviewPrimitiveType::U32,
                arithmetic_domain: PackageReviewArithmeticDomain::Exact,
            }),
        });
    let mut expected = vec![3];
    expected.extend_from_slice(&1u64.to_le_bytes());
    expected.extend_from_slice(b"7");
    expected.push(1);
    expected.extend_from_slice(&3u64.to_le_bytes());
    expected.extend_from_slice(b"u32");
    expected.extend_from_slice(&5u64.to_le_bytes());
    expected.extend_from_slice(b"Exact");
    assert_eq!(encoded_scalar(&expression), expected);
}

#[test]
fn closed_operation_vocabularies_retain_every_existing_tag() {
    let comparisons = [
        PackageReviewIntegerComparisonKind::Equal,
        PackageReviewIntegerComparisonKind::LessThan,
        PackageReviewIntegerComparisonKind::LessOrEqual,
    ];
    assert_eq!(comparisons.map(integer_comparison_tag), [0, 1, 2],);
    let binaries = [
        PackageReviewIntegerBinaryKind::ExactAdd,
        PackageReviewIntegerBinaryKind::ExactSubtract,
        PackageReviewIntegerBinaryKind::ExactMultiply,
        PackageReviewIntegerBinaryKind::ExactDivide,
        PackageReviewIntegerBinaryKind::ExactRemainder,
        PackageReviewIntegerBinaryKind::WrappingDivide,
        PackageReviewIntegerBinaryKind::WrappingRemainder,
        PackageReviewIntegerBinaryKind::SaturatingDivide,
        PackageReviewIntegerBinaryKind::SaturatingRemainder,
        PackageReviewIntegerBinaryKind::WrappingAdd,
        PackageReviewIntegerBinaryKind::SaturatingAdd,
        PackageReviewIntegerBinaryKind::WrappingSubtract,
        PackageReviewIntegerBinaryKind::SaturatingSubtract,
        PackageReviewIntegerBinaryKind::WrappingMultiply,
        PackageReviewIntegerBinaryKind::SaturatingMultiply,
        PackageReviewIntegerBinaryKind::BitwiseAnd,
        PackageReviewIntegerBinaryKind::BitwiseOr,
        PackageReviewIntegerBinaryKind::BitwiseXor,
        PackageReviewIntegerBinaryKind::WrappingShiftLeft,
        PackageReviewIntegerBinaryKind::WrappingShiftRight,
        PackageReviewIntegerBinaryKind::ExactShiftLeft,
        PackageReviewIntegerBinaryKind::ExactShiftRight,
    ];
    assert_eq!(
        binaries.map(integer_binary_tag),
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21
        ],
    );
}
