//! Range-against-literal integer-comparison reconstruction.

mod semantics;

pub(crate) use semantics::ValidatedIntegerRangeComparisonKind;
#[cfg(test)]
pub(crate) use semantics::{
    classify as independently_validated_integer_range_comparison_kind,
    evaluate as independently_evaluate_integer_range_comparison,
};

use omega_optimization_core::{
    AnalysisKind, OptimizationSafetyClass, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
use omega_optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRewriteCandidate,
};

use crate::{OptimizationUnitValidationError, current_value_ranges};

use super::super::BooleanEvaluation;
use super::IntegerComparisonShape;
use crate::candidates::sparse_conditional_constant_propagation::integer_evaluation::direct_literal_integer_fact;
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
    range_fact: ValueRangeFactIdentity,
    constant_fact: ScalarConstantFactIdentity,
) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ValueRanges)
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let kind = classify(candidate.rule(), &node.operation)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let (range_operand, constant_operand) = match kind {
        ValidatedIntegerRangeComparisonKind::RangeEqualConstant
        | ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
        | ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => {
            (shape.left, shape.right)
        }
        ValidatedIntegerRangeComparisonKind::ConstantEqualRange
        | ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
        | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => {
            (shape.right, shape.left)
        }
    };
    let constant_value =
        direct_literal_integer_fact(function, candidate.input(), constant_operand, constant_fact)
            .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let range = current_value_ranges::independently_reconstruct_value_range_fact_at(
        input,
        range_fact,
        function.machine,
        range_operand,
        location.block,
        location.node,
    )
    .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
    if validator_integer_value_type(function, constant_operand) != Some(range.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let constant = evaluate_semantics(
        kind,
        range.scalar_type,
        range.minimum,
        range.maximum,
        constant_value,
    )
    .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    Ok((
        shape.psi_operation,
        shape.result,
        constant,
        OptimizationSafetyClass::ProofCertified,
    ))
}
