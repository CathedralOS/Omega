//! Literal integer-comparison reconstruction.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationSafetyClass;
use optimization_unit::{
    IntegerEvaluationWitness, OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate,
};

use crate::OptimizationUnitValidationError;

use super::super::{BooleanEvaluation, rule_identity};
use super::IntegerComparisonShape;
use crate::candidates::sparse_conditional_constant_propagation::integer_evaluation::literal_integer_fact;
use crate::candidates::sparse_conditional_constant_propagation::snapshot_reconstruction::validator_integer_value_type;

pub(super) fn evaluate(
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    candidate: &PsiRewriteCandidate,
    shape: IntegerComparisonShape,
) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
    rule_identity::validate(&node.operation, candidate.rule())?;
    let (left_fact, right_fact) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::binary_operands)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let left_value = literal_integer_fact(function, candidate.input(), shape.left, left_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let right_value = literal_integer_fact(function, candidate.input(), shape.right, right_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let left_type = validator_integer_value_type(function, shape.left)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if validator_integer_value_type(function, shape.right) != Some(left_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let ordering = left_type
        .compare(left_value, right_value)
        .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    let constant = match node.operation {
        O::IntegerEqual { .. } => ordering.is_eq(),
        O::IntegerLessThan { .. } => ordering.is_lt(),
        O::IntegerLessOrEqual { .. } => !ordering.is_gt(),
        _ => unreachable!(),
    };
    Ok((
        shape.psi_operation,
        shape.result,
        constant,
        OptimizationSafetyClass::ExactOperationSemantics,
    ))
}
