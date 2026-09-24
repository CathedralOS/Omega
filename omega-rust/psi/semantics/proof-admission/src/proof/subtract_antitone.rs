//! Strict order between two exact differences from one minuend.

use semantic_vocabulary::{IntegerCarrier, Proposition, ScalarTerm, ScalarType};

use super::ProofError;

/// From exact fixed-integer `smaller = minuend - large`, `larger = minuend -
/// small` and `small < large`, conclude `smaller < larger`: subtraction is
/// strictly antitone in its subtrahend. Both differences name the same exact
/// minuend term; each subtrahend is matched to the order's endpoint exactly.
pub(crate) fn check(
    smaller: &Proposition,
    larger: &Proposition,
    order: &Proposition,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    let difference = |proposition: &Proposition| match proposition {
        Proposition::Equal(
            result,
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            },
        ) => Some((
            result.clone(),
            *scalar_type,
            left.as_ref().clone(),
            right.as_ref().clone(),
        )),
        _ => None,
    };
    let (
        Some((smaller_result, scalar_type, minuend, large)),
        Some((larger_result, larger_type, larger_minuend, small)),
    ) = (difference(smaller), difference(larger))
    else {
        return Err(ProofError::RulePremiseMismatch(
            "integer subtraction antitonicity",
        ));
    };
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || scalar_type.is_address()
        || larger_type != scalar_type
        || larger_minuend != minuend
        || smaller_result.scalar_type() != ScalarType::Integer(scalar_type)
        || larger_result.scalar_type() != ScalarType::Integer(scalar_type)
        || order != &Proposition::LessThan(small, large)
    {
        return Err(ProofError::RulePremiseMismatch(
            "integer subtraction antitonicity",
        ));
    }
    (conclusion == &Proposition::LessThan(smaller_result, larger_result))
        .then_some(())
        .ok_or(ProofError::IntegerOrderConclusionMismatch)
}
