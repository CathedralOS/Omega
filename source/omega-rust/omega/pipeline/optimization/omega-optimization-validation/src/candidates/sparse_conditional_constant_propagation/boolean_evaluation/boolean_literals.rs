//! Literal Boolean operation reconstruction.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::OptimizationSafetyClass;
use omega_optimization_unit::{
    IntegerEvaluationWitness, OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate,
};

use crate::OptimizationUnitValidationError;

use super::super::integer_evaluation::literal_boolean_fact;
use super::{BooleanEvaluation, rule_identity};

pub(super) fn evaluate(
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
    rule_identity::validate(&node.operation, candidate.rule())?;
    match node.operation {
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => {
            let operand_fact = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::unary_operand)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let operand = literal_boolean_fact(function, candidate.input(), operand, operand_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((
                psi_operation,
                result,
                !operand,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let (left_fact, right_fact) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::binary_operands)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let left = literal_boolean_fact(function, candidate.input(), left, left_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right = literal_boolean_fact(function, candidate.input(), right, right_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((
                psi_operation,
                result,
                left == right,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    }
}
