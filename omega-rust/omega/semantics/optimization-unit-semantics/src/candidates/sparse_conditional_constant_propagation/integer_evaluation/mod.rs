//! Optimizer module role: executable entrance. Independent integer-evaluation reconstruction by operation shape.
//!
//! The parent SCCP validator enters here after candidate-level custody checks.
//! Unary operations are recognized first because they own their operand-fact
//! reconstruction. Every other operation must pass the exhaustive binary
//! shape classifier before this coordinator reconstructs both operand facts,
//! applies exact language semantics, and returns the safety classification.

mod binary_operation_shape;
mod binary_semantics;
mod literal_facts;
mod model;
mod rule_identity;
mod unary_operations;

use optimization_unit::{OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate};

use crate::OptimizationUnitValidationError;

pub(crate) use literal_facts::{
    direct_literal_integer_fact, literal_boolean_fact, literal_integer_fact, unary_integer_operand,
};
use model::IntegerEvaluation;

pub(crate) fn evaluate_integer_operation(
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<IntegerEvaluation, OptimizationUnitValidationError> {
    rule_identity::validate(&node.operation, candidate.rule())?;
    if let Some(evaluation) = unary_operations::evaluate(function, node, candidate) {
        return evaluation;
    }

    let operation = binary_operation_shape::recognize(&node.operation)?;
    let (left, right) = literal_facts::binary_integer_operands(
        function,
        candidate,
        operation.left,
        operation.right,
    )?;
    let (evaluated, safety_class) =
        binary_semantics::evaluate(operation.kind, operation.scalar_type, left, right);
    let evaluated =
        evaluated.ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;

    Ok((
        operation.source,
        operation.result,
        operation.scalar_type,
        evaluated,
        safety_class,
    ))
}
