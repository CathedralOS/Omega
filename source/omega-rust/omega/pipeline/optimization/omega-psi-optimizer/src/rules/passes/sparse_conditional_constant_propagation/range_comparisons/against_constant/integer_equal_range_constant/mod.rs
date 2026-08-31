//! Optimizer module role: executable entrance. Exact SCCP range-comparison rule contract and proposal join.

use omega_optimization_core::{AnalysisKind, OptimizationRuleContract};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::IntegerRangeComparisonKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct IntegerEqualRangeConstantRule;

impl IntegerEqualRangeConstantRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(b"omega.psi-rule.integer-equal-range-constant.v1")
    }
}

impl PsiOptimizationRule for IntegerEqualRangeConstantRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::ScalarConstants(constants)) =
            analyses.get(AnalysisKind::ScalarConstants)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants,
            ));
        };
        let Some(AnalysisProduct::ValueRanges(ranges)) = analyses.get(AnalysisKind::ValueRanges)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ValueRanges,
            ));
        };
        super::proposal::propose(
            unit,
            constants,
            ranges,
            Self::contract(),
            IntegerRangeComparisonKind::RangeEqualConstant,
        )
    }
}
