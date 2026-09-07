//! Fixed discrete order conversion with one exact literal endpoint.

use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm};

use super::ProofError;

pub(super) fn check(relation: &Proposition, conclusion: &Proposition) -> Result<(), ProofError> {
    let Proposition::LessOrEqual(left, right) = relation else {
        return Err(ProofError::RulePremiseMismatch(
            "integer order discreteness",
        ));
    };
    let Proposition::LessThan(strict_left, strict_right) = conclusion else {
        return Err(ProofError::IntegerOrderConclusionMismatch);
    };
    if left.scalar_type() != right.scalar_type() {
        return Err(ProofError::RulePremiseMismatch(
            "integer order discreteness",
        ));
    }
    ((right == strict_right && adjacent(left, false).as_ref() == Some(strict_left))
        || (left == strict_left && adjacent(right, true).as_ref() == Some(strict_right)))
    .then_some(())
    .ok_or(ProofError::IntegerOrderConclusionMismatch)
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
