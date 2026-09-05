//! Optimizer module role: executable entrance.
//!
//! Owns the proof-certified saturating integer-remainder constant fold contract and proposal join.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::model::IntegerBinaryKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct SaturatingIntegerRemainderConstantsRule;

impl SaturatingIntegerRemainderConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(
            b"omega.psi-rule.saturating-integer-remainder-constants.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for SaturatingIntegerRemainderConstantsRule {
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
            IntegerBinaryKind::SaturatingRemainder,
        )
    }
}
