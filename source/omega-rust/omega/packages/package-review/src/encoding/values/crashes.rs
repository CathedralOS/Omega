use crate::encoding::PackageReviewEncodingError;
use crate::encoding::canonical::encoder::Encoder;
use crate::evidence::{
    PackageReviewCrash, PackageReviewCrashCall, PackageReviewCrashInterface,
    PackageReviewCrashPredicate, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewCrashSite, PackageReviewPermissionClaim, PackageReviewPermissionSource,
};
use psi_checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment,
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
        psi_checked_trees::CrashCause::Trap => 0,
        psi_checked_trees::CrashCause::Abort => 1,
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
        psi_checked_trees::CrashCause::Trap => 0,
        psi_checked_trees::CrashCause::Abort => 1,
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
    expression: &CheckedBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        CheckedBooleanExpression::Constant(value) => {
            encoder.byte(0);
            encoder.boolean(*value);
        }
        CheckedBooleanExpression::Parameter { position } => {
            encoder.byte(1);
            encoder.usize(*position)?;
        }
        CheckedBooleanExpression::Local { position } => {
            encoder.byte(2);
            encoder.usize(*position)?;
        }
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            encoder.byte(3);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
        }
        CheckedBooleanExpression::Not(operand) => {
            encoder.byte(4);
            encode_boolean_expression(encoder, operand)?;
        }
        CheckedBooleanExpression::Equal { left, right } => {
            encoder.byte(5);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            encoder.byte(6);
            encoder.byte(integer_comparison_tag(*kind));
            encode_scalar_expression(encoder, left)?;
            encode_scalar_expression(encoder, right)?;
        }
        CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.byte(7);
            encoder.byte(match kind {
                psi_checked_trees::CheckedIeeeFloatComparisonKind::Equal => 0,
                psi_checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => 1,
            });
            encode_primitive_type(encoder, *primitive_type);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            encoder.byte(8);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
        }
        CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            encoder.byte(9);
            encode_structural_field(encoder, left)?;
            encode_structural_field(encoder, right)?;
            encoder.sequence(cases, |encoder, case| encoder.string(case))?;
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            encoder.byte(10);
            encode_structural_field(encoder, subject)?;
            encoder.string(case)?;
        }
        CheckedBooleanExpression::And { left, right } => {
            encoder.byte(11);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
        CheckedBooleanExpression::Or { left, right } => {
            encoder.byte(12);
            encode_boolean_expression(encoder, left)?;
            encode_boolean_expression(encoder, right)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_scalar_expression(
    encoder: &mut Encoder,
    expression: &CheckedScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            encoder.byte(0);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => {
            encoder.byte(1);
            encoder.usize(*position)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            encoder.byte(2);
            encoder.u32(*parameter_position);
            encode_structural_path(encoder, path)?;
            encode_primitive_type(encoder, *primitive_type);
        }
        CheckedScalarExpression::IntegerLiteral { literal } => {
            encoder.byte(3);
            encoder.string(literal.text())?;
            let landing = literal.landing();
            encoder.option(landing.as_ref(), |encoder, landing| {
                encoder.string(landing.landed_type.name())?;
                encoder.string(landing.domain.name())
            })?;
        }
        CheckedScalarExpression::IntegerBinary {
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
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            encoder.byte(5);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            encoder.byte(6);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            encoder.byte(7);
            encode_primitive_type(encoder, *primitive_type);
            encode_scalar_expression(encoder, operand)?;
            encoder.string(&range.minimum.to_string())?;
            encoder.string(&range.maximum.to_string())?;
        }
        CheckedScalarExpression::Boolean(expression) => {
            encoder.byte(8);
            encode_boolean_expression(encoder, expression)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_structural_field(
    encoder: &mut Encoder,
    field: &CheckedStructuralParameterField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32(field.parameter_position);
    encode_structural_path(encoder, &field.path)
}

pub(crate) fn encode_structural_path(
    encoder: &mut Encoder,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(path, |encoder, segment| {
        match segment {
            CheckedStructuralPredicatePathSegment::Field(field) => {
                encoder.byte(0);
                encoder.string(field)?;
            }
            CheckedStructuralPredicatePathSegment::Case(case) => {
                encoder.byte(1);
                encoder.string(case)?;
            }
        }
        Ok(())
    })
}

pub(crate) fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: psi_typed_trees::types::PrimitiveType,
) {
    encoder.byte(match primitive_type {
        psi_typed_trees::types::PrimitiveType::Bool => 0,
        psi_typed_trees::types::PrimitiveType::F32 => 1,
        psi_typed_trees::types::PrimitiveType::F64 => 2,
        psi_typed_trees::types::PrimitiveType::I8 => 3,
        psi_typed_trees::types::PrimitiveType::I16 => 4,
        psi_typed_trees::types::PrimitiveType::I32 => 5,
        psi_typed_trees::types::PrimitiveType::I64 => 6,
        psi_typed_trees::types::PrimitiveType::U8 => 7,
        psi_typed_trees::types::PrimitiveType::U16 => 8,
        psi_typed_trees::types::PrimitiveType::U32 => 9,
        psi_typed_trees::types::PrimitiveType::U64 => 10,
        psi_typed_trees::types::PrimitiveType::Addr => 11,
    });
}

pub(crate) const fn integer_comparison_tag(kind: CheckedIntegerComparisonKind) -> u8 {
    match kind {
        CheckedIntegerComparisonKind::Equal => 0,
        CheckedIntegerComparisonKind::LessThan => 1,
        CheckedIntegerComparisonKind::LessOrEqual => 2,
    }
}

pub(crate) const fn integer_binary_tag(kind: CheckedIntegerBinaryKind) -> u8 {
    match kind {
        CheckedIntegerBinaryKind::ExactAdd => 0,
        CheckedIntegerBinaryKind::ExactSubtract => 1,
        CheckedIntegerBinaryKind::ExactMultiply => 2,
        CheckedIntegerBinaryKind::ExactDivide => 3,
        CheckedIntegerBinaryKind::ExactRemainder => 4,
        CheckedIntegerBinaryKind::WrappingDivide => 5,
        CheckedIntegerBinaryKind::WrappingRemainder => 6,
        CheckedIntegerBinaryKind::SaturatingDivide => 7,
        CheckedIntegerBinaryKind::SaturatingRemainder => 8,
        CheckedIntegerBinaryKind::WrappingAdd => 9,
        CheckedIntegerBinaryKind::SaturatingAdd => 10,
        CheckedIntegerBinaryKind::WrappingSubtract => 11,
        CheckedIntegerBinaryKind::SaturatingSubtract => 12,
        CheckedIntegerBinaryKind::WrappingMultiply => 13,
        CheckedIntegerBinaryKind::SaturatingMultiply => 14,
        CheckedIntegerBinaryKind::BitwiseAnd => 15,
        CheckedIntegerBinaryKind::BitwiseOr => 16,
        CheckedIntegerBinaryKind::BitwiseXor => 17,
        CheckedIntegerBinaryKind::WrappingShiftLeft => 18,
        CheckedIntegerBinaryKind::WrappingShiftRight => 19,
        CheckedIntegerBinaryKind::ExactShiftLeft => 20,
        CheckedIntegerBinaryKind::ExactShiftRight => 21,
    }
}
