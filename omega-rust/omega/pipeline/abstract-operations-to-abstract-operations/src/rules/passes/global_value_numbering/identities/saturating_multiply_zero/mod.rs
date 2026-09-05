//! Optimizer module role: executable entrance. Exact saturating multiply-zero rule and proposal join.

mod laws;

use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{contract::exact_total_scalar_identity, proposal::propose_total_scalar_identities};

#[derive(Debug, Clone, Copy, Default)]
pub struct SaturatingMultiplyZeroAnnihilationRule;

impl SaturatingMultiplyZeroAnnihilationRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1",
        )
    }
}

impl PsiOptimizationRule for SaturatingMultiplyZeroAnnihilationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(unit, analyses, Self::contract(), laws::classify)
    }
}
