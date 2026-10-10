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

psi_rule_contract!(
    WrappingMultiplyZeroAnnihilationRule,
    exact_total_scalar_identity(
                b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1",
            ),
    propose = propose_total_scalar_identities,
    classify,
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
