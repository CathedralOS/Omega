//! Exact remainder-by-unit laws `x % 1 = 0` and `x % -1 = 0`.
//!
//! Both laws are written against a divisor that is a materialized typed
//! literal of the dividend's own type, so their evidence is that literal's
//! accepted constant fact together with the authored operation's accepted
//! obligation.
use super::integer_constant_law::{
    IndependentIntegerConstantLawShape, accepted_law_obligation, constant_law_shape,
};
use abstract_operations::{AbstractOperation as O, IntegerArithmeticOperator};
use optimization_unit::{
    OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate,
    ScalarConstantValue, ValueDefinitionSite, literal_scalar_constant_fact_identity,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType, IntegerValue, ScalarType};

use crate::OptimizationUnitValidationError;

/// The one divisor literal a remainder-by-unit law is written against: typed
/// positive one, or typed negative one for a signed carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndependentRemainderUnitDivisor {
    PositiveOne,
    SignedNegativeOne,
}

impl IndependentRemainderUnitDivisor {
    fn value(self, scalar_type: IntegerType) -> Option<IntegerValue> {
        match self {
            Self::PositiveOne => Some(scalar_type.one()),
            Self::SignedNegativeOne if scalar_type.sign() == IntegerSign::Signed => {
                Some(IntegerValue::Signed(-1))
            }
            Self::SignedNegativeOne => None,
        }
    }
}

/// The defined law `x % 1 = 0` for exact, wrapping and saturating fixed-width
/// integers.
pub(super) fn integer_remainder_by_one_shape(
    operation: &O,
) -> Option<IndependentIntegerConstantLawShape> {
    literal_divisor_shape(operation, IndependentRemainderUnitDivisor::PositiveOne)
}

/// The defined signed law `x % -1 = 0`; for exact arithmetic the accepted
/// obligation proves the otherwise exceptional signed-minimum input
/// unreachable.
pub(super) fn signed_integer_remainder_by_negative_one_shape(
    operation: &O,
) -> Option<IndependentIntegerConstantLawShape> {
    literal_divisor_shape(
        operation,
        IndependentRemainderUnitDivisor::SignedNegativeOne,
    )
}

/// Both laws' shape: a fixed-width remainder whose divisor is a different
/// value than the dividend, so the divisor's identity comes from its own
/// materialized literal; the replacement constant itself is zero.
fn literal_divisor_shape(
    operation: &O,
    divisor: IndependentRemainderUnitDivisor,
) -> Option<IndependentIntegerConstantLawShape> {
    let operands = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Remainder)?;
    if operands.scalar_type.carrier() != IntegerCarrier::Fixed || operands.left == operands.right {
        return None;
    }
    let divisor_literal = divisor.value(operands.scalar_type)?;
    constant_law_shape(
        &operands,
        operands.scalar_type.zero(),
        Some(divisor_literal),
    )
}

/// Both laws' evidence: the divisor operand must be that exact typed literal
/// carrying its accepted constant fact, and the authored operation must still
/// carry the accepted obligation the law consumed. Neither the dividend's
/// value nor any range fact is inferred from the literal.
pub(super) fn materialized_divisor_evidence(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    shape: &IndependentIntegerConstantLawShape,
    candidate: &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError> {
    let expected_divisor = shape
        .divisor_literal
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let right_definition =
        crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition(
            function,
            shape.right,
        )
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if right_definition.scalar_type != ScalarType::Integer(shape.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let ValueDefinitionSite::Node {
        block: literal_block,
        node: literal_node,
    } = right_definition.site
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let literal = function
        .block(literal_block)
        .and_then(|block| {
            usize::try_from(literal_node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let O::IntegerConstant {
        psi_operation: literal_support,
        result: literal_result,
        scalar_type: literal_type,
        value: literal_value,
    } = literal.operation
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    if literal_result != shape.right
        || literal_type != ScalarType::Integer(shape.scalar_type)
        || literal_value != expected_divisor
        || !shape.scalar_type.admits(expected_divisor)
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.right
                    && *constant == expected_divisor
                    && *support == literal_support)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected_constant_fact = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        right_definition,
        ScalarConstantValue::Integer(expected_divisor),
        literal_support,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let expected_obligation_fact = accepted_law_obligation(input, function, shape)?;
    let Some((constant_fact, obligation_fact)) =
        candidate.proof_certified_scalar_identity_witness()
    else {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    };
    if constant_fact != expected_constant_fact {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    if obligation_fact != expected_obligation_fact {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    Ok(())
}
