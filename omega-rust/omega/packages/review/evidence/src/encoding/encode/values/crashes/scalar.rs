//! Structural scalar expressions preserve exact numeric vocabulary.

use super::*;

pub(crate) fn encode_scalar_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| encode_scalar_node(encoder, expression))
}

fn encode_scalar_node(
    encoder: &mut Encoder,
    expression: &PackageReviewScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            encoder.tag("parameter", 0);
            encoder.field("position", |encoder| encoder.usize(*position))?;
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
        }
        PackageReviewScalarExpression::Local {
            position,
            primitive_type,
        } => {
            encoder.tag("local", 1);
            encoder.field("position", |encoder| encoder.usize(*position))?;
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
        }
        PackageReviewScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            encoder.tag("structural_parameter_field", 2);
            encoder.field("parameter_position", |encoder| {
                encoder.u32(*parameter_position);
                Ok(())
            })?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
        }
        PackageReviewScalarExpression::IntegerLiteral(literal) => {
            encoder.tag("integer_literal", 3);
            encoder.field("canonical_text", |encoder| {
                encoder.string(&literal.canonical_text)
            })?;
            encoder.field("landing", |encoder| {
                encoder.option(literal.landing.as_ref(), |encoder, landing| {
                    encoder.field("landed_type", |encoder| {
                        encoder.string(primitive_type_name(landing.landed_type))
                    })?;
                    encoder.field("arithmetic_domain", |encoder| {
                        encoder.string(arithmetic_domain_name(landing.arithmetic_domain))
                    })
                })
            })?;
        }
        PackageReviewScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.tag("integer_binary", 4);
            encoder.field("kind", |encoder| {
                encoder.tag(integer_binary_name(*kind), integer_binary_tag(*kind));
                Ok(())
            })?;
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
            encoder.field("left", |encoder| encode_scalar_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_scalar_expression(encoder, right))?;
        }
        PackageReviewScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            encoder.tag("integer_bitwise_not", 5);
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
            encoder.field("operand", |encoder| {
                encode_scalar_expression(encoder, operand)
            })?;
        }
        PackageReviewScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            encoder.tag("integer_widen", 6);
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
            encoder.field("operand", |encoder| {
                encode_scalar_expression(encoder, operand)
            })?;
        }
        PackageReviewScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            encoder.tag("integer_exact_cast", 7);
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
            encoder.field("operand", |encoder| {
                encode_scalar_expression(encoder, operand)
            })?;
            encoder.field("minimum", |encoder| encoder.string(&range.minimum))?;
            encoder.field("maximum", |encoder| encoder.string(&range.maximum))?;
        }
        PackageReviewScalarExpression::Boolean(expression) => {
            encoder.tag("boolean", 8);
            encoder.field("expression", |encoder| {
                encode_boolean_expression(encoder, expression)
            })?;
        }
    }
    Ok(())
}

const fn integer_binary_name(kind: PackageReviewIntegerBinaryKind) -> &'static str {
    match kind {
        PackageReviewIntegerBinaryKind::ExactAdd => "exact_add",
        PackageReviewIntegerBinaryKind::ExactSubtract => "exact_subtract",
        PackageReviewIntegerBinaryKind::ExactMultiply => "exact_multiply",
        PackageReviewIntegerBinaryKind::ExactDivide => "exact_divide",
        PackageReviewIntegerBinaryKind::ExactRemainder => "exact_remainder",
        PackageReviewIntegerBinaryKind::WrappingDivide => "wrapping_divide",
        PackageReviewIntegerBinaryKind::WrappingRemainder => "wrapping_remainder",
        PackageReviewIntegerBinaryKind::SaturatingDivide => "saturating_divide",
        PackageReviewIntegerBinaryKind::SaturatingRemainder => "saturating_remainder",
        PackageReviewIntegerBinaryKind::WrappingAdd => "wrapping_add",
        PackageReviewIntegerBinaryKind::SaturatingAdd => "saturating_add",
        PackageReviewIntegerBinaryKind::WrappingSubtract => "wrapping_subtract",
        PackageReviewIntegerBinaryKind::SaturatingSubtract => "saturating_subtract",
        PackageReviewIntegerBinaryKind::WrappingMultiply => "wrapping_multiply",
        PackageReviewIntegerBinaryKind::SaturatingMultiply => "saturating_multiply",
        PackageReviewIntegerBinaryKind::BitwiseAnd => "bitwise_and",
        PackageReviewIntegerBinaryKind::BitwiseOr => "bitwise_or",
        PackageReviewIntegerBinaryKind::BitwiseXor => "bitwise_xor",
        PackageReviewIntegerBinaryKind::WrappingShiftLeft => "wrapping_shift_left",
        PackageReviewIntegerBinaryKind::WrappingShiftRight => "wrapping_shift_right",
        PackageReviewIntegerBinaryKind::ExactShiftLeft => "exact_shift_left",
        PackageReviewIntegerBinaryKind::ExactShiftRight => "exact_shift_right",
    }
}
