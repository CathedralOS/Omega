//! Optimizer module role: executable entrance. Exact integer bitwise-not constant-fold contract and proposal join.

use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::model::IntegerUnaryKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct IntegerBitwiseNotConstantsRule;

impl IntegerBitwiseNotConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(b"omega.psi-rule.integer-bitwise-not-constants.v1")
    }
}

impl PsiOptimizationRule for IntegerBitwiseNotConstantsRule {
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
            IntegerUnaryKind::BitwiseNot,
        )
    }
}
