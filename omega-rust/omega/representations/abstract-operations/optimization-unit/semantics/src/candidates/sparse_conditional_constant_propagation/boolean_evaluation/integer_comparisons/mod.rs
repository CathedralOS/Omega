//! Optimizer module role: stage group. Integer-comparison validation by exact evidence family.
mod constant;
mod range_against_constant;
mod range_against_range;

#[cfg(test)]
pub(crate) use range_against_constant::{
    ValidatedIntegerRangeComparisonKind, independently_evaluate_integer_range_comparison,
    independently_validated_integer_range_comparison_kind,
};
#[cfg(test)]
pub(crate) use range_against_range::{
    ValidatedIntegerRangePairComparisonKind, independently_evaluate_integer_range_pair_comparison,
    independently_validated_integer_range_pair_comparison_kind,
};

use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    IntegerEvaluationWitness, NodeLocation, OptimizationNode, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::{OperationId, ValueId};

use crate::OptimizationUnitValidationError;

use super::BooleanEvaluation;

#[derive(Clone, Copy)]
pub(super) struct IntegerComparisonShape {
    pub psi_operation: OperationId,
    pub result: ValueId,
    pub left: ValueId,
    pub right: ValueId,
}

pub(super) fn evaluate(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    candidate: &PsiRewriteCandidate,
    location: NodeLocation,
) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
    let shape = match node.operation {
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => IntegerComparisonShape {
            psi_operation,
            result,
            left,
            right,
        },
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    if let Some((left_range_fact, right_range_fact)) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::range_against_range)
    {
        return range_against_range::evaluate(
            input,
            function,
            node,
            candidate,
            location,
            shape,
            left_range_fact,
            right_range_fact,
        );
    }
    if let Some((range_fact, constant_fact)) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::range_against_constant)
    {
        return range_against_constant::evaluate(
            input,
            function,
            node,
            candidate,
            location,
            shape,
            range_fact,
            constant_fact,
        );
    }
    constant::evaluate(function, node, candidate, shape)
}
