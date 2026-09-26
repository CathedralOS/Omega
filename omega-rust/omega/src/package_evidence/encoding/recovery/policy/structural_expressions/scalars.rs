//! Exact scalar nodes share Boolean recursion and allocation accounting.
use crate::package_evidence::encoding::recovery::policy::structural_expressions::Reader;
use crate::package_evidence::encoding::recovery::policy::structural_expressions::boolean_expression;
use crate::package_evidence::encoding::recovery::policy::structural_expressions::primitive_type;
use crate::package_evidence::encoding::recovery::policy::structural_expressions::structural_path;
use crate::package_evidence::record::PackageReviewArithmeticDomain;
use crate::package_evidence::record::PackageReviewIntegerLiteral;
use crate::package_evidence::record::PackageReviewIntegerLiteralLanding;
use crate::package_evidence::record::PackageReviewIntegerRange;
use crate::package_evidence::record::PackageReviewScalarExpression;

use super::Error;
use super::tags::{integer_binary, primitive_type_name};

pub(super) fn scalar_expression(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewScalarExpression, Error> {
    reader.nested(|reader| {
        use PackageReviewScalarExpression as Expression;
        Ok(match reader.byte()? {
            0 => Expression::Parameter {
                position: reader.usize()?,
                primitive_type: primitive_type(reader)?,
            },
            1 => Expression::Local {
                position: reader.usize()?,
                primitive_type: primitive_type(reader)?,
            },
            2 => Expression::StructuralParameterField {
                parameter_position: reader.u32()?,
                path: structural_path(reader)?,
                primitive_type: primitive_type(reader)?,
            },
            3 => Expression::IntegerLiteral(PackageReviewIntegerLiteral {
                canonical_text: reader.string()?,
                landing: reader.option(|reader| {
                    Ok(PackageReviewIntegerLiteralLanding {
                        landed_type: primitive_type_name(reader)?,
                        arithmetic_domain: match reader.string()?.as_str() {
                            "Exact" => PackageReviewArithmeticDomain::Exact,
                            "Wrapping" => PackageReviewArithmeticDomain::Wrapping,
                            "Saturating" => PackageReviewArithmeticDomain::Saturating,
                            "Trapping" => PackageReviewArithmeticDomain::Trapping,
                            _ => return Err(Error::InvalidTag),
                        },
                    })
                })?,
            }),
            4 => Expression::IntegerBinary {
                kind: integer_binary(reader)?,
                primitive_type: primitive_type(reader)?,
                left: reader.boxed(scalar_expression)?,
                right: reader.boxed(scalar_expression)?,
            },
            5 => Expression::IntegerBitwiseNot {
                primitive_type: primitive_type(reader)?,
                operand: reader.boxed(scalar_expression)?,
            },
            6 => Expression::IntegerWiden {
                primitive_type: primitive_type(reader)?,
                operand: reader.boxed(scalar_expression)?,
            },
            7 => Expression::IntegerExactCast {
                primitive_type: primitive_type(reader)?,
                operand: reader.boxed(scalar_expression)?,
                range: PackageReviewIntegerRange {
                    minimum: reader.string()?,
                    maximum: reader.string()?,
                },
            },
            8 => Expression::Boolean(reader.boxed(boolean_expression)?),
            _ => return Err(Error::InvalidTag),
        })
    })
}
