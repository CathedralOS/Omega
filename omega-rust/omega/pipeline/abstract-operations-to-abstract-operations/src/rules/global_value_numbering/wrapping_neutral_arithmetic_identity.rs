//! The exact wrapping neutral-arithmetic rule, and the closed semantic partition
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
pub struct WrappingNeutralArithmeticIdentityRule;

impl WrappingNeutralArithmeticIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1",
        )
    }
}

impl PsiOptimizationRule for WrappingNeutralArithmeticIdentityRule {
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

/// Return the exact laws in canonical left-literal/right-literal tie order.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                law_operand: *left,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
                expected_law_value: integer_value(*scalar_type, 0),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
                expected_law_value: integer_value(*scalar_type, 0),
            },
        ],
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *left,
            law_operand: *right,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
            expected_law_value: integer_value(*scalar_type, 0),
        }],
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                law_operand: *left,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
                expected_law_value: integer_value(*scalar_type, 1),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
                expected_law_value: integer_value(*scalar_type, 1),
            },
        ],
        _ => Vec::new(),
    }
}

const fn integer_value(scalar_type: IntegerType, value: u128) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}
