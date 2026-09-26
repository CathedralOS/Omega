//! The exact wrapping shift-zero-count rule, and the closed semantic partition
//! (`classify`) it proposes over.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

use crate::rules::global_value_numbering::total_scalar_identities::{
    TotalScalarIdentityShape, exact_total_scalar_identity, propose_total_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingShiftZeroCountIdentityRule;

impl WrappingShiftZeroCountIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1",
        )
    }
}

impl PsiOptimizationRule for WrappingShiftZeroCountIdentityRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(unit, analyses, Self::contract(), classify)
    }
}

pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            law_operand: *count,
            scalar_type: *value_type,
            law_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
            expected_law_value: integer_zero(*count_type),
        }],
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            law_operand: *count,
            scalar_type: *value_type,
            law_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
            expected_law_value: integer_zero(*count_type),
        }],
        _ => Vec::new(),
    }
}

const fn integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}
