//! The exact saturating neutral-arithmetic rule, and the closed semantic partition
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
pub struct SaturatingNeutralArithmeticIdentityRule;

impl SaturatingNeutralArithmeticIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1",
        )
    }
}

impl PsiOptimizationRule for SaturatingNeutralArithmeticIdentityRule {
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

/// Return the five saturating neutral laws in canonical tie order.
///
/// For commutative operations the left-identity row precedes the right row.
/// Signed one-bit integers have no positive-one literal, so the shared literal
/// fact lookup naturally declines both multiplication rows for that type.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::SaturatingIntegerAdd {
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
                identity: TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft,
                expected_law_value: integer_value(*scalar_type, 0),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::SaturatingIntegerAddZeroRight,
                expected_law_value: integer_value(*scalar_type, 0),
            },
        ],
        O::SaturatingIntegerSubtract {
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
            identity: TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight,
            expected_law_value: integer_value(*scalar_type, 0),
        }],
        O::SaturatingIntegerMultiply {
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
                identity: TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
                expected_law_value: integer_value(*scalar_type, 1),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
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
