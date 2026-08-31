//! Optimizer module role: executable entrance. Exact unused scalar-literal elimination rule.

use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{family::DeadScalarFamily, proposal::propose_dead_scalar_nodes};

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadScalarLiteralEliminationRule;

impl DeadScalarLiteralEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        DeadScalarFamily::Literal
            .contract(b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1")
    }
}

impl PsiOptimizationRule for DeadScalarLiteralEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(unit, analyses, Self::contract(), DeadScalarFamily::Literal)
    }
}
