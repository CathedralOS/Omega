//! Range-against-range integer-comparison reconstruction.

mod semantics;

#[cfg(test)]
pub(crate) use semantics::ValidatedIntegerRangePairComparisonKind;
#[cfg(test)]
pub(crate) use semantics::{
    classify as independently_validated_integer_range_pair_comparison_kind,
    evaluate as independently_evaluate_integer_range_pair_comparison,
};

use optimization_core::{AnalysisKind, OptimizationSafetyClass, ValueRangeFactIdentity};
use optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRewriteCandidate,
};

use crate::{OptimizationUnitValidationError, current_value_ranges};

use super::super::BooleanEvaluation;
use super::IntegerComparisonShape;
use crate::candidates::sparse_conditional_constant_propagation::snapshot_reconstruction::validator_integer_value_type;
use semantics::{classify, evaluate as evaluate_semantics};

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    candidate: &PsiRewriteCandidate,
    location: NodeLocation,
    shape: IntegerComparisonShape,
    left_range_fact: ValueRangeFactIdentity,
    right_range_fact: ValueRangeFactIdentity,
) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ValueRanges)
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let kind = classify(candidate.rule(), &node.operation)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let left_range = current_value_ranges::independently_reconstruct_value_range_fact_at(
        input,
        left_range_fact,
        function.machine,
        shape.left,
        location.block,
        location.node,
    )
    .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
    let right_range = current_value_ranges::independently_reconstruct_value_range_fact_at(
        input,
        right_range_fact,
        function.machine,
        shape.right,
        location.block,
        location.node,
    )
    .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
    if left_range.scalar_type != right_range.scalar_type
        || validator_integer_value_type(function, shape.left) != Some(left_range.scalar_type)
        || validator_integer_value_type(function, shape.right) != Some(right_range.scalar_type)
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let constant = evaluate_semantics(
        kind,
        left_range.scalar_type,
        shape.left == shape.right,
        left_range.minimum,
        left_range.maximum,
        right_range.minimum,
        right_range.maximum,
    )
    .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    Ok((
        shape.psi_operation,
        shape.result,
        constant,
        OptimizationSafetyClass::ProofCertified,
    ))
}
