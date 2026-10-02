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
        Expression::ByteSequenceEqual { left, right } => {
            field(left.parameter_position, &left.path, parameters, depth + 1)?;
            field(right.parameter_position, &right.path, parameters, depth + 1)
        }
        Expression::PayloadlessSumEqual { left, right, cases } => {
            field(left.parameter_position, &left.path, parameters, depth + 1)?;
            field(right.parameter_position, &right.path, parameters, depth + 1)?;
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
        | Expression::IntegerWrappingCast { operand, .. }
        | Expression::IntegerSaturatingCast { operand, .. }
        | Expression::IntegerTrappingCast { operand, .. }
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

#[cfg(test)]
mod tests {
    use super::PackageReviewStructuralPredicatePathSegment;
    use crate::record::package::callable_policy::validation::structural::field;

    #[test]
    fn fixed_index_is_numeric_while_named_segments_and_parameter_scope_remain_checked() {
        use PackageReviewStructuralPredicatePathSegment as Segment;
        assert!(field(0, &[Segment::FixedIndex(0)], 1, 0).is_ok());
        assert!(field(0, &[Segment::FixedIndex(u64::MAX)], 1, 0).is_ok());
        assert!(field(1, &[Segment::FixedIndex(0)], 1, 0).is_err());
        for empty in [Segment::Field(String::new()), Segment::Case(String::new())] {
            assert!(field(0, &[Segment::FixedIndex(0), empty], 1, 0).is_err());
        }
    }

    #[test]
    fn runtime_path_indexes_validate_their_own_scope_recursively() {
        use super::PackageReviewRuntimeIndex as Index;
        use crate::record::PackageReviewStructuralParameterField as Field;
        use PackageReviewStructuralPredicatePathSegment as Segment;
        let wrap = |index| {
            Segment::RuntimeIndex(Index::StructuralField(Field {
                parameter_position: 0,
                path: vec![Segment::RuntimeIndex(index)],
            }))
        };
        assert!(field(0, &[wrap(Index::Parameter { position: 1 })], 2, 0).is_ok());
        for index in [
            Index::Parameter { position: 2 },
            Index::Local { position: 0 },
            Index::AssignmentIndex { depth: 0 },
        ] {
            assert!(field(0, &[wrap(index)], 2, 0).is_err());
        }
        let mut path = vec![Segment::Field("value".into())];
        for _ in 0..64 {
            path = vec![Segment::RuntimeIndex(Index::StructuralField(Field {
                parameter_position: 0,
                path,
            }))];
        }
        assert!(field(0, &path, 1, 0).is_err());
    }

    #[test]
    fn indexed_scalar_scope_checks_cast_operands_and_every_projection() {
        use super::{
            PackageReviewPrimitiveType as Primitive, PackageReviewRuntimeIndex as Index,
            PackageReviewScalarExpression as Expression, scalar,
        };
        use PackageReviewStructuralPredicatePathSegment as Segment;
        let parameter = || Expression::Parameter {
            position: 1,
            primitive_type: Primitive::U64,
        };
        let valid = Expression::StructuralParameterIndexedRead {
            parameter_position: 0,
            path: vec![Segment::Field("rows".into())],
            index: Box::new(parameter()),
            inner_indexes: vec![Expression::IntegerWrappingCast {
                primitive_type: Primitive::U64,
                operand: Box::new(parameter()),
            }],
            element_path: vec![Segment::Field("value".into())],
            primitive_type: Primitive::U8,
        };
        assert!(scalar(&valid, 2, 0).is_ok());
        let mut bad_root = valid.clone();
        let Expression::StructuralParameterIndexedRead {
            parameter_position, ..
        } = &mut bad_root
        else {
            unreachable!()
        };
        *parameter_position = 2;
        assert!(scalar(&bad_root, 2, 0).is_err());
        let mut bad_inner = valid.clone();
        let Expression::StructuralParameterIndexedRead { inner_indexes, .. } = &mut bad_inner
        else {
            unreachable!()
        };
        inner_indexes.push(Expression::IntegerTrappingCast {
            primitive_type: Primitive::U8,
            operand: Box::new(Expression::Local {
                position: 0,
                primitive_type: Primitive::U32,
            }),
        });
        assert!(scalar(&bad_inner, 2, 0).is_err());
        let mut bad_path = valid;
        let Expression::StructuralParameterIndexedRead { element_path, .. } = &mut bad_path else {
            unreachable!()
        };
        element_path.push(Segment::RuntimeIndex(Index::Parameter { position: 2 }));
        assert!(scalar(&bad_path, 2, 0).is_err());
    }
}
