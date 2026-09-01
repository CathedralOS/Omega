//! Optimizer module role: executable entrance. Independent Boolean-result evaluation by evidence shape.
//!
//! Literal Boolean operations and integer comparisons descend through separate
//! semantic leaves. Integer comparisons retain their constant, range/constant,
//! and range/range evidence joins without sharing producer logic.

mod boolean_literals;
mod integer_comparisons;
mod rule_identity;

#[cfg(test)]
pub(crate) use integer_comparisons::{
    ValidatedIntegerRangeComparisonKind, ValidatedIntegerRangePairComparisonKind,
    independently_evaluate_integer_range_comparison,
    independently_evaluate_integer_range_pair_comparison,
    independently_validated_integer_range_comparison_kind,
    independently_validated_integer_range_pair_comparison_kind,
};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::OptimizationSafetyClass;
use omega_optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRewriteCandidate,
};
use psi_core::{OperationId, ValueId};

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
