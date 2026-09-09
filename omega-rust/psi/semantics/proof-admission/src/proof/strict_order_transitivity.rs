//! Strict or mixed integer order composition with exact typed endpoints.
//!
//! Keep this separate from the nonstrict rule: weakening loses the strict
//! conclusion, and literal adjacency cannot compose symbolic endpoints. Both
//! cited children are checked by the ordinary kernel traversal before this
//! rule combines their relations; search cannot supply an inferred axiom.

use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarTerm, ScalarType};

use super::ProofError;

pub(super) fn check(
    left_to_middle: &Proposition,
    middle_to_right: &Proposition,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    if let (Some(first), Some(second), Proposition::LessThan(left, right)) = (
        scalar_order(left_to_middle),
        scalar_order(middle_to_right),
        conclusion,
    ) {
        let scalar_type = first.1.scalar_type();
        if !matches!(scalar_type, ScalarType::Integer(_))
            || [first.2, second.1, second.2, left, right]
                .iter()
                .any(|term| term.scalar_type() != scalar_type)
        {
            return Err(ProofError::RulePremiseMismatch(
                "strict integer order carrier",
            ));
        }
        return endpoints(first, second, left, right);
    }
    if let (Some(first), Some(second), Proposition::IntegerMathLessThan(left, right)) = (
        mathematical_order(left_to_middle),
        mathematical_order(middle_to_right),
        conclusion,
    ) {
        return endpoints(first, second, left, right);
    }
    Err(ProofError::RulePremiseMismatch(
        "strict integer order transitivity",
    ))
}

fn endpoints<Term: Eq>(
    first: (bool, &Term, &Term),
    second: (bool, &Term, &Term),
    left: &Term,
    right: &Term,
) -> Result<(), ProofError> {
    if !first.0 && !second.0 {
        return Err(ProofError::RulePremiseMismatch("strict integer order edge"));
    }
    if first.2 != second.1 {
        return Err(ProofError::IntegerOrderMiddleMismatch);
    }
    (first.1 == left && second.2 == right)
        .then_some(())
        .ok_or(ProofError::IntegerOrderConclusionMismatch)
}

fn scalar_order(proposition: &Proposition) -> Option<(bool, &ScalarTerm, &ScalarTerm)> {
    match proposition {
        Proposition::LessThan(left, right) => Some((true, left, right)),
        Proposition::LessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

fn mathematical_order(
    proposition: &Proposition,
) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match proposition {
        Proposition::IntegerMathLessThan(left, right) => Some((true, left, right)),
        Proposition::IntegerMathLessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
