//! Structural Boolean expressions share binary tags and named text fields.

use super::*;

pub(crate) fn encode_boolean_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| encode_boolean_node(encoder, expression))
}

fn encode_boolean_node(
    encoder: &mut Encoder,
    expression: &PackageReviewBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewBooleanExpression::Constant(value) => {
            encoder.tag("constant", 0);
            encoder.field("value", |encoder| {
                encoder.boolean(*value);
                Ok(())
            })?;
        }
        PackageReviewBooleanExpression::Parameter { position } => {
            encoder.tag("parameter", 1);
            encoder.field("position", |encoder| encoder.usize(*position))?;
        }
        PackageReviewBooleanExpression::Local { position } => {
            encoder.tag("local", 2);
            encoder.field("position", |encoder| encoder.usize(*position))?;
        }
        PackageReviewBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            encoder.tag("structural_parameter_field", 3);
            encoder.field("parameter_position", |encoder| {
                encoder.u32(*parameter_position);
                Ok(())
            })?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
        }
        PackageReviewBooleanExpression::Not(operand) => {
            encoder.tag("not", 4);
            encoder.field("operand", |encoder| {
                encode_boolean_expression(encoder, operand)
            })?;
        }
        PackageReviewBooleanExpression::Equal { left, right } => {
            encoder.tag("equal", 5);
            encoder.field("left", |encoder| encode_boolean_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_boolean_expression(encoder, right))?;
        }
        PackageReviewBooleanExpression::IntegerComparison { kind, left, right } => {
            encoder.tag("integer_comparison", 6);
            encoder.field("kind", |encoder| {
                let name = match kind {
                    PackageReviewIntegerComparisonKind::Equal => "equal",
                    PackageReviewIntegerComparisonKind::LessThan => "less_than",
                    PackageReviewIntegerComparisonKind::LessOrEqual => "less_or_equal",
                };
                encoder.tag(name, integer_comparison_tag(*kind));
                Ok(())
            })?;
            encoder.field("left", |encoder| encode_scalar_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_scalar_expression(encoder, right))?;
        }
        PackageReviewBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.tag("ieee_float_comparison", 7);
            encoder.field("kind", |encoder| {
                match kind {
                    PackageReviewIeeeFloatComparisonKind::Equal => encoder.tag("equal", 0),
                    PackageReviewIeeeFloatComparisonKind::NotEqual => encoder.tag("not_equal", 1),
                }
                Ok(())
            })?;
            encoder.field("primitive_type", |encoder| {
                encode_primitive_type(encoder, *primitive_type);
                Ok(())
            })?;
            encoder.field("left", |encoder| encode_structural_field(encoder, left))?;
            encoder.field("right", |encoder| encode_structural_field(encoder, right))?;
        }
        PackageReviewBooleanExpression::ByteSequenceEqual { left, right } => {
            encoder.tag("byte_sequence_equal", 8);
            encoder.field("left", |encoder| encode_structural_field(encoder, left))?;
            encoder.field("right", |encoder| encode_structural_field(encoder, right))?;
        }
        PackageReviewBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            encoder.tag("payloadless_sum_equal", 9);
            encoder.field("left", |encoder| encode_structural_field(encoder, left))?;
            encoder.field("right", |encoder| encode_structural_field(encoder, right))?;
            encoder.field("cases", |encoder| {
                encoder.sequence(cases, |encoder, case| encoder.string(case))
            })?;
        }
        PackageReviewBooleanExpression::StructuralCaseMembership { subject, case } => {
            encoder.tag("structural_case_membership", 10);
            encoder.field("subject", |encoder| {
                encode_structural_field(encoder, subject)
            })?;
            encoder.field("case", |encoder| encoder.string(case))?;
        }
        PackageReviewBooleanExpression::And { left, right } => {
            encoder.tag("and", 11);
            encoder.field("left", |encoder| encode_boolean_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_boolean_expression(encoder, right))?;
        }
        PackageReviewBooleanExpression::Or { left, right } => {
            encoder.tag("or", 12);
            encoder.field("left", |encoder| encode_boolean_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_boolean_expression(encoder, right))?;
        }
    }
    Ok(())
}
