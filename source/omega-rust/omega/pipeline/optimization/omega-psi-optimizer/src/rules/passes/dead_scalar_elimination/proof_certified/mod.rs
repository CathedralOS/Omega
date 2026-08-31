//! Optimizer module role: executable entrance. Exact proof-certified unused scalar elimination rule.

use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{family::DeadScalarFamily, proposal::propose_dead_scalar_nodes};

#[derive(Debug, Clone, Copy, Default)]
pub struct ProofCertifiedDeadScalarEliminationRule;

impl ProofCertifiedDeadScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        DeadScalarFamily::ProofCertified
            .contract(b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1")
    }
}

impl PsiOptimizationRule for ProofCertifiedDeadScalarEliminationRule {
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
            DeadScalarFamily::ProofCertified,
        )
    }
}
