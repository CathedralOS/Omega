//! Obligation-free cross-block elimination from dominating leaders.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::total_scalar_expression;
use crate::rules::global_value_numbering::dominating_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    DominatorTotalScalarGvnRule,
    dominating_contract(
        b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1",
        OptimizationSafetyClass::ExactOperationSemantics,
    ),
);

impl PsiOptimizationRule for DominatorTotalScalarGvnRule {
    psi_rule_contract!(@adapter);

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_dominating_scalar_gvn(
            unit,
            analyses,
            super::function_value_types,
            |value_types, _, _, operation| total_scalar_expression(operation, value_types),
            |value_types, _, _, operation| {
                total_scalar_expression(operation, value_types).map(|row| (row, ()))
            },
            |same| {
                PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
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
