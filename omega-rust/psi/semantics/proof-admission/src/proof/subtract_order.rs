//! Strict order after subtracting an independently proved positive integer.

use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm, ScalarType};

use super::ProofError;

pub(super) fn check(
    difference: &Proposition,
    positive: &Proposition,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    let Proposition::Equal(
        result,
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        },
    ) = difference
    else {
        return Err(ProofError::RulePremiseMismatch("integer subtraction order"));
    };
    let Proposition::LessThan(zero, decrement) = positive else {
        return Err(ProofError::RulePremiseMismatch(
            "integer subtraction positivity",
        ));
    };
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || scalar_type.is_address()
        || result.scalar_type() != ScalarType::Integer(*scalar_type)
        || zero.scalar_type() != ScalarType::Integer(*scalar_type)
        || decrement != right.as_ref()
        || !matches!(
            zero.integer_value(),
            Some((_, IntegerValue::Signed(0) | IntegerValue::Unsigned(0)))
        )
    {
        return Err(ProofError::RulePremiseMismatch("integer subtraction order"));
    }
    (conclusion == &Proposition::LessThan(result.clone(), left.as_ref().clone()))
        .then_some(())
        .ok_or(ProofError::IntegerOrderConclusionMismatch)
}
