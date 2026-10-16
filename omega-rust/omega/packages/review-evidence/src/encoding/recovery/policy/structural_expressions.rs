//! Typed structural runtime requirements under the enclosing policy budget.

use crate::record::{
    PackageReviewBooleanExpression, PackageReviewIeeeFloatComparisonKind,
    PackageReviewIntegerComparisonKind, PackageReviewRuntimeIndex,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
};
pub(in crate::encoding::recovery) mod scalars;
pub(in crate::encoding::recovery) mod tags;

use super::{Error, reader::Reader};
use scalars::scalar_expression;
use tags::primitive_type;

pub(in crate::encoding::recovery) fn boolean_expression(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBooleanExpression, Error> {
    reader.nested(|reader| {
        use PackageReviewBooleanExpression as Expression;
        Ok(super::tag!(reader => {
            0 => Expression::Constant(reader.boolean()?),
            1 => Expression::Parameter {
                position: reader.usize()?,
            },
            2 => Expression::Local {
                position: reader.usize()?,
            },
            3 => Expression::StructuralParameterField {
                parameter_position: reader.u32()?,
                path: structural_path(reader)?,
            },
            4 => Expression::Not(reader.boxed(boolean_expression)?),
            5 => Expression::Equal {
                left: reader.boxed(boolean_expression)?,
                right: reader.boxed(boolean_expression)?,
            },
            6 => Expression::IntegerComparison {
                kind: super::tag!(reader => {
                    0 => PackageReviewIntegerComparisonKind::Equal,
                    1 => PackageReviewIntegerComparisonKind::LessThan,
                    2 => PackageReviewIntegerComparisonKind::LessOrEqual,
                }),
                left: reader.boxed(scalar_expression)?,
                right: reader.boxed(scalar_expression)?,
            },
            7 => Expression::IeeeFloatComparison {
                kind: ieee_float_comparison_kind(reader)?,
                primitive_type: primitive_type(reader)?,
                left: structural_field(reader)?,
                right: structural_field(reader)?,
            },
            8 => Expression::ByteSequenceEqual {
                left: structural_field(reader)?,
                right: structural_field(reader)?,
            },
            10 => Expression::StructuralCaseMembership {
                subject: structural_field(reader)?,
                case: reader.string()?,
            },
            11 => Expression::And {
                left: reader.boxed(boolean_expression)?,
                right: reader.boxed(boolean_expression)?,
            },
            12 => Expression::Or {
                left: reader.boxed(boolean_expression)?,
                right: reader.boxed(boolean_expression)?,
            },
            13 => Expression::ScalarIeeeFloatComparison {
                kind: ieee_float_comparison_kind(reader)?,
                left: reader.boxed(scalar_expression)?,
                right: reader.boxed(scalar_expression)?,
            },
            14 => Expression::StructuralIdentity {
                left: structural_field(reader)?,
                right: structural_field(reader)?,
            },
        }))
    })
}

fn ieee_float_comparison_kind(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewIeeeFloatComparisonKind, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewIeeeFloatComparisonKind::Equal,
        1 => PackageReviewIeeeFloatComparisonKind::NotEqual,
        2 => PackageReviewIeeeFloatComparisonKind::LessThan,
        3 => PackageReviewIeeeFloatComparisonKind::LessOrEqual,
        4 => PackageReviewIeeeFloatComparisonKind::NotLessThan,
        5 => PackageReviewIeeeFloatComparisonKind::NotLessOrEqual,
    }))
}

pub(in crate::encoding::recovery) fn structural_field(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewStructuralParameterField, Error> {
    Ok(PackageReviewStructuralParameterField {
        parameter_position: reader.u32()?,
        path: structural_path(reader)?,
    })
}

pub(in crate::encoding::recovery) fn structural_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<PackageReviewStructuralPredicatePathSegment>, Error> {
    reader.sequence(9, |reader| {
        Ok(super::tag!(reader => {
            0 => PackageReviewStructuralPredicatePathSegment::Field(reader.string()?),
            1 => PackageReviewStructuralPredicatePathSegment::Case(reader.string()?),
            2 => PackageReviewStructuralPredicatePathSegment::FixedIndex(reader.u64()?),
            3 => PackageReviewStructuralPredicatePathSegment::RuntimeIndex(runtime_index(reader)?),
        }))
    })
}

fn runtime_index(reader: &mut Reader<'_>) -> Result<PackageReviewRuntimeIndex, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewRuntimeIndex::Parameter {
            position: reader.u32()?,
        },
        1 => PackageReviewRuntimeIndex::AssignmentIndex {
            depth: reader.u32()?,
        },
        2 => PackageReviewRuntimeIndex::StructuralField(PackageReviewStructuralParameterField {
            parameter_position: reader.u32()?,
            path: structural_path(reader)?,
        }),
        3 => PackageReviewRuntimeIndex::Local {
            position: reader.u32()?,
        },
    }))
}
