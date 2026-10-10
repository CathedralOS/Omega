//! The exact wrapping neutral-arithmetic rule, and the closed semantic partition
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
pub(crate) struct WrappingNeutralArithmeticIdentityRule;

impl WrappingNeutralArithmeticIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1",
        )
    }
}

psi_optimization_rule_delegate!(
    WrappingNeutralArithmeticIdentityRule,
    propose = propose_total_scalar_identities,
    contract = Self::contract(),
    classify
);

/// Return the exact laws in canonical left-literal/right-literal tie order.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    use TotalScalarIdentityKind as Kind;
    neutral_arithmetic_shapes(
        operation,
        IntegerOverflowPolicy::Wrapping,
        NeutralArithmeticKinds {
            add_zero_left: Kind::WrappingIntegerAddZeroLeft,
            add_zero_right: Kind::WrappingIntegerAddZeroRight,
            subtract_zero_right: Kind::WrappingIntegerSubtractZeroRight,
            multiply_one_left: Kind::WrappingIntegerMultiplyOneLeft,
            multiply_one_right: Kind::WrappingIntegerMultiplyOneRight,
        },
    )
}
