//! Project an already checked, reduced mathematical predicate into the common
//! scalar contract representation. Captures are rebound by the caller's normal
//! observation path; no authored body is reopened and no storage permission is
//! inferred. Only the retained integer relation vocabulary is interpreted.
//! Opaque/custom/IEEE operations and unrepresentable mathematical operands
//! decline rather than acquiring finite arithmetic or comparison laws.

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
    mut capture: impl FnMut(ExpressionHandle) -> Option<Scalar>,
) -> Option<Boolean> {
    let view = program.prepared_mathematics()?;
    let application = view.application(expression)?;
    let captures = view
        .applications
        .captures
        .span_or_empty(application.captures)
        .iter()
        .map(|row| capture(row.expression).map(Some))
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
        return value.primitive_type()?.is_fixed_integer().then_some(value);
    }
    let (declaration, arguments) = application_spine(view, term)?;
    if !arguments.is_empty() {
        return None;
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
