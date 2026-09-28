//! Fixed discrete order conversion with one exact literal endpoint, in
//! either direction between non-strict and strict order.

use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm};

use super::ProofError;

pub(crate) fn check(relation: &Proposition, conclusion: &Proposition) -> Result<(), ProofError> {
    match (relation, conclusion) {
        // `a <= b` moves one literal endpoint outward: `a - 1 < b` or `a < b + 1`.
        (
            Proposition::LessOrEqual(left, right),
            Proposition::LessThan(strict_left, strict_right),
        ) => {
            same_carrier(left, right)?;
            ((right == strict_right && adjacent(left, false).as_ref() == Some(strict_left))
                || (left == strict_left && adjacent(right, true).as_ref() == Some(strict_right)))
            .then_some(())
            .ok_or(ProofError::IntegerOrderConclusionMismatch)
        }
        // `a < b` moves one literal endpoint inward: `a + 1 <= b` or `a <= b - 1`.
        (Proposition::LessThan(left, right), Proposition::LessOrEqual(bound_left, bound_right)) => {
            same_carrier(left, right)?;
            ((right == bound_right && adjacent(left, true).as_ref() == Some(bound_left))
                || (left == bound_left && adjacent(right, false).as_ref() == Some(bound_right)))
            .then_some(())
            .ok_or(ProofError::IntegerOrderConclusionMismatch)
        }
        (Proposition::LessOrEqual(_, _) | Proposition::LessThan(_, _), _) => {
            Err(ProofError::IntegerOrderConclusionMismatch)
        }
        _ => Err(ProofError::RulePremiseMismatch(
            "integer order discreteness",
        )),
    }
}

fn same_carrier(left: &ScalarTerm, right: &ScalarTerm) -> Result<(), ProofError> {
    (left.scalar_type() == right.scalar_type())
        .then_some(())
        .ok_or(ProofError::RulePremiseMismatch(
            "integer order discreteness",
        ))
}

fn adjacent(literal: &ScalarTerm, increasing: bool) -> Option<ScalarTerm> {
    let (integer_type, value) = literal.integer_value()?;
    if integer_type.carrier() != IntegerCarrier::Fixed || integer_type.is_address() {
        return None;
    }
    let adjacent = match (value, increasing) {
        (IntegerValue::Signed(value), true) => IntegerValue::Signed(value.checked_add(1)?),
        (IntegerValue::Signed(value), false) => IntegerValue::Signed(value.checked_sub(1)?),
        (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value.checked_add(1)?),
        (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value.checked_sub(1)?),
    };
    ScalarTerm::integer(integer_type, adjacent).ok()
}

#[cfg(test)]
mod tests;
