//! The exact saturating neutral-arithmetic rule, and the closed semantic partition
//! (`classify`) it proposes over.

use abstract_operations::{AbstractOperation as O, IntegerOverflowPolicy};
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};

use crate::rules::global_value_numbering::total_scalar_identities::{
    NeutralArithmeticKinds, TotalScalarIdentityShape, exact_total_scalar_identity,
    neutral_arithmetic_shapes, propose_total_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SaturatingNeutralArithmeticIdentityRule;

impl SaturatingNeutralArithmeticIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1",
        )
    }
}

psi_optimization_rule_delegate!(
    SaturatingNeutralArithmeticIdentityRule,
    propose = propose_total_scalar_identities,
    contract = Self::contract(),
    classify
);

/// Return the five saturating neutral laws in canonical tie order.
///
/// For commutative operations the left-identity row precedes the right row.
/// Signed one-bit integers have no positive-one literal, so the shared literal
/// fact lookup naturally declines both multiplication rows for that type.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    use TotalScalarIdentityKind as Kind;
    neutral_arithmetic_shapes(
        operation,
        IntegerOverflowPolicy::Saturating,
        NeutralArithmeticKinds {
            add_zero_left: Kind::SaturatingIntegerAddZeroLeft,
            add_zero_right: Kind::SaturatingIntegerAddZeroRight,
            subtract_zero_right: Kind::SaturatingIntegerSubtractZeroRight,
            multiply_one_left: Kind::SaturatingIntegerMultiplyOneLeft,
            multiply_one_right: Kind::SaturatingIntegerMultiplyOneRight,
        },
    )
}
