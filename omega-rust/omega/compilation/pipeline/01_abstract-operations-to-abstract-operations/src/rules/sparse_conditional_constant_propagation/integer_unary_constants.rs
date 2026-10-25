//! The two unary integer constant folds, widening and bitwise-not: one row
//! each over the family's shared constant-fold traversal.

use abstract_operations::AbstractOperation as O;
use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate};

use crate::rules::sparse_conditional_constant_propagation::{
    constant_evaluation_contract as contract, integer_constant,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

constant_fold_rules! {
    IntegerBitwiseNotConstantsRule = (b"omega.psi-rule.integer-bitwise-not-constants.v1", ExactOperationSemantics, IntegerUnaryKind::BitwiseNot);
    IntegerWidenConstantsRule = (b"omega.psi-rule.integer-widen-constants.v1", ExactOperationSemantics, IntegerUnaryKind::Widen);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerUnaryKind {
    Widen,
    BitwiseNot,
}

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: IntegerUnaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    super::propose_integer_constant_folds(
        unit,
        analyses,
        contract,
        |_, machine, node, constants| {
            let (source_operation, result, source_type, target_type, operand) =
                match (&node.operation, kind) {
                    (
                        O::IntegerWiden {
                            psi_operation,
                            result,
                            source_type,
                            target_type,
                            operand,
                        },
                        IntegerUnaryKind::Widen,
                    ) => (
                        *psi_operation,
                        *result,
                        *source_type,
                        *target_type,
                        *operand,
                    ),
                    (
                        O::IntegerBitwiseNot {
                            psi_operation,
                            result,
                            scalar_type,
                            operand,
                        },
                        IntegerUnaryKind::BitwiseNot,
                    ) => (
                        *psi_operation,
                        *result,
                        *scalar_type,
                        *scalar_type,
                        *operand,
                    ),
                    _ => return Ok(None),
                };
            let Some((operand_value, operand_fact)) = integer_constant(constants, machine, operand)
            else {
                return Ok(None);
            };
            let constant = match kind {
                IntegerUnaryKind::Widen => source_type.widen_value_to(target_type, operand_value),
                IntegerUnaryKind::BitwiseNot => source_type.bitwise_not(operand_value),
            };
            let Some(constant) = constant else {
                return Ok(None);
            };
            Ok(Some(super::IntegerConstantFold {
                source_operation,
                result,
                scalar_type: target_type,
                constant,
                witness: IntegerEvaluationWitness::Unary { operand_fact },
            }))
        },
    )
}
