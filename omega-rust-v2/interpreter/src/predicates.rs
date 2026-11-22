//! Evaluate a data declaration's `requires` clause on the value an operation
//! just established. The checker proves the clause for every execution; this
//! is the executor's own check of the one value it holds, so a false clause
//! stops the invocation instead of publishing the value.
//!
//! Only closed forms are evaluated: the subject, its declared fields and fixed
//! elements, integer and Boolean literals, and the comparison and Boolean
//! operators. Admission refuses a schema whose clause uses any other step.
use crate::places::field_position;
use crate::values::Value;
use omega_psi::{ContractClause, LoweredPackage, PredicateOperator, PredicateStep};

pub(crate) fn evaluable(clause: &ContractClause) -> bool {
    !clause.predicate.is_empty()
        && clause.predicate.iter().all(|step| {
            matches!(
                step,
                PredicateStep::SelfValue
                    | PredicateStep::Field(_)
                    | PredicateStep::FixedIndex(_)
                    | PredicateStep::Integer(_)
                    | PredicateStep::Boolean(_)
                    | PredicateStep::Apply(_)
            )
        })
}

enum Term<'value> {
    Subject(&'value Value),
    Literal(Value),
}

impl Term<'_> {
    fn value(&self) -> &Value {
        match self {
            Self::Subject(value) => value,
            Self::Literal(value) => value,
        }
    }
}

/// `Some(truth)` of the clause for `subject`, or `None` when the clause does
/// not denote a Boolean over this value.
pub(crate) fn holds(
    unit: &LoweredPackage,
    clause: &ContractClause,
    subject: &Value,
) -> Option<bool> {
    let mut stack: Vec<Term<'_>> = Vec::new();
    for step in &clause.predicate {
        match step {
            PredicateStep::SelfValue => stack.push(Term::Subject(subject)),
            PredicateStep::Integer(value) => stack.push(Term::Literal(Value::Integer(value.clone()))),
            PredicateStep::Boolean(value) => stack.push(Term::Literal(Value::Boolean(*value))),
            PredicateStep::Field(declaration) => {
                let Term::Subject(Value::Record { data, fields }) = stack.pop()? else {
                    return None;
                };
                stack.push(Term::Subject(fields.get(field_position(unit, *data, None, declaration)?)?));
            }
            PredicateStep::FixedIndex(index) => {
                let Term::Subject(Value::Array(elements)) = stack.pop()? else { return None };
                stack.push(Term::Subject(elements.get(usize::try_from(*index).ok()?)?));
            }
            PredicateStep::Apply(PredicateOperator::Not) => {
                let operand = stack.pop()?;
                let Value::Boolean(operand) = operand.value() else { return None };
                stack.push(Term::Literal(Value::Boolean(!operand)));
            }
            PredicateStep::Apply(operator) => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                let truth = match (left.value(), right.value(), operator) {
                    (Value::Boolean(left), Value::Boolean(right), PredicateOperator::And) => *left && *right,
                    (Value::Boolean(left), Value::Boolean(right), PredicateOperator::Or) => *left || *right,
                    (Value::Boolean(left), Value::Boolean(right), PredicateOperator::Equal) => left == right,
                    (Value::Boolean(left), Value::Boolean(right), PredicateOperator::NotEqual) => left != right,
                    (Value::Integer(left), Value::Integer(right), operator) => match operator {
                        PredicateOperator::Equal => left == right,
                        PredicateOperator::NotEqual => left != right,
                        PredicateOperator::LessThan => left < right,
                        PredicateOperator::LessOrEqual => left <= right,
                        PredicateOperator::GreaterThan => left > right,
                        PredicateOperator::GreaterOrEqual => left >= right,
                        PredicateOperator::Not | PredicateOperator::And | PredicateOperator::Or => return None,
                    },
                    _ => return None,
                };
                stack.push(Term::Literal(Value::Boolean(truth)));
            }
            _ => return None,
        }
    }
    match (stack.pop()?, stack.is_empty()) {
        (term, true) => match term.value() {
            Value::Boolean(truth) => Some(*truth),
            _ => None,
        },
        _ => None,
    }
}
