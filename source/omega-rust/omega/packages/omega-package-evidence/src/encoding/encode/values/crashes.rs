use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::evidence::{
    PackageReviewArithmeticDomain, PackageReviewBooleanExpression, PackageReviewCrash,
    PackageReviewCrashCall, PackageReviewCrashCause, PackageReviewCrashInterface,
    PackageReviewCrashPredicate, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewCrashSite, PackageReviewIeeeFloatComparisonKind, PackageReviewIntegerBinaryKind,
    PackageReviewIntegerComparisonKind, PackageReviewPermissionClaim,
    PackageReviewPermissionSource, PackageReviewPrimitiveType, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
};

use super::expressions::encode_contract_expression;
use super::identity::encode_nominal;

pub(crate) fn encode_crash(
    encoder: &mut Encoder,
    crash: &PackageReviewCrash,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match crash.interface {
        PackageReviewCrashInterface::InternalInferred => 0,
        PackageReviewCrashInterface::PublishedCeiling => 1,
    });
    encoder.sequence(&crash.published, encode_crash_route)?;
    encoder.option(
        crash.structural_runtime_requirements.as_deref(),
        |encoder, requirements| encoder.sequence(requirements, encode_boolean_expression),
    )?;
    encoder.sequence(&crash.checked_sites, encode_crash_site)?;
    encoder.sequence(&crash.checked_calls, encode_crash_call)
}

pub(crate) fn encode_crash_route(
    encoder: &mut Encoder,
    route: &PackageReviewCrashRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match route.cause {
        PackageReviewCrashCause::Trap => 0,
        PackageReviewCrashCause::Abort => 1,
    });
    encoder.sequence(&route.alternative_guards, |encoder, guard| {
        match guard {
            PackageReviewCrashRouteGuard::Truth => encoder.byte(0),
            PackageReviewCrashRouteGuard::Predicate(predicate) => {
                encoder.byte(1);
                encoder.bytes(&predicate.canonical_bytes)?;
            }
            PackageReviewCrashRouteGuard::Expression(expression) => {
                encoder.byte(2);
                encode_contract_expression(encoder, expression)?;
            }
        }
        Ok(())
    })
}

pub(crate) fn encode_crash_site(
    encoder: &mut Encoder,
    site: &PackageReviewCrashSite,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &site.state)?;
    encoder.u32(site.statement_ordinal);
    encoder.byte(match site.cause {
        PackageReviewCrashCause::Trap => 0,
        PackageReviewCrashCause::Abort => 1,
    });
    encoder.sequence(&site.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&site.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&site.guard_covering_buckets, |encoder, bucket| {
        encoder.u32(*bucket);
        Ok(())
    })?;
    encoder.sequence(&site.frontier_lower_bound, encode_permission_claim)
}

pub(crate) fn encode_crash_predicate(
    encoder: &mut Encoder,
    predicate: &PackageReviewCrashPredicate,
) -> Result<(), PackageReviewEncodingError> {
    encoder.bytes(&predicate.canonical_bytes)
}

pub(crate) fn encode_permission_claim(
    encoder: &mut Encoder,
    claim: &PackageReviewPermissionClaim,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &claim.machine)?;
    encode_nominal(encoder, &claim.state)?;
    match &claim.source {
        PackageReviewPermissionSource::StateEntry => encoder.byte(0),
        PackageReviewPermissionSource::Statement { statement_ordinal } => {
            encoder.byte(1);
            encoder.u64(*statement_ordinal);
        }
        PackageReviewPermissionSource::Call {
            statement_ordinal,
            call_ordinal,
            target,
        } => {
            encoder.byte(2);
            encoder.u64(*statement_ordinal);
            encoder.u64(*call_ordinal);
            encode_nominal(encoder, target)?;
        }
        PackageReviewPermissionSource::StateExit => encoder.byte(3),
    }
    encoder.u32(claim.ordinal);
    Ok(())
}

pub(crate) fn encode_crash_call(
    encoder: &mut Encoder,
    call: &PackageReviewCrashCall,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &call.state)?;
    encoder.u32(call.statement_ordinal);
    encoder.u32(call.call_ordinal);
    encode_nominal(encoder, &call.target_machine)?;
    encode_nominal(encoder, &call.target_state)?;
    encoder.sequence(&call.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&call.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&call.surviving_buckets, encode_crash_route)
}

pub(crate) fn encode_boolean_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewBooleanExpression::Constant(value) => {
            encoder.byte(0);
            encoder.boolean(*value);
        }
        PackageReviewBooleanExpression::Parameter { position } => {
            encoder.byte(1);
            encoder.usize(*position)?;
        }
        PackageReviewBooleanExpression::Local { position } => {
            encoder.byte(2);
            encoder.usize(*position)?;
        }
        PackageReviewBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            encoder.byte(3);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
        }
        PackageReviewBooleanExpression::Not(operand) => {
            encoder.byte(4);
            encode_boolean_expression(encoder, operand)?;
        }
        PackageReviewBooleanExpression::Equal { left, right } => {
            encoder.byte(5);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        PackageReviewBooleanExpression::IntegerComparison { kind, left, right } => {
            encoder.byte(6);
            encoder.byte(integer_comparison_tag(*kind));
            encode_scalar_expression(encoder, left)?;
            encode_scalar_expression(encoder, right)?;
        }
        PackageReviewBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.byte(7);
            encoder.byte(match kind {
                PackageReviewIeeeFloatComparisonKind::Equal => 0,
                PackageReviewIeeeFloatComparisonKind::NotEqual => 1,
            });
            encode_primitive_type(encoder, *primitive_type);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        PackageReviewBooleanExpression::ByteSequenceEqual { left, right } => {
            encoder.byte(8);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        PackageReviewBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            encoder.byte(9);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
            encoder.sequence(cases, |encoder, case| encoder.string(case))?;
        }
        PackageReviewBooleanExpression::StructuralCaseMembership { subject, case } => {
            encoder.byte(10);
            encode_structural_field(encoder, subject)?;
            encoder.string(case)?;
        }
        PackageReviewBooleanExpression::And { left, right } => {
            encoder.byte(11);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        PackageReviewBooleanExpression::Or { left, right } => {
            encoder.byte(12);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_scalar_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            encoder.byte(0);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        PackageReviewScalarExpression::Local {
            position,
            primitive_type,
        } => {
            encoder.byte(1);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        PackageReviewScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            encoder.byte(2);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        PackageReviewScalarExpression::IntegerLiteral(literal) => {
            encoder.byte(3);
            encoder.string(&literal.canonical_text)?;
            let landing = literal.landing.as_ref();
            encoder.option(landing, |encoder, landing| {
                encoder.string(primitive_type_name(landing.landed_type))?;
                encoder.string(arithmetic_domain_name(landing.arithmetic_domain))
            })?;
        }
        PackageReviewScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.byte(4);
            encoder.byte(integer_binary_tag(*kind));
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, left)?;
            encode_scalar_expression(encoder, right)?;
        }
        PackageReviewScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            encoder.byte(5);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        PackageReviewScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            encoder.byte(6);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        PackageReviewScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            encoder.byte(7);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
            encoder.string(&range.minimum)?;
            encoder.string(&range.maximum)?;
        }
        PackageReviewScalarExpression::Boolean(expression) => {
            encoder.byte(8);
            encode_boolean_expression(encoder, expression)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_structural_field(
    encoder: &mut Encoder,
    field: &PackageReviewStructuralParameterField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32(field.parameter_position);
    encode_structural_path(encoder, &field.path)
}

pub(crate) fn encode_structural_path(
    encoder: &mut Encoder,
    path: &[PackageReviewStructuralPredicatePathSegment],
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(path, |encoder, segment| {
        match segment {
            PackageReviewStructuralPredicatePathSegment::Field(field) => {
                encoder.byte(0);
                encoder.string(field)?;
            }
            PackageReviewStructuralPredicatePathSegment::Case(case) => {
                encoder.byte(1);
                encoder.string(case)?;
            }
        }
        Ok(())
    })
}

pub(crate) fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: PackageReviewPrimitiveType,
) {
    encoder.byte(match primitive_type {
        PackageReviewPrimitiveType::Bool => 0,
        PackageReviewPrimitiveType::F32 => 1,
        PackageReviewPrimitiveType::F64 => 2,
        PackageReviewPrimitiveType::I8 => 3,
        PackageReviewPrimitiveType::I16 => 4,
        PackageReviewPrimitiveType::I32 => 5,
        PackageReviewPrimitiveType::I64 => 6,
        PackageReviewPrimitiveType::U8 => 7,
        PackageReviewPrimitiveType::U16 => 8,
        PackageReviewPrimitiveType::U32 => 9,
        PackageReviewPrimitiveType::U64 => 10,
        PackageReviewPrimitiveType::Addr => 11,
    });
}

pub(crate) const fn integer_comparison_tag(kind: PackageReviewIntegerComparisonKind) -> u8 {
    match kind {
        PackageReviewIntegerComparisonKind::Equal => 0,
        PackageReviewIntegerComparisonKind::LessThan => 1,
        PackageReviewIntegerComparisonKind::LessOrEqual => 2,
    }
}

pub(crate) const fn integer_binary_tag(kind: PackageReviewIntegerBinaryKind) -> u8 {
    match kind {
        PackageReviewIntegerBinaryKind::ExactAdd => 0,
        PackageReviewIntegerBinaryKind::ExactSubtract => 1,
        PackageReviewIntegerBinaryKind::ExactMultiply => 2,
        PackageReviewIntegerBinaryKind::ExactDivide => 3,
        PackageReviewIntegerBinaryKind::ExactRemainder => 4,
        PackageReviewIntegerBinaryKind::WrappingDivide => 5,
        PackageReviewIntegerBinaryKind::WrappingRemainder => 6,
        PackageReviewIntegerBinaryKind::SaturatingDivide => 7,
        PackageReviewIntegerBinaryKind::SaturatingRemainder => 8,
        PackageReviewIntegerBinaryKind::WrappingAdd => 9,
        PackageReviewIntegerBinaryKind::SaturatingAdd => 10,
        PackageReviewIntegerBinaryKind::WrappingSubtract => 11,
        PackageReviewIntegerBinaryKind::SaturatingSubtract => 12,
        PackageReviewIntegerBinaryKind::WrappingMultiply => 13,
        PackageReviewIntegerBinaryKind::SaturatingMultiply => 14,
        PackageReviewIntegerBinaryKind::BitwiseAnd => 15,
        PackageReviewIntegerBinaryKind::BitwiseOr => 16,
        PackageReviewIntegerBinaryKind::BitwiseXor => 17,
        PackageReviewIntegerBinaryKind::WrappingShiftLeft => 18,
        PackageReviewIntegerBinaryKind::WrappingShiftRight => 19,
        PackageReviewIntegerBinaryKind::ExactShiftLeft => 20,
        PackageReviewIntegerBinaryKind::ExactShiftRight => 21,
    }
}

const fn primitive_type_name(primitive_type: PackageReviewPrimitiveType) -> &'static str {
    match primitive_type {
        PackageReviewPrimitiveType::Bool => "bool",
        PackageReviewPrimitiveType::F32 => "f32",
        PackageReviewPrimitiveType::F64 => "f64",
        PackageReviewPrimitiveType::I8 => "i8",
        PackageReviewPrimitiveType::I16 => "i16",
        PackageReviewPrimitiveType::I32 => "i32",
        PackageReviewPrimitiveType::I64 => "i64",
        PackageReviewPrimitiveType::U8 => "u8",
        PackageReviewPrimitiveType::U16 => "u16",
        PackageReviewPrimitiveType::U32 => "u32",
        PackageReviewPrimitiveType::U64 => "u64",
        PackageReviewPrimitiveType::Addr => "addr",
    }
}

const fn arithmetic_domain_name(domain: PackageReviewArithmeticDomain) -> &'static str {
    match domain {
        PackageReviewArithmeticDomain::Exact => "Exact",
        PackageReviewArithmeticDomain::Wrapping => "Wrapping",
        PackageReviewArithmeticDomain::Saturating => "Saturating",
        PackageReviewArithmeticDomain::Trapping => "Trapping",
    }
}

#[cfg(test)]
mod tests {
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
        let expression = PackageReviewScalarExpression::IntegerLiteral(
            crate::evidence::PackageReviewIntegerLiteral {
                canonical_text: "7".to_owned(),
                landing: Some(crate::evidence::PackageReviewIntegerLiteralLanding {
                    landed_type: PackageReviewPrimitiveType::U32,
                    arithmetic_domain: PackageReviewArithmeticDomain::Exact,
                }),
            },
        );
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
}
