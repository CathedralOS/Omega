//! Optimizer module role: executable entrance.
//!
//! Owns the wrapping integer left-shift constant fold contract and proposal join.

use omega_optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::model::IntegerBinaryKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingIntegerShiftLeftConstantsRule;

impl WrappingIntegerShiftLeftConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(
            b"omega.psi-rule.wrapping-integer-shift-left-constants.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        )
    }
}

impl PsiOptimizationRule for WrappingIntegerShiftLeftConstantsRule {
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
            IntegerBinaryKind::WrappingShiftLeft,
        )
    }
}
