//! Independent neutral-literal fact reconstruction.

use super::classification::IndependentTotalScalarIdentity;
use super::*;

pub(super) fn independently_validate_neutral_literal(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    shape: IndependentTotalScalarIdentity,
    candidate: &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError> {
    let definition = scalar_value_definition(function, shape.identity_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if definition.scalar_type != ScalarType::Integer(shape.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let ValueDefinitionSite::Node { block, node } = definition.site else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let literal = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .and_then(|block| {
            usize::try_from(node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let O::IntegerConstant {
        psi_operation: support,
        result,
        scalar_type,
        value,
    } = literal.operation
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    if result != shape.identity_operand
        || scalar_type != ScalarType::Integer(shape.scalar_type)
        || value != shape.identity_constant
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant {
                value: fact_value,
                constant,
                support: fact_support,
            } if *fact_value == shape.identity_operand
                && *constant == shape.identity_constant
                && *fact_support == support)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        definition,
        ScalarConstantValue::Integer(shape.identity_constant),
        support,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if candidate.total_scalar_identity_witness() != Some(expected)
        || candidate.accepted_obligation_witness().is_some()
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    Ok(())
}
