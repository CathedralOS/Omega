//! Optimizer module role: executable entrance. Exact Boolean-not constant-fold contract and proposal join.

use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::model::BooleanEvaluationKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct BooleanNotConstantsRule;

impl BooleanNotConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(b"omega.psi-rule.boolean-not-constants.v1")
    }
}

impl PsiOptimizationRule for BooleanNotConstantsRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::proposal::propose(unit, analyses, Self::contract(), BooleanEvaluationKind::Not)
    }
}
