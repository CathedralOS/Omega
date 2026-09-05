//! Optimizer module role: executable entrance. Exact unused scalar-literal elimination rule.

use crate::rules::passes::DEAD_PURE_SCALAR_PASS_NAME;
use crate::rules::passes::support::propose_unproved_dead_scalar_nodes;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod operation_admission;

use operation_admission::classify;

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadScalarLiteralEliminationRule;

impl DeadScalarLiteralEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
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

impl PsiOptimizationRule for DeadScalarLiteralEliminationRule {
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
