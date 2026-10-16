//! Public runtime requirements contain entry parameters, never private locals.
use crate::record::PackageReviewBooleanExpression;
use crate::record::PackageReviewPrimitiveType;
use crate::record::PackageReviewRuntimeIndex;
use crate::record::PackageReviewScalarExpression;
use crate::record::PackageReviewStructuralPredicatePathSegment;

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
        } => field(*parameter_position, path, parameters, depth + 1),
        Expression::Not(operand) => validate(operand, parameters, depth + 1),
        Expression::Equal { left, right }
        | Expression::And { left, right }
        | Expression::Or { left, right } => {
            validate(left, parameters, depth + 1)?;
            validate(right, parameters, depth + 1)
        }
        Expression::IntegerComparison { left, right, .. }
        | Expression::ScalarIeeeFloatComparison { left, right, .. } => {
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
            field(left.parameter_position, &left.path, parameters, depth + 1)?;
            field(right.parameter_position, &right.path, parameters, depth + 1)
        }
        Expression::ByteSequenceEqual { left, right }
        | Expression::StructuralIdentity { left, right } => {
            field(left.parameter_position, &left.path, parameters, depth + 1)?;
            field(right.parameter_position, &right.path, parameters, depth + 1)
        }
        Expression::StructuralCaseMembership { subject, case } => {
            field(
                subject.parameter_position,
                &subject.path,
                parameters,
                depth + 1,
            )?;
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
        }
        | Expression::StructuralParameterByteLength {
            parameter_position,
            path,
        } => field(*parameter_position, path, parameters, depth + 1),
        Expression::StructuralParameterIndexedRead {
            parameter_position,
            path,
            index,
            inner_indexes,
            element_path,
            ..
        } => {
            field(*parameter_position, path, parameters, depth + 1)?;
            scalar(index, parameters, depth + 1)?;
            for inner_index in inner_indexes {
                scalar(inner_index, parameters, depth + 1)?;
            }
            structural_path(element_path, parameters, depth + 1)
        }
        Expression::IntegerLiteral(literal) => {
            if literal.canonical_text.is_empty() {
                return Err("structural requirement has an empty integer literal");
            }
            Ok(())
        }
        Expression::Float32Literal(_) | Expression::Float64Literal(_) => Ok(()),
        Expression::IntegerBinary { left, right, .. } => {
            scalar(left, parameters, depth + 1)?;
            scalar(right, parameters, depth + 1)
        }
        Expression::IntegerBitwiseNot { operand, .. }
        | Expression::IntegerWiden { operand, .. }
        | Expression::RepresentationView { operand, .. } => scalar(operand, parameters, depth + 1),
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
    depth: usize,
) -> Result<(), &'static str> {
    parameter(position as usize, count)?;
    structural_path(path, count, depth)
}

fn structural_path(
    path: &[PackageReviewStructuralPredicatePathSegment],
    count: usize,
    depth: usize,
) -> Result<(), &'static str> {
    bounded(depth)?;
    for segment in path {
        match segment {
            PackageReviewStructuralPredicatePathSegment::Field(name)
            | PackageReviewStructuralPredicatePathSegment::Case(name)
                if name.is_empty() =>
            {
                return Err("structural requirement has an empty path segment");
            }
            PackageReviewStructuralPredicatePathSegment::RuntimeIndex(index) => {
                bounded(depth + 1)?;
                match index {
                    PackageReviewRuntimeIndex::Parameter { position } => {
                        parameter(*position as usize, count)?
                    }
                    PackageReviewRuntimeIndex::StructuralField(subject) => {
                        field(subject.parameter_position, &subject.path, count, depth + 2)?;
                    }
                    PackageReviewRuntimeIndex::Local { .. }
                    | PackageReviewRuntimeIndex::AssignmentIndex { .. } => {
                        return Err(
                            "published callable requirement contains a private runtime index",
                        );
                    }
                }
            }
            _ => {}
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
