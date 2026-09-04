//! Optimizer module role: executable entrance. Exact SCCP range-comparison rule contract and proposal join.

use super::IntegerRangePairComparisonKind;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use omega_optimization_core::{AnalysisKind, OptimizationRuleContract};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

#[derive(Debug, Clone, Copy, Default)]
pub struct IntegerLessThanRangeRangeRule;

impl IntegerLessThanRangeRangeRule {
    pub fn contract() -> OptimizationRuleContract {
        super::contract(b"omega.psi-rule.integer-less-than-range-range.v1")
    }
}

impl PsiOptimizationRule for IntegerLessThanRangeRangeRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }
    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::ValueRanges(ranges)) = analyses.get(AnalysisKind::ValueRanges)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ValueRanges,
            ));
        };
        super::proposal::propose(
            unit,
            ranges,
            Self::contract(),
            IntegerRangePairComparisonKind::LessThan,
        )
    }
}
