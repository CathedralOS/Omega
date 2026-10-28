//! The five Boolean-result constant folds -- Boolean not and equal, integer
//! equal, less-than and less-or-equal -- one row each over the shared
//! traversal in `propose` and typed `evaluate`; all five emit the same
//! Boolean rewrite shape from scalar-constant evidence.

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    IntegerEvaluationWitness, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRewriteCandidate,
};

use crate::rules::sparse_conditional_constant_propagation::{
    constant_evaluation_contract as contract, integer_constant, integer_value_type,
};
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    ScalarConstantAnalysis,
};

constant_fold_rules! {
    BooleanEqualConstantsRule = (b"omega.psi-rule.boolean-equal-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::Equal);
    BooleanNotConstantsRule = (b"omega.psi-rule.boolean-not-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::Not);
    IntegerEqualConstantsRule = (b"omega.psi-rule.integer-equal-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::IntegerEqual);
    IntegerLessOrEqualConstantsRule = (b"omega.psi-rule.integer-less-or-equal-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::IntegerLessOrEqual);
    IntegerLessThanConstantsRule = (b"omega.psi-rule.integer-less-than-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::IntegerLessThan);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BooleanEvaluationKind {
    Not,
    Equal,
    IntegerEqual,
    IntegerLessThan,
    IntegerLessOrEqual,
}

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: BooleanEvaluationKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    super::propose_boolean_evaluations(unit, contract, |function, _, node, _| {
        evaluate(function, node, constants, kind)
    })
}

fn evaluate(
    function: &PsiOptimizationFunction,
    node: &OptimizationNode,
    constants: &ScalarConstantAnalysis,
    kind: BooleanEvaluationKind,
) -> Option<super::BooleanConstantEvaluation> {
    let (source_operation, result, constant, witness) = match (&node.operation, kind) {
        (
            O::BooleanNot {
                psi_operation,
                result,
                operand,
            },
            BooleanEvaluationKind::Not,
        ) => {
            let (operand, operand_fact) = constants.boolean_constant(function.machine, *operand)?;
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
            let (left, left_fact) = constants.boolean_constant(function.machine, *left)?;
            let (right, right_fact) = constants.boolean_constant(function.machine, *right)?;
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
    Some(super::BooleanConstantEvaluation {
        source_operation,
        result,
        constant,
        witness,
    })
}
