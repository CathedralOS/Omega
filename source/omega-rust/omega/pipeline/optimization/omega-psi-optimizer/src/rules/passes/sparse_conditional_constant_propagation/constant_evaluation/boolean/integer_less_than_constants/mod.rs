//! Optimizer module role: executable entrance. Exact integer-less-than constant-fold contract and proposal join.

use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::model::BooleanEvaluationKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct IntegerLessThanConstantsRule;

impl IntegerLessThanConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(b"omega.psi-rule.integer-less-than-constants.v1")
    }
}

impl PsiOptimizationRule for IntegerLessThanConstantsRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::proposal::propose(
            unit,
            analyses,
            Self::contract(),
            BooleanEvaluationKind::IntegerLessThan,
        )
    }
}
