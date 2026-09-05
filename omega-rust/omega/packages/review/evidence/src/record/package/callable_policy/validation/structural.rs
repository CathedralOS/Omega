//! Public runtime requirements contain entry parameters, never private locals.

use super::*;

pub(super) fn validate(
    value: &PackageReviewBooleanExpression,
    parameters: usize,
    depth: usize,
) -> Result<(), &'static str> {
    bounded(depth)?;
    use PackageReviewBooleanExpression as Expression;
    match value {
        Expression::Constant(_) => Ok(()),
        Expression::Parameter { position } => parameter(*position, parameters),
        Expression::Local { .. } => Err("published callable requirement contains a private local"),
        Expression::StructuralParameterField {
            parameter_position,
            path,
        } => field(*parameter_position, path, parameters),
        Expression::Not(operand) => validate(operand, parameters, depth + 1),
        Expression::Equal { left, right }
        | Expression::And { left, right }
        | Expression::Or { left, right } => {
            validate(left, parameters, depth + 1)?;
            validate(right, parameters, depth + 1)
        }
        Expression::IntegerComparison { left, right, .. } => {
            scalar(left, parameters, depth + 1)?;
            scalar(right, parameters, depth + 1)
        }
        Expression::IeeeFloatComparison {
            primitive_type,
            left,
            right,
            ..
        } => {
            if !matches!(
                primitive_type,
                PackageReviewPrimitiveType::F32 | PackageReviewPrimitiveType::F64
            ) {
                return Err("IEEE callable comparison has a non-float type");
            }
            field(left.parameter_position, &left.path, parameters)?;
            field(right.parameter_position, &right.path, parameters)
        }
        Expression::ByteSequenceEqual { left, right } => {
            field(left.parameter_position, &left.path, parameters)?;
            field(right.parameter_position, &right.path, parameters)
        }
        Expression::PayloadlessSumEqual { left, right, cases } => {
            field(left.parameter_position, &left.path, parameters)?;
            field(right.parameter_position, &right.path, parameters)?;
            if cases.is_empty()
                || cases.iter().any(String::is_empty)
                || cases
                    .iter()
                    .enumerate()
                    .any(|(position, case)| cases[..position].contains(case))
            {
                return Err("sum equality has empty or duplicate cases");
            }
            Ok(())
        }
        Expression::StructuralCaseMembership { subject, case } => {
            field(subject.parameter_position, &subject.path, parameters)?;
            if case.is_empty() {
                return Err("structural requirement has an empty case");
            }
            Ok(())
        }
    }
}

fn scalar(
    value: &PackageReviewScalarExpression,
    parameters: usize,
    depth: usize,
) -> Result<(), &'static str> {
    bounded(depth)?;
    use PackageReviewScalarExpression as Expression;
    match value {
        Expression::Parameter { position, .. } => parameter(*position, parameters),
        Expression::Local { .. } => {
            Err("published callable requirement contains a private scalar local")
        }
        Expression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } => field(*parameter_position, path, parameters),
        Expression::IntegerLiteral(literal) => {
            if literal.canonical_text.is_empty() {
                return Err("structural requirement has an empty integer literal");
            }
            Ok(())
        }
        Expression::IntegerBinary { left, right, .. } => {
            scalar(left, parameters, depth + 1)?;
            scalar(right, parameters, depth + 1)
        }
        Expression::IntegerBitwiseNot { operand, .. }
        | Expression::IntegerWiden { operand, .. } => scalar(operand, parameters, depth + 1),
        Expression::IntegerExactCast { operand, range, .. } => {
            if range.minimum.is_empty() || range.maximum.is_empty() {
                return Err("structural requirement has an empty cast range");
            }
            scalar(operand, parameters, depth + 1)
        }
        Expression::Boolean(value) => validate(value, parameters, depth + 1),
    }
}

fn parameter(position: usize, count: usize) -> Result<(), &'static str> {
    if position >= count {
        return Err("structural requirement parameter is outside its entry telescope");
    }
    Ok(())
}

fn field(
    position: u32,
    path: &[PackageReviewStructuralPredicatePathSegment],
    count: usize,
) -> Result<(), &'static str> {
    parameter(position as usize, count)?;
    for segment in path {
        let (PackageReviewStructuralPredicatePathSegment::Field(name)
        | PackageReviewStructuralPredicatePathSegment::Case(name)) = segment;
        if name.is_empty() {
            return Err("structural requirement has an empty path segment");
        }
    }
    Ok(())
}

fn bounded(depth: usize) -> Result<(), &'static str> {
    if depth >= 128 {
        return Err("structural requirement exceeds the policy nesting ceiling");
    }
    Ok(())
}
