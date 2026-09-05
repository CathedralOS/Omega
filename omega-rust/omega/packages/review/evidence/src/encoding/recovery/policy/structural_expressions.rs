//! Typed structural runtime requirements under the enclosing policy budget.

mod scalars;
mod tags;
#[cfg(test)]
mod tests;

use super::{Error, reader::Reader};
use crate::record::*;
use scalars::scalar_expression;
use tags::primitive_type;

pub(super) fn boolean_expression(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBooleanExpression, Error> {
    reader.nested(|reader| {
        use PackageReviewBooleanExpression as Expression;
        Ok(match reader.byte()? {
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
                kind: match reader.byte()? {
                    0 => PackageReviewIntegerComparisonKind::Equal,
                    1 => PackageReviewIntegerComparisonKind::LessThan,
                    2 => PackageReviewIntegerComparisonKind::LessOrEqual,
                    _ => return Err(Error::InvalidTag),
                },
                left: reader.boxed(scalar_expression)?,
                right: reader.boxed(scalar_expression)?,
            },
            7 => Expression::IeeeFloatComparison {
                kind: match reader.byte()? {
                    0 => PackageReviewIeeeFloatComparisonKind::Equal,
                    1 => PackageReviewIeeeFloatComparisonKind::NotEqual,
                    _ => return Err(Error::InvalidTag),
                },
                primitive_type: primitive_type(reader)?,
                left: structural_field(reader)?,
                right: structural_field(reader)?,
            },
            8 => Expression::ByteSequenceEqual {
                left: structural_field(reader)?,
                right: structural_field(reader)?,
            },
            9 => Expression::PayloadlessSumEqual {
                left: structural_field(reader)?,
                right: structural_field(reader)?,
                cases: reader.sequence(8, |reader| reader.string())?,
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
            _ => return Err(Error::InvalidTag),
        })
    })
}

fn structural_field(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewStructuralParameterField, Error> {
    Ok(PackageReviewStructuralParameterField {
        parameter_position: reader.u32()?,
        path: structural_path(reader)?,
    })
}

fn structural_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<PackageReviewStructuralPredicatePathSegment>, Error> {
    reader.sequence(9, |reader| {
        Ok(match reader.byte()? {
            0 => PackageReviewStructuralPredicatePathSegment::Field(reader.string()?),
            1 => PackageReviewStructuralPredicatePathSegment::Case(reader.string()?),
            _ => return Err(Error::InvalidTag),
        })
    })
}
