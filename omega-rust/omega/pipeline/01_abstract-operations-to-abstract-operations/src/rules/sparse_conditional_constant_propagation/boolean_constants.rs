//! The five Boolean-result constant folds -- Boolean not and equal, integer
//! equal, less-than and less-or-equal -- one row each over the shared
//! traversal in `propose` and typed `evaluate`; all five emit the same
//! Boolean rewrite shape from scalar-constant evidence.

use abstract_operations::AbstractOperation as O;
use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, NodeLocation, OptimizationNode,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRealizationSite, PsiRewriteCandidate,
};
use semantic_vocabulary::{OperationId, ValueId};

use crate::rules::sparse_conditional_constant_propagation::{
    constant_evaluation_contract, integer_constant, integer_value_type,
};
use crate::rules::support::boolean_constant;
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    ScalarConstantAnalysis,
};

fn contract(rule_name: &[u8], safety: OptimizationSafetyClass) -> OptimizationRuleContract {
    constant_evaluation_contract(rule_name, safety)
}

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

pub(super) struct BooleanEvaluation {
    pub source_operation: OperationId,
    pub result: ValueId,
    pub constant: bool,
    pub witness: IntegerEvaluationWitness,
}

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: BooleanEvaluationKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(BooleanEvaluation {
                    source_operation,
                    result,
                    constant,
                    witness,
                }) = evaluate(function, node, constants, kind)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_boolean_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        witness,
                        -1,
                        BooleanConstantRewrite {
                            location,
                            source_operation,
                            result,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn evaluate(
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
