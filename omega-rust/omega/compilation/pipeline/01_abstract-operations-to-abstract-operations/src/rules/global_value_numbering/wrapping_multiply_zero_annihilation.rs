//! The exact wrapping multiply-zero rule, and the closed semantic partition
//! (`classify`) it proposes over.

use abstract_operations::{AbstractOperation as O, IntegerOverflowPolicy};
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};

use crate::rules::global_value_numbering::total_scalar_identities::{
    TotalScalarIdentityShape, exact_total_scalar_identity, multiply_zero_annihilation_shapes,
    propose_total_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WrappingMultiplyZeroAnnihilationRule;

impl WrappingMultiplyZeroAnnihilationRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1",
        )
    }
}

psi_optimization_rule_delegate!(
    WrappingMultiplyZeroAnnihilationRule,
    propose = propose_total_scalar_identities,
    contract = Self::contract(),
    classify
);

/// Return left-zero then right-zero so ties remain deterministic.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    multiply_zero_annihilation_shapes(
        operation,
        IntegerOverflowPolicy::Wrapping,
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
    )
}
