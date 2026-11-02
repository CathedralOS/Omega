//! Positive IEEE bounds from a checked, reduced predicate application. Only
//! producer-authenticated relations and format-landed literals carry laws.
//! The kernel's Id, opaque operations, negation and disjunction grant none.

use super::{PreparedConstantMeaning, PreparedMathematicalView, application_spine};
use proof_admission::{Term, TermHandle};
use typed_trees::expression::ExpressionHandle;

pub struct MathematicalFloatPredicate {
    pub subject: ExpressionHandle,
    pub reference: typed_trees::types::TypeReferenceHandle,
    pub predicate: facts::FloatPredicate,
    pub operator: typed_trees::expression::BinaryOperator,
    pub bound: numerics::bignum::ExactFloat,
}

pub fn float_predicates(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<Vec<MathematicalFloatPredicate>> {
    let view = program.prepared_mathematics()?;
    let application = view.application(expression)?;
    let captures = view
        .applications
        .captures
        .span_or_empty(application.captures)
        .iter()
        .map(|capture| capture.expression)
        .collect::<Vec<_>>();
    let mut predicates = Vec::new();
    collect(
        program,
        view,
        application.term,
        &captures,
        &mut predicates,
        0,
    )?;
    Some(predicates)
}

fn collect(
    program: &dyn crate::CheckingInput,
    view: PreparedMathematicalView<'_>,
    term: TermHandle,
    captures: &[ExpressionHandle],
    predicates: &mut Vec<MathematicalFloatPredicate>,
    depth: usize,
) -> Option<()> {
    if depth >= 128 {
        return None;
    }
    match view.arena.get(term) {
        Term::Squash { ty } => collect(program, view, ty, captures, predicates, depth + 1),
        Term::Sigma { domain, codomain } if !matches!(view.arena.get(domain), Term::Two) => {
            collect(program, view, domain, captures, predicates, depth + 1)?;
            let mut extended = captures.to_vec();
            extended.push(ExpressionHandle::default());
            collect(program, view, codomain, &extended, predicates, depth + 1)
        }
        _ => {
            let (declaration, arguments) = application_spine(view, term)?;
            let PreparedConstantMeaning::FloatRelation {
                operator,
                format,
                reference,
            } = view.constant(declaration)?
            else {
                return None;
            };
            let [left, right] = arguments.as_slice() else {
                return None;
            };
            let literal = |term| {
                // Literal actuals retain their source capture instead of
                // becoming body constants. Read only that immutable literal,
                // landing directly in the relation's checked format.
                if let Term::Variable(position) = view.arena.get(term) {
                    let expression =
                        *captures.get(captures.len().checked_sub(position as usize + 1)?)?;
                    if !expression.is_valid() {
                        return None;
                    }
                    let typed_trees::expression::ExpressionNode::Float(literal) =
                        program.expression_table.peeled_expression(expression)
                    else {
                        return None;
                    };
                    if literal.landing().is_some_and(|landing| landing != *format) {
                        return None;
                    }
                    return Some(match format {
                        numerics::literals::FloatFormat::F32 => {
                            numerics::bignum::ExactFloat::from_f32(literal.value_f32())
                        }
                        numerics::literals::FloatFormat::F64 => {
                            numerics::bignum::ExactFloat::from_f64(literal.value_f64())
                        }
                    });
                }
                let (declaration, arguments) = application_spine(view, term)?;
                if !arguments.is_empty() {
                    return None;
                }
                let PreparedConstantMeaning::FloatLiteral {
                    format: retained,
                    bits,
                } = view.constant(declaration)?
                else {
                    return None;
                };
                if retained != format {
                    return None;
                }
                Some(match format {
                    numerics::literals::FloatFormat::F32 => {
                        numerics::bignum::ExactFloat::from_f32(f32::from_bits(*bits as u32))
                    }
                    numerics::literals::FloatFormat::F64 => {
                        numerics::bignum::ExactFloat::from_f64(f64::from_bits(*bits))
                    }
                })
            };
            let (subject, bound, operator) = if let Some(bound) = literal(*right) {
                (*left, bound, *operator)
            } else {
                (*right, literal(*left)?, operator.mirrored())
            };
            let Term::Variable(position) = view.arena.get(subject) else {
                return None;
            };
            let subject = *captures.get(captures.len().checked_sub(position as usize + 1)?)?;
            if !subject.is_valid() {
                return None;
            }
            let predicate = facts::FloatPredicate::comparison(*format, operator, &bound)?;
            predicates.push(MathematicalFloatPredicate {
                subject,
                reference: *reference,
                predicate,
                operator,
                bound,
            });
            Some(())
        }
    }
}
