//! Structural scalar expressions preserve exact numeric vocabulary.
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::values::crashes::Encoder;
use crate::encoding::encode::values::crashes::arithmetic_domain_name;
use crate::encoding::encode::values::crashes::encode_boolean_expression;
use crate::encoding::encode::values::crashes::encode_primitive_type;
use crate::encoding::encode::values::crashes::encode_structural_path;
use crate::encoding::encode::values::crashes::integer_binary_row;
use crate::encoding::encode::values::crashes::primitive_type_name;
use crate::record::PackageReviewScalarExpression;

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
        PackageReviewScalarExpression::Float32Literal(bits) => {
            encoder.tag("float32_literal", 9);
            encoder.field("bits", |encoder| {
                encoder.u32(*bits);
                Ok(())
            })?;
        }
        PackageReviewScalarExpression::Float64Literal(bits) => {
            encoder.tag("float64_literal", 10);
            encoder.field("bits", |encoder| {
                encoder.u64(*bits);
                Ok(())
            })?;
        }
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
                let (tag, name) = integer_binary_row(*kind);
                encoder.tag(name, tag);
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
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
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
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
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
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
            encoder.field("minimum", |encoder| encoder.string(&range.minimum))?;
            encoder.field("maximum", |encoder| encoder.string(&range.maximum))?;
        }
        PackageReviewScalarExpression::RepresentationView {
            primitive_type,
            operand,
        } => {
            encoder.tag("representation_view", 14);
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
        }
        PackageReviewScalarExpression::StructuralParameterByteLength {
            parameter_position,
            path,
        } => {
            encoder.tag("structural_parameter_byte_length", 15);
            encoder.field("parameter_position", |encoder| {
                encoder.u32(*parameter_position);
                Ok(())
            })?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
        }
        PackageReviewScalarExpression::StructuralParameterIndexedRead {
            parameter_position,
            path,
            index,
            inner_indexes,
            element_path,
            primitive_type,
        } => {
            encoder.tag("structural_parameter_indexed_read", 16);
            encoder.field("parameter_position", |encoder| {
                encoder.u32(*parameter_position);
                Ok(())
            })?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
            encoder.field("index", |encoder| encode_scalar_expression(encoder, index))?;
            encoder.field("inner_indexes", |encoder| {
                encoder.sequence(inner_indexes, encode_scalar_expression)
            })?;
            encoder.field_by(
                "element_path",
                element_path.as_slice(),
                encode_structural_path,
            )?;
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
        }
        PackageReviewScalarExpression::Boolean(expression) => {
            encoder.tag("boolean", 8);
            encoder.field_by("expression", &**expression, encode_boolean_expression)?;
        }
    }
    Ok(())
}
