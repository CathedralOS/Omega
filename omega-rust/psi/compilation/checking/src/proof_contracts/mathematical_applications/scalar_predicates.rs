//! Project an already checked, reduced mathematical predicate into the common
//! scalar contract representation. Captures are rebound by the caller's normal
//! observation path; no authored body is reopened and no storage permission is
//! inferred. Retained integer and authenticated IEEE relations keep distinct
//! comparison vocabularies. Opaque/custom operations and unrepresentable
//! mathematical operands decline rather than acquiring scalar laws.

use super::{PreparedConstantMeaning, PreparedMathematicalView, application_spine};
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    CheckedScalarExpression as Scalar,
};
use proof_admission::{Term, TermHandle};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::PrimitiveType;

pub fn scalar_predicate(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    mut capture: impl FnMut(ExpressionHandle, typed_trees::types::TypeReferenceHandle) -> Option<Scalar>,
) -> Option<Boolean> {
    let view = program.prepared_mathematics()?;
    let application = view.application(expression)?;
    let captures = view
        .applications
        .captures
        .span_or_empty(application.captures)
        .iter()
        .map(|row| capture(row.expression, row.source_type).map(Some))
        .collect::<Option<Vec<_>>>()?;
    predicate(view, application.term, &captures, 0)
}

fn predicate(
    view: PreparedMathematicalView<'_>,
    term: TermHandle,
    captures: &[Option<Scalar>],
    depth: usize,
) -> Option<Boolean> {
    if depth >= 128 {
        return None;
    }
    match view.arena.get(term) {
        Term::Squash { ty } => predicate(view, ty, captures, depth + 1),
        Term::Id { ty, left, right } => {
            if matches!(view.arena.get(ty), Term::Two) {
                return match (view.arena.get(left), view.arena.get(right)) {
                    (Term::TwoZero, Term::TwoZero) | (Term::TwoOne, Term::TwoOne) => {
                        Some(Boolean::Constant(true))
                    }
                    (Term::TwoZero, Term::TwoOne) | (Term::TwoOne, Term::TwoZero) => {
                        Some(Boolean::Constant(false))
                    }
                    _ => None,
                };
            }
            let Term::Constant { declaration, .. } = view.arena.get(ty) else {
                return None;
            };
            if !matches!(
                view.constant(declaration),
                Some(PreparedConstantMeaning::IntegerCarrier)
            ) {
                return None;
            }
            comparison(view, BinaryOperator::Equal, left, right, captures)
        }
        Term::Sigma { domain, codomain } => {
            let mut extended = captures.to_vec();
            extended.push(None);
            if matches!(view.arena.get(domain), Term::Two) {
                let Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    scrutinee,
                    ..
                } = view.arena.get(codomain)
                else {
                    return None;
                };
                if view.arena.get(scrutinee) != Term::Variable(0) {
                    return None;
                }
                Some(Boolean::Or {
                    left: Box::new(predicate(view, zero_branch, &extended, depth + 1)?),
                    right: Box::new(predicate(view, one_branch, &extended, depth + 1)?),
                })
            } else {
                Some(Boolean::And {
                    left: Box::new(predicate(view, domain, captures, depth + 1)?),
                    right: Box::new(predicate(view, codomain, &extended, depth + 1)?),
                })
            }
        }
        _ => {
            let (declaration, arguments) = application_spine(view, term)?;
            match (view.constant(declaration)?, arguments.as_slice()) {
                (PreparedConstantMeaning::Negation, [operand]) => Some(Boolean::Not(Box::new(
                    predicate(view, *operand, captures, depth + 1)?,
                ))),
                (PreparedConstantMeaning::IntegerRelation(operator), [left, right]) => {
                    comparison(view, *operator, *left, *right, captures)
                }
                (
                    PreparedConstantMeaning::FloatRelation {
                        operator, format, ..
                    },
                    [left, right],
                ) => {
                    let primitive = match format {
                        numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                        numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                    };
                    let left = scalar(view, *left, captures, Some(primitive))?;
                    let right = scalar(view, *right, captures, Some(primitive))?;
                    if left.primitive_type() != Some(primitive)
                        || right.primitive_type() != Some(primitive)
                    {
                        return None;
                    }
                    use checked_trees::CheckedIeeeFloatComparisonKind as FloatComparison;
                    let (kind, left, right) = match operator {
                        BinaryOperator::Equal => (FloatComparison::Equal, left, right),
                        BinaryOperator::NotEqual => (FloatComparison::NotEqual, left, right),
                        BinaryOperator::Less => (FloatComparison::LessThan, left, right),
                        BinaryOperator::LessOrEqual => (FloatComparison::LessOrEqual, left, right),
                        BinaryOperator::Greater => (FloatComparison::LessThan, right, left),
                        BinaryOperator::GreaterOrEqual => {
                            (FloatComparison::LessOrEqual, right, left)
                        }
                        _ => return None,
                    };
                    Some(Boolean::ScalarIeeeFloatComparison {
                        kind,
                        left: Box::new(left),
                        right: Box::new(right),
                    })
                }
                _ => None,
            }
        }
    }
}

fn scalar(
    view: PreparedMathematicalView<'_>,
    term: TermHandle,
    captures: &[Option<Scalar>],
    expected: Option<PrimitiveType>,
) -> Option<Scalar> {
    if let Term::Variable(index) = view.arena.get(term) {
        let value = captures
            .get(captures.len().checked_sub(index as usize + 1)?)?
            .clone()?;
        let primitive = value.primitive_type()?;
        return (primitive.is_fixed_integer()
            || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64))
        .then_some(value);
    }
    let (declaration, arguments) = application_spine(view, term)?;
    if !arguments.is_empty() {
        return None;
    }
    if let PreparedConstantMeaning::FloatLiteral { format, bits } = view.constant(declaration)? {
        let (primitive, value) = match format {
            numerics::literals::FloatFormat::F32 => (
                PrimitiveType::F32,
                semantic_vocabulary::IeeeFloatValue::Binary32(*bits as u32),
            ),
            numerics::literals::FloatFormat::F64 => (
                PrimitiveType::F64,
                semantic_vocabulary::IeeeFloatValue::Binary64(*bits),
            ),
        };
        if expected.is_some_and(|expected| expected != primitive) {
            return None;
        }
        return Some(Scalar::IeeeFloatLiteral { value });
    }
    let PreparedConstantMeaning::Integer(value) = view.constant(declaration)? else {
        return None;
    };
    Some(Scalar::IntegerLiteral {
        literal: crate::land_integer_value(value, expected?)?,
    })
}

fn comparison(
    view: PreparedMathematicalView<'_>,
    operator: BinaryOperator,
    left: TermHandle,
    right: TermHandle,
    captures: &[Option<Scalar>],
) -> Option<Boolean> {
    let left_value = scalar(view, left, captures, None);
    let right_value = scalar(view, right, captures, None);
    let primitive = left_value
        .as_ref()
        .or(right_value.as_ref())?
        .primitive_type()?;
    if !primitive.is_fixed_integer() {
        return None;
    }
    let left = left_value.or_else(|| scalar(view, left, captures, Some(primitive)))?;
    let right = right_value.or_else(|| scalar(view, right, captures, Some(primitive)))?;
    let (kind, left, right, negated) = match operator {
        BinaryOperator::Equal => (Comparison::Equal, left, right, false),
        BinaryOperator::NotEqual => (Comparison::Equal, left, right, true),
        BinaryOperator::Less => (Comparison::LessThan, left, right, false),
        BinaryOperator::LessOrEqual => (Comparison::LessOrEqual, left, right, false),
        BinaryOperator::Greater => (Comparison::LessThan, right, left, false),
        BinaryOperator::GreaterOrEqual => (Comparison::LessOrEqual, right, left, false),
        _ => return None,
    };
    let result = Boolean::IntegerComparison {
        kind,
        left: Box::new(left),
        right: Box::new(right),
    };
    Some(if negated {
        Boolean::Not(Box::new(result))
    } else {
        result
    })
}
