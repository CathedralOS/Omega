//! Obligation-free same-block common-subexpression elimination.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::total_scalar_expression;
use crate::rules::global_value_numbering::same_block_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SameBlockTotalScalarCseRule;

impl SameBlockTotalScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        same_block_contract(
            b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        )
    }
}

impl PsiOptimizationRule for SameBlockTotalScalarCseRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_same_block_scalar_cse(
            unit,
            analyses,
            false,
            super::function_value_types,
            |value_types, operation| total_scalar_expression(operation, value_types),
            |same| {
                PsiRewriteCandidate::new_local_scalar_common_subexpression(
                    unit.identity,
                    Self::contract(),
                    same.affected_blocks,
                    same.provenance,
                    -1,
                    same.patch,
                )
                .map_err(RuleProposalError::InvalidCandidate)
            },
        )
    }
}
