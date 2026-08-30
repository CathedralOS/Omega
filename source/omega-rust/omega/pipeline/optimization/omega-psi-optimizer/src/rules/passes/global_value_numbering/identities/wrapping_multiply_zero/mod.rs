//! Optimizer module role: executable entrance. Exact wrapping multiply-zero rule and proposal join.

mod laws;

use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{contract::exact_total_scalar_identity, proposal::propose_total_scalar_identities};

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingMultiplyZeroAnnihilationRule;

impl WrappingMultiplyZeroAnnihilationRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1",
        )
    }
}

impl PsiOptimizationRule for WrappingMultiplyZeroAnnihilationRule {
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
