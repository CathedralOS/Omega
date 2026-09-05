//! Optimizer module role: executable entrance.
//!
//! Owns the proof-certified exact integer-remainder constant fold contract and proposal join.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::model::IntegerBinaryKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct ExactIntegerRemainderConstantsRule;

impl ExactIntegerRemainderConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(
            b"omega.psi-rule.exact-integer-remainder-constants.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for ExactIntegerRemainderConstantsRule {
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
            IntegerBinaryKind::ExactRemainder,
        )
    }
}
