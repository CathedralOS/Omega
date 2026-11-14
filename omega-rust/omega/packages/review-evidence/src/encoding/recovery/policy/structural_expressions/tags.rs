// Inverses of the shared structural-expression closed vocabulary for primitive types and integer binary operators.

use crate::encoding::recovery::policy::structural_expressions::Reader;
use crate::record::PackageReviewIntegerBinaryKind;
use crate::record::PackageReviewPrimitiveType;

use super::Error;
use numerics::integer_carrier::{FixedInteger, IntegerSign};
pub(in crate::encoding::recovery) fn primitive_type(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPrimitiveType, Error> {
    use PackageReviewPrimitiveType as Primitive;
    Ok(super::super::tag!(reader => {
        0 => Primitive::Bool,
        1 => Primitive::F32,
        2 => Primitive::F64,
        3 => {
            let sign = if reader.boolean()? { IntegerSign::Signed } else { IntegerSign::Unsigned };
            let carrier = FixedInteger::new(sign, reader.u64()?).ok_or(Error::InvalidValue)?;
            Primitive::Integer(carrier)
        },
        11 => Primitive::Addr,
    }))
}

pub(in crate::encoding::recovery) fn integer_binary(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewIntegerBinaryKind, Error> {
    use PackageReviewIntegerBinaryKind as Kind;
    Ok(super::super::tag!(reader => {
        0 => Kind::ExactAdd,
        1 => Kind::ExactSubtract,
        2 => Kind::ExactMultiply,
        3 => Kind::ExactDivide,
        4 => Kind::ExactRemainder,
        5 => Kind::WrappingDivide,
        6 => Kind::WrappingRemainder,
        7 => Kind::SaturatingDivide,
        8 => Kind::SaturatingRemainder,
        9 => Kind::WrappingAdd,
        10 => Kind::SaturatingAdd,
        11 => Kind::WrappingSubtract,
        12 => Kind::SaturatingSubtract,
        13 => Kind::WrappingMultiply,
        14 => Kind::SaturatingMultiply,
        15 => Kind::BitwiseAnd,
        16 => Kind::BitwiseOr,
        17 => Kind::BitwiseXor,
        18 => Kind::WrappingShiftLeft,
        19 => Kind::WrappingShiftRight,
        20 => Kind::ExactShiftLeft,
        21 => Kind::ExactShiftRight,
        22 => Kind::TrappingShiftLeft,
        23 => Kind::TrappingShiftRight,
        24 => Kind::TrappingAdd,
        25 => Kind::TrappingSubtract,
        26 => Kind::TrappingMultiply,
        27 => Kind::TrappingDivide,
        28 => Kind::TrappingRemainder,
        29 => Kind::SaturatingShiftLeft,
        30 => Kind::SaturatingShiftRight,
    }))
}

#[cfg(test)]
mod tests {
    use super::{Error, FixedInteger, PackageReviewPrimitiveType, Reader, primitive_type};
    use crate::encoding::{
        PackagePolicyRecoveryLimits,
        encode::{
            encode_primitive_type, encode_scalar_expression,
            encoder::{Encoder, text::Writer},
        },
    };
    use crate::record::{
        PackageReviewArithmeticDomain, PackageReviewIntegerLiteral,
        PackageReviewIntegerLiteralLanding, PackageReviewScalarExpression,
    };

    fn encode(primitive: PackageReviewPrimitiveType) -> Vec<u8> {
        let mut encoder = Encoder::bounded(10);
        encode_primitive_type(&mut encoder, primitive).expect("encode exact carrier");
        encoder.finish().expect("bounded carrier bytes")
    }

    fn recover(bytes: &[u8]) -> Result<PackageReviewPrimitiveType, Error> {
        let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default())?;
        let primitive = primitive_type(&mut reader)?;
        reader.finish()?;
        Ok(primitive)
    }

    #[test]
    fn integer_width_evidence_round_trips_without_host_width_limits() {
        for width in [1, 8, 13, 64, 128, 257, u64::MAX] {
            for carrier in [FixedInteger::signed(width), FixedInteger::unsigned(width)] {
                let primitive = PackageReviewPrimitiveType::Integer(carrier);
                let bytes = encode(primitive);
                assert_eq!(bytes.len(), 10, "width metadata has constant size");
                assert_eq!(bytes[1], u8::from(carrier.is_signed()));
                assert_eq!(&bytes[2..], &width.to_le_bytes());
                assert_eq!(recover(&bytes), Ok(primitive));
            }
        }
        for primitive in [
            PackageReviewPrimitiveType::Bool,
            PackageReviewPrimitiveType::F32,
            PackageReviewPrimitiveType::F64,
            PackageReviewPrimitiveType::Addr,
        ] {
            assert_eq!(recover(&encode(primitive)), Ok(primitive));
        }
    }

    #[test]
    fn integer_width_evidence_rejects_malformed_carriers() {
        let bytes = encode(PackageReviewPrimitiveType::Integer(FixedInteger::unsigned(
            13,
        )));
        for length in 0..bytes.len() {
            assert_eq!(recover(&bytes[..length]), Err(Error::UnexpectedEnd));
        }
        let mut invalid = bytes.clone();
        invalid[1] = 2;
        assert_eq!(recover(&invalid), Err(Error::InvalidTag));
        invalid = bytes.clone();
        invalid[2..].fill(0);
        assert_eq!(recover(&invalid), Err(Error::InvalidValue));
        invalid = bytes;
        invalid.push(0);
        assert_eq!(recover(&invalid), Err(Error::TrailingBytes));
        for tag in [4, 5, 6, 7, 8, 9, 10, 12, 255] {
            assert_eq!(recover(&[tag]), Err(Error::InvalidTag));
        }
    }

    #[test]
    fn integer_width_evidence_survives_scalar_and_literal_text_and_binary() {
        use super::super::scalars::scalar_expression;
        use crate::encoding::recovery::policy::text::framing;

        for carrier in [FixedInteger::unsigned(13), FixedInteger::signed(257)] {
            let primitive_type = PackageReviewPrimitiveType::Integer(carrier);
            for expression in [
                PackageReviewScalarExpression::Parameter {
                    position: 0,
                    primitive_type,
                },
                PackageReviewScalarExpression::IntegerLiteral(PackageReviewIntegerLiteral {
                    canonical_text: "1".into(),
                    landing: Some(PackageReviewIntegerLiteralLanding {
                        landed_type: primitive_type,
                        arithmetic_domain: PackageReviewArithmeticDomain::Exact,
                    }),
                }),
            ] {
                let mut encoder = Encoder::bounded(256);
                encode_scalar_expression(&mut encoder, &expression).expect("encode scalar");
                let bytes = encoder.finish().expect("finish scalar");
                let mut reader =
                    Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
                assert_eq!(scalar_expression(&mut reader), Ok(expression.clone()));
                reader.finish().expect("complete scalar");

                let mut encoder = Encoder::policy_text(Writer::new(2048, None));
                encoder
                    .field_by("expression", &expression, encode_scalar_expression)
                    .unwrap();
                let text = encoder.finish_text().expect("named metadata fields");
                let (text_bytes, _) =
                    framing::binary(&text, PackagePolicyRecoveryLimits::default()).unwrap();
                assert_eq!(text_bytes, bytes);
                assert!(text.contains("tag integer 3"));
                assert!(text.contains("field signed"));
                assert!(text.contains("field width"));
            }
        }
    }

    #[test]
    fn integer_width_evidence_rejects_previous_policy_versions() {
        use crate::encoding::{
            CALLABLE_POLICY_MAGIC, PACKAGE_CALLABLE_POLICY_VERSION, PACKAGE_POLICY_BASELINE_MAGIC,
            PACKAGE_POLICY_BASELINE_VERSION,
        };
        use crate::record::{PackagePolicyBaseline, PackagePolicyCallables};

        let mut callable_header = CALLABLE_POLICY_MAGIC.to_vec();
        callable_header.extend_from_slice(&(PACKAGE_CALLABLE_POLICY_VERSION - 1).to_le_bytes());
        assert_eq!(
            PackagePolicyCallables::recover_canonical(
                &callable_header,
                PackagePolicyRecoveryLimits::default()
            ),
            Err(Error::UnsupportedVersion)
        );
        let mut baseline_header = PACKAGE_POLICY_BASELINE_MAGIC.to_vec();
        baseline_header.extend_from_slice(&(PACKAGE_POLICY_BASELINE_VERSION - 1).to_le_bytes());
        assert_eq!(
            PackagePolicyBaseline::recover_canonical(
                &baseline_header,
                PackagePolicyRecoveryLimits::default()
            ),
            Err(Error::UnsupportedVersion)
        );
    }
}
