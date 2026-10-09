//! The exact wrapping neutral-arithmetic rule, and the closed semantic partition
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
pub struct WrappingNeutralArithmeticIdentityRule;

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
    let Some(operands) = operation.integer_arithmetic() else {
        return Vec::new();
    };
    if operands.arithmetic.policy() != IntegerOverflowPolicy::Wrapping {
        return Vec::new();
    }
    let zero = operands.scalar_type.zero();
    let one = operands.scalar_type.one();
    // (replacement, law operand, identity, expected law value)
    let laws = match operands.arithmetic.operator() {
        IntegerArithmeticOperator::Add => vec![
            (
                operands.right,
                operands.left,
                Kind::WrappingIntegerAddZeroLeft,
                zero,
            ),
            (
                operands.left,
                operands.right,
                Kind::WrappingIntegerAddZeroRight,
                zero,
            ),
        ],
        IntegerArithmeticOperator::Subtract => {
            vec![(
                operands.left,
                operands.right,
                Kind::WrappingIntegerSubtractZeroRight,
                zero,
            )]
        }
        IntegerArithmeticOperator::Multiply => vec![
            (
                operands.right,
                operands.left,
                Kind::WrappingIntegerMultiplyOneLeft,
                one,
            ),
            (
                operands.left,
                operands.right,
                Kind::WrappingIntegerMultiplyOneRight,
                one,
            ),
        ],
        IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => Vec::new(),
    };
    laws.into_iter()
        .map(
            |(replacement, law_operand, identity, expected_law_value)| TotalScalarIdentityShape {
                source_operation: operands.psi_operation,
                result: operands.result,
                replacement,
                law_operand,
                scalar_type: operands.scalar_type,
                law_operand_type: operands.scalar_type,
                identity,
                expected_law_value,
            },
        )
        .collect()
}
