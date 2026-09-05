//! Optimizer module role: executable entrance. Exact integer-widen constant-fold contract and proposal join.

use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::model::IntegerUnaryKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct IntegerWidenConstantsRule;

impl IntegerWidenConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(b"omega.psi-rule.integer-widen-constants.v1")
    }
}

impl PsiOptimizationRule for IntegerWidenConstantsRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::proposal::propose(unit, analyses, Self::contract(), IntegerUnaryKind::Widen)
    }
}
