//! Optimizer module role: executable entrance. Exact unused unconditionally-total scalar elimination rule.

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::rules::passes::DEAD_PURE_SCALAR_PASS_NAME;
use crate::rules::passes::support::propose_unproved_dead_scalar_nodes;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

mod operation_admission;

use operation_admission::classify;

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadUnconditionallyTotalScalarEliminationRule;

impl DeadUnconditionallyTotalScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(DEAD_PURE_SCALAR_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
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
        propose_unproved_dead_scalar_nodes(unit, analyses, Self::contract(), classify)
    }
}
