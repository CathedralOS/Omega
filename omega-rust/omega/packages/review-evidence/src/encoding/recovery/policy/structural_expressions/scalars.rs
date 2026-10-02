//! Exact scalar nodes share Boolean recursion and allocation accounting.
use crate::encoding::recovery::policy::structural_expressions::Reader;
use crate::encoding::recovery::policy::structural_expressions::boolean_expression;
use crate::encoding::recovery::policy::structural_expressions::primitive_type;
use crate::encoding::recovery::policy::structural_expressions::structural_path;
use crate::record::PackageReviewArithmeticDomain;
use crate::record::PackageReviewIntegerLiteral;
use crate::record::PackageReviewIntegerLiteralLanding;
use crate::record::PackageReviewIntegerRange;
use crate::record::PackageReviewScalarExpression;

use super::Error;
use super::tags::{integer_binary, primitive_type_name};

pub(in crate::encoding::recovery) fn scalar_expression(
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
            9 => Expression::Float32Literal(reader.u32()?),
            10 => Expression::Float64Literal(reader.u64()?),
            11 => Expression::IntegerWrappingCast {
                primitive_type: primitive_type(reader)?,
                operand: reader.boxed(scalar_expression)?,
            },
            12 => Expression::IntegerSaturatingCast {
                primitive_type: primitive_type(reader)?,
                operand: reader.boxed(scalar_expression)?,
            },
            13 => Expression::IntegerTrappingCast {
                primitive_type: primitive_type(reader)?,
                operand: reader.boxed(scalar_expression)?,
            },
            14 => Expression::RepresentationView {
                primitive_type: primitive_type(reader)?,
                operand: reader.boxed(scalar_expression)?,
            },
            15 => Expression::StructuralParameterByteLength {
                parameter_position: reader.u32()?,
                path: structural_path(reader)?,
            },
            16 => Expression::StructuralParameterIndexedRead {
                parameter_position: reader.u32()?,
                path: structural_path(reader)?,
                index: reader.boxed(scalar_expression)?,
                inner_indexes: reader.sequence(1, scalar_expression)?,
                element_path: structural_path(reader)?,
                primitive_type: primitive_type(reader)?,
            },
            _ => return Err(Error::InvalidTag),
        })
    })
}
