//! Exact cast, widening, and bitwise-not reconstruction.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationSafetyClass;
use optimization_unit::{OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate};

use crate::OptimizationUnitValidationError;

use super::{model::IntegerEvaluation, unary_integer_operand};

pub(super) fn evaluate(
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Option<Result<IntegerEvaluation, OptimizationUnitValidationError>> {
    match node.operation {
        O::IntegerExactCast {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
            ..
        } => Some((|| {
            let operand = unary_integer_operand(function, candidate, operand)?;
            let evaluated = source_type
                .exact_cast_value_to(target_type, operand)
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
            Ok((
                psi_operation,
                result,
                target_type,
                evaluated,
                OptimizationSafetyClass::ProofCertified,
            ))
        })()),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => Some((|| {
            let operand = unary_integer_operand(function, candidate, operand)?;
            let evaluated = source_type
                .widen_value_to(target_type, operand)
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
            Ok((
                psi_operation,
                result,
                target_type,
                evaluated,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        })()),
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => Some((|| {
            let operand = unary_integer_operand(function, candidate, operand)?;
            let evaluated = scalar_type
                .bitwise_not(operand)
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
            Ok((
                psi_operation,
                result,
                scalar_type,
                evaluated,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        })()),
        _ => None,
    }
}
