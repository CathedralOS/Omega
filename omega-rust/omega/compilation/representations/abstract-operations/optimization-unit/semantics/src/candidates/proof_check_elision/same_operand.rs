//! Same-operand exact integer laws: `x - x = 0`, `x % x = 0` and `x / x = 1`.
//!
//! Each law recognizes its own operator, overflow policy and carrier
//! admission; its whole evidence is the authored operation's accepted
//! obligation, and the acceptance protocol itself is shared.
use super::integer_constant_law::{
    IndependentIntegerConstantLawShape, accepted_law_obligation, constant_law_shape,
};
use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::{IntegerCarrier, IntegerSign};

use crate::OptimizationUnitValidationError;

/// The exact symbolic law `x - x = 0`: subtract under the exact policy, where
/// the admitted operation carries its range obligation.
pub(super) fn exact_integer_self_subtract_shape(
    operation: &O,
) -> Option<IndependentIntegerConstantLawShape> {
    let operands = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Subtract)?;
    if operands.arithmetic.policy() != IntegerOverflowPolicy::Exact
        || operands.left != operands.right
    {
        return None;
    }
    constant_law_shape(&operands, operands.scalar_type.zero(), None)
}

/// The defined remainder law `x % x = 0` for exact, wrapping and saturating
/// fixed-width integers. The accepted obligation is required because it is the
/// capability proving the authored divisor legal; no operand constant or range
/// fact is inferred.
pub(super) fn integer_self_remainder_shape(
    operation: &O,
) -> Option<IndependentIntegerConstantLawShape> {
    let operands = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Remainder)?;
    if operands.left != operands.right {
        return None;
    }
    constant_law_shape(&operands, operands.scalar_type.zero(), None)
}

/// The defined division law `x / x = 1` for exact, wrapping and saturating
/// fixed-width integers. The accepted obligation proves the authored divisor
/// nonzero and the signed overflow case absent; signed one-bit integers are
/// excluded because typed positive one is not representable.
pub(super) fn integer_self_divide_shape(
    operation: &O,
) -> Option<IndependentIntegerConstantLawShape> {
    let operands = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Divide)?;
    let positive_one_is_representable = operands.scalar_type.carrier() == IntegerCarrier::Fixed
        && !(operands.scalar_type.sign() == IntegerSign::Signed
            && operands.scalar_type.bits() == 1);
    if !positive_one_is_representable || operands.left != operands.right {
        return None;
    }
    constant_law_shape(&operands, operands.scalar_type.one(), None)
}

/// The same-operand laws' evidence: the accepted obligation of the operation
/// the law consumed, and nothing else.
pub(super) fn accepted_operation_obligation_evidence(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    shape: &IndependentIntegerConstantLawShape,
    candidate: &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError> {
    let expected_fact = accepted_law_obligation(input, function, shape)?;
    if candidate.accepted_obligation_witness() != Some(expected_fact) {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    Ok(())
}
