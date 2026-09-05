use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
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

mod boolean;
mod scalar;
pub(crate) use boolean::encode_boolean_expression;
pub(crate) use scalar::encode_scalar_expression;

mod replay;
#[cfg(test)]
mod tests;
mod vocabulary;

pub(crate) use replay::{encode_crash, encode_crash_route};
use vocabulary::{arithmetic_domain_name, primitive_type_name};
pub(crate) use vocabulary::{encode_primitive_type, integer_binary_tag, integer_comparison_tag};

pub(crate) fn encode_structural_field(
    encoder: &mut Encoder,
    field: &PackageReviewStructuralParameterField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("parameter_position", |encoder| {
        encoder.u32(field.parameter_position);
        Ok(())
    })?;
    encoder.field("path", |encoder| {
        encode_structural_path(encoder, &field.path)
    })
}

pub(crate) fn encode_structural_path(
    encoder: &mut Encoder,
    path: &[PackageReviewStructuralPredicatePathSegment],
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(path, |encoder, segment| {
        match segment {
            PackageReviewStructuralPredicatePathSegment::Field(field) => {
                encoder.tag("field", 0);
                encoder.field("field", |encoder| encoder.string(field))?;
            }
            PackageReviewStructuralPredicatePathSegment::Case(case) => {
                encoder.tag("case", 1);
                encoder.field("case", |encoder| encoder.string(case))?;
            }
        }
        Ok(())
    })
}
