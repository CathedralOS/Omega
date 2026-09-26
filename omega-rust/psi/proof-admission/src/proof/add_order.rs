//! Strict order after adding an independently proved positive integer.

use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm, ScalarType};

use super::ProofError;

/// From exact fixed-integer `result = original + increment` and
/// `0 < increment`, conclude `original < result`: the mirror of
/// `IntegerSubtractOrder`. The addition is mathematical, so the conclusion
/// holds for every representable `result`; only exact addition qualifies.
pub(crate) fn check(
    sum: &Proposition,
    positive: &Proposition,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    let Proposition::Equal(
        result,
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        },
    ) = sum
    else {
        return Err(ProofError::RulePremiseMismatch("integer addition order"));
    };
    let Proposition::LessThan(zero, increment) = positive else {
        return Err(ProofError::RulePremiseMismatch(
            "integer addition positivity",
        ));
    };
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || scalar_type.is_address()
        || result.scalar_type() != ScalarType::Integer(*scalar_type)
        || zero.scalar_type() != ScalarType::Integer(*scalar_type)
        || increment != right.as_ref()
        || !matches!(
            zero.integer_value(),
            Some((_, IntegerValue::Signed(0) | IntegerValue::Unsigned(0)))
        )
    {
        return Err(ProofError::RulePremiseMismatch("integer addition order"));
    }
    (conclusion == &Proposition::LessThan(left.as_ref().clone(), result.clone()))
        .then_some(())
        .ok_or(ProofError::IntegerOrderConclusionMismatch)
}
