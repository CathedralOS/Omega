// Validation that public runtime requirements reference entry parameters and never private locals.
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
        Expression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            scalar(left, parameters, depth + 1)?;
            scalar(right, parameters, depth + 1)?;
            if !integer_binary_types(
                *kind,
                *primitive_type,
                scalar_carrier(left),
                scalar_carrier(right),
            ) {
                return Err(
                    "integer binary requirement has inconsistent operand or result carriers",
                );
            }
            Ok(())
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

// A literal without a landing takes its type from the surrounding expression.
fn scalar_carrier(value: &PackageReviewScalarExpression) -> Option<PackageReviewPrimitiveType> {
    use PackageReviewPrimitiveType as Primitive;
    use PackageReviewScalarExpression as Expression;
    match value {
        Expression::IntegerLiteral(literal) => {
            literal.landing.as_ref().map(|landing| landing.landed_type)
        }
        Expression::Parameter { primitive_type, .. }
        | Expression::Local { primitive_type, .. }
        | Expression::StructuralParameterField { primitive_type, .. }
        | Expression::StructuralParameterIndexedRead { primitive_type, .. }
        | Expression::IntegerBinary { primitive_type, .. }
        | Expression::IntegerBitwiseNot { primitive_type, .. }
        | Expression::IntegerWiden { primitive_type, .. }
        | Expression::IntegerExactCast { primitive_type, .. }
        | Expression::RepresentationView { primitive_type, .. } => Some(*primitive_type),
        Expression::StructuralParameterByteLength { .. } => Some(Primitive::U64),
        Expression::Float32Literal(_) => Some(Primitive::F32),
        Expression::Float64Literal(_) => Some(Primitive::F64),
        Expression::Boolean(_) => Some(Primitive::Bool),
    }
}

fn integer_binary_types(
    kind: crate::record::PackageReviewIntegerBinaryKind,
    result: PackageReviewPrimitiveType,
    left: Option<PackageReviewPrimitiveType>,
    right: Option<PackageReviewPrimitiveType>,
) -> bool {
    use crate::record::PackageReviewIntegerBinaryKind as Binary;
    use PackageReviewPrimitiveType as Primitive;
    let matches = |left_type, right_type| {
        left.is_none_or(|actual| actual == left_type)
            && right.is_none_or(|actual| actual == right_type)
    };
    // An address has its own arithmetic. Offsets and address differences are u64.
    if result == Primitive::Addr {
        return match kind {
            Binary::ExactAdd => {
                matches(Primitive::Addr, Primitive::U64) || matches(Primitive::U64, Primitive::Addr)
            }
            Binary::ExactSubtract => matches(Primitive::Addr, Primitive::U64),
            _ => false,
        };
    }
    if !result.is_integer() {
        return false;
    }
    match kind {
        Binary::ExactSubtract if result == Primitive::U64 => {
            matches(result, result) || matches(Primitive::Addr, Primitive::Addr)
        }
        Binary::WrappingShiftLeft
        | Binary::WrappingShiftRight
        | Binary::SaturatingShiftLeft
        | Binary::SaturatingShiftRight
        | Binary::TrappingShiftLeft
        | Binary::TrappingShiftRight
        | Binary::ExactShiftLeft
        | Binary::ExactShiftRight => {
            left.is_none_or(|actual| actual == result)
                && right.is_none_or(|actual| actual.is_integer())
        }
        _ => matches(result, result),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{
        PackageReviewIntegerBinaryKind as Binary, PackageReviewIntegerComparisonKind,
    };
    use PackageReviewPrimitiveType as Primitive;
    use PackageReviewScalarExpression as Scalar;

    #[test]
    fn integer_binary_requirements_retain_their_operand_and_result_types() {
        let byte = Primitive::U8;
        let count = Primitive::U64;
        let addr = Primitive::Addr;
        let float = Primitive::F32;
        let wide = Primitive::Integer(numerics::integer_carrier::FixedInteger::signed(257));
        let parameter = |position, primitive_type| {
            Box::new(Scalar::Parameter {
                position,
                primitive_type,
            })
        };
        for (expected, kind, result, left, right) in [
            (true, Binary::WrappingAdd, byte, byte, byte),
            (true, Binary::WrappingAdd, wide, wide, wide),
            (true, Binary::ExactAdd, addr, addr, count),
            (true, Binary::ExactSubtract, count, addr, addr),
            (true, Binary::ExactShiftLeft, count, count, byte),
            (false, Binary::WrappingAdd, float, float, float),
            (false, Binary::WrappingAdd, count, float, count),
            (false, Binary::WrappingAdd, count, byte, count),
            (false, Binary::ExactAdd, addr, addr, addr),
        ] {
            let value = Scalar::IntegerBinary {
                kind,
                primitive_type: result,
                left: parameter(0, left),
                right: parameter(1, right),
            };
            let requirement = PackageReviewBooleanExpression::IntegerComparison {
                kind: PackageReviewIntegerComparisonKind::Equal,
                left: Box::new(value),
                right: parameter(2, result),
            };
            assert_eq!(
                validate(&requirement, 3, 0).is_ok(),
                expected,
                "{kind:?}({left}, {right}) -> {result}"
            );
        }
    }
}
