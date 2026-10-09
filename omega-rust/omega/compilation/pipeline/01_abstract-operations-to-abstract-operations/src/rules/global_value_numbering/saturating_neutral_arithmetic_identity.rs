//! The exact saturating neutral-arithmetic rule, and the closed semantic partition
//! (`classify`) it proposes over.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};

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
    let O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        arithmetic,
        left,
        right,
    } = operation
    else {
        return Vec::new();
    };
    if arithmetic.policy() != IntegerOverflowPolicy::Saturating {
        return Vec::new();
    }
    let zero = scalar_type.zero();
    let one = scalar_type.one();
    // (replacement, law operand, identity, expected law value)
    let laws = match arithmetic.operator() {
        IntegerArithmeticOperator::Add => vec![
            (*right, *left, Kind::SaturatingIntegerAddZeroLeft, zero),
            (*left, *right, Kind::SaturatingIntegerAddZeroRight, zero),
        ],
        IntegerArithmeticOperator::Subtract => {
            vec![(
                *left,
                *right,
                Kind::SaturatingIntegerSubtractZeroRight,
                zero,
            )]
        }
        IntegerArithmeticOperator::Multiply => vec![
            (*right, *left, Kind::SaturatingIntegerMultiplyOneLeft, one),
            (*left, *right, Kind::SaturatingIntegerMultiplyOneRight, one),
        ],
        IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => Vec::new(),
    };
    laws.into_iter()
        .map(
            |(replacement, law_operand, identity, expected_law_value)| TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement,
                law_operand,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity,
                expected_law_value,
            },
        )
        .collect()
}
