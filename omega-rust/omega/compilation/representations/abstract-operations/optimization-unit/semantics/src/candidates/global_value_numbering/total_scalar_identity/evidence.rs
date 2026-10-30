//! Independent law-literal fact reconstruction.
use crate::OptimizationUnitValidationError;
use crate::candidates::proof_check_elision::materialized_integer_constant;
use crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition;
use optimization_unit::{
    OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate,
    ScalarConstantValue, literal_scalar_constant_fact_identity,
};
use semantic_vocabulary::ScalarType;

use super::classification::IndependentTotalScalarIdentity;

pub(super) fn independently_validate_law_literal(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    shape: IndependentTotalScalarIdentity,
    candidate: &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError> {
    let definition = scalar_value_definition(function, shape.law_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if definition.scalar_type != ScalarType::Integer(shape.law_operand_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let literal = materialized_integer_constant(function, definition)?;
    if literal.result != shape.law_operand
        || literal.scalar_type != ScalarType::Integer(shape.law_operand_type)
        || literal.value != shape.law_constant
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant {
                value: fact_value,
                constant,
                support: fact_support,
            } if *fact_value == shape.law_operand
                && *constant == shape.law_constant
                && *fact_support == literal.operation)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        definition,
        ScalarConstantValue::Integer(shape.law_constant),
        literal.operation,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if candidate.total_scalar_identity_witness() != Some(expected)
        || candidate.accepted_obligation_witness().is_some()
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    Ok(())
}
