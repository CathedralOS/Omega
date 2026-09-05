//! Optimizer module role: executable entrance. Exact SCCP range-comparison rule contract and proposal join.

use super::IntegerRangeComparisonKind;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use optimization_core::{AnalysisKind, OptimizationRuleContract};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

#[derive(Debug, Clone, Copy, Default)]
pub struct IntegerLessOrEqualConstantRangeRule;

impl IntegerLessOrEqualConstantRangeRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(b"omega.psi-rule.integer-less-or-equal-constant-range.v1")
    }
}

impl PsiOptimizationRule for IntegerLessOrEqualConstantRangeRule {
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
            IntegerRangeComparisonKind::ConstantLessOrEqualRange,
        )
    }
}
