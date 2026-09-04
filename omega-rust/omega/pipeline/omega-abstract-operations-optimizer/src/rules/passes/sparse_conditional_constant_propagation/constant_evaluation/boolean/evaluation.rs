//! Typed operation matching and scalar-fact evaluation for Boolean-result folds.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::{
    IntegerEvaluationWitness, OptimizationNode, PsiOptimizationFunction,
};

use crate::ScalarConstantAnalysis;
use crate::rules::passes::support::boolean_constant;

use super::super::integer::{integer_constant, integer_value_type};
use super::model::{BooleanEvaluation, BooleanEvaluationKind};

pub(super) fn evaluate(
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    constants: &ScalarConstantAnalysis,
    kind: BooleanEvaluationKind,
) -> Option<BooleanEvaluation> {
    let (source_operation, result, constant, witness) = match (&node.operation, kind) {
        (
            O::BooleanNot {
                psi_operation,
                result,
                operand,
            },
            BooleanEvaluationKind::Not,
        ) => {
            let (operand, operand_fact) = boolean_constant(constants, function.machine, *operand)?;
            (
                *psi_operation,
                *result,
                !operand,
                IntegerEvaluationWitness::Unary { operand_fact },
            )
        }
        (
            O::BooleanEqual {
                psi_operation,
                result,
                left,
                right,
            },
            BooleanEvaluationKind::Equal,
        ) => {
            let (left, left_fact) = boolean_constant(constants, function.machine, *left)?;
            let (right, right_fact) = boolean_constant(constants, function.machine, *right)?;
            (
                *psi_operation,
                *result,
                left == right,
                IntegerEvaluationWitness::Binary {
                    left_fact,
                    right_fact,
                },
            )
        }
        (
            O::IntegerEqual {
                psi_operation,
                result,
                left,
                right,
            },
            BooleanEvaluationKind::IntegerEqual,
        )
        | (
            O::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            },
            BooleanEvaluationKind::IntegerLessThan,
        )
        | (
            O::IntegerLessOrEqual {
                psi_operation,
                result,
                left,
                right,
            },
            BooleanEvaluationKind::IntegerLessOrEqual,
        ) => {
            let (left_value, left_fact) = integer_constant(constants, function.machine, *left)?;
            let (right_value, right_fact) = integer_constant(constants, function.machine, *right)?;
            let left_type = integer_value_type(function, *left)?;
            if integer_value_type(function, *right) != Some(left_type) {
                return None;
            }
            let ordering = left_type.compare(left_value, right_value)?;
            let constant = match kind {
                BooleanEvaluationKind::IntegerEqual => ordering.is_eq(),
                BooleanEvaluationKind::IntegerLessThan => ordering.is_lt(),
                BooleanEvaluationKind::IntegerLessOrEqual => !ordering.is_gt(),
                BooleanEvaluationKind::Not | BooleanEvaluationKind::Equal => unreachable!(),
            };
            (
                *psi_operation,
                *result,
                constant,
                IntegerEvaluationWitness::Binary {
                    left_fact,
                    right_fact,
                },
            )
        }
        _ => return None,
    };
    Some(BooleanEvaluation {
        source_operation,
        result,
        constant,
        witness,
    })
}
