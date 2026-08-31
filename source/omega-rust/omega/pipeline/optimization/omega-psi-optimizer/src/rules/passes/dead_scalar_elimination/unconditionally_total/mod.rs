//! Optimizer module role: executable entrance. Exact unused unconditionally-total scalar elimination rule.

use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{family::DeadScalarFamily, proposal::propose_dead_scalar_nodes};

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadUnconditionallyTotalScalarEliminationRule;

impl DeadUnconditionallyTotalScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        DeadScalarFamily::UnconditionallyTotal
            .contract(b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1")
    }
}

impl PsiOptimizationRule for DeadUnconditionallyTotalScalarEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(
            unit,
            analyses,
            Self::contract(),
            DeadScalarFamily::UnconditionallyTotal,
        )
    }
}
