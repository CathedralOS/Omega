//! Recover strict order from the entry contract's adjacent inclusive encoding.
//! The existing kernel rule checks the conversion; this is certificate search,
//! not an additional source of numeric facts or a normalization rule.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm};

pub(super) fn from_premise(goal: &Proposition, premise: &ProofNode) -> Option<ProofNode> {
    let Proposition::LessThan(left, right) = goal else {
        return None;
    };
    let Proposition::LessOrEqual(inclusive_left, inclusive_right) = &premise.conclusion else {
        return None;
    };
    if left.scalar_type() != right.scalar_type() {
        return None;
    }
    if (right == inclusive_right && adjacent(left, true).as_ref() == Some(inclusive_left))
        || (left == inclusive_left && adjacent(right, false).as_ref() == Some(inclusive_right))
    {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerOrderDiscreteness {
                relation: Box::new(premise.clone()),
            },
        });
    }
    None
}

fn adjacent(literal: &ScalarTerm, increasing: bool) -> Option<ScalarTerm> {
    let (integer_type, value) = literal.integer_value()?;
    if integer_type.carrier() != IntegerCarrier::Fixed || integer_type.is_address() {
        return None;
    }
    let value = match (value, increasing) {
        (IntegerValue::Signed(value), true) => IntegerValue::Signed(value.checked_add(1)?),
        (IntegerValue::Signed(value), false) => IntegerValue::Signed(value.checked_sub(1)?),
        (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value.checked_add(1)?),
        (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value.checked_sub(1)?),
    };
    ScalarTerm::integer(integer_type, value).ok()
}

#[cfg(test)]
mod tests;
