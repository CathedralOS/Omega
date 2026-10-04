//! Optimizer module role: executable entrance. Independent Boolean-result evaluation by evidence shape.
//!
//! Literal Boolean operations and integer comparisons descend through separate
//! semantic leaves. Integer comparisons retain their constant, range/constant,
//! and range/range evidence joins without sharing producer logic.

mod boolean_literals;
mod integer_comparisons;
mod rule_identity;

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationSafetyClass;
use optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRewriteCandidate,
};
use semantic_vocabulary::{OperationId, ValueId};

use crate::OptimizationUnitValidationError;

pub(super) type BooleanEvaluation = (OperationId, ValueId, bool, OptimizationSafetyClass);

pub(super) fn evaluate(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    candidate: &PsiRewriteCandidate,
    location: NodeLocation,
) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
    match node.operation {
        O::BooleanNot { .. } | O::BooleanEqual { .. } => {
            boolean_literals::evaluate(function, node, candidate)
        }
        O::IntegerEqual { .. } | O::IntegerLessThan { .. } | O::IntegerLessOrEqual { .. } => {
            integer_comparisons::evaluate(input, function, node, candidate, location)
        }
        _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    }
}
