//! Proof-certified zero dividends.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::IntegerValue;

use crate::rules::PROOF_CHECK_ELISION_PASS_NAME;
use crate::rules::proof_check_elision::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, propose_proof_certified_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

fn proof_certified_integer_zero_dividend_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_arithmetic() else {
        return Vec::new();
    };
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    use ProofCertifiedScalarIdentityKind as Kind;
    let identity = match (operands.arithmetic.policy(), operands.arithmetic.operator()) {
        (Policy::Exact, Operator::Divide) => Kind::ExactIntegerDivideZeroLeft,
        (Policy::Wrapping, Operator::Divide) => Kind::WrappingIntegerDivideZeroLeft,
        (Policy::Saturating, Operator::Divide) => Kind::SaturatingIntegerDivideZeroLeft,
        (Policy::Exact, Operator::Remainder) => Kind::ExactIntegerRemainderZeroLeft,
        (Policy::Wrapping, Operator::Remainder) => Kind::WrappingIntegerRemainderZeroLeft,
        (Policy::Saturating, Operator::Remainder) => Kind::SaturatingIntegerRemainderZeroLeft,
        (_, Operator::Add | Operator::Subtract | Operator::Multiply) => {
            return Vec::new();
        }
    };
    let (source_operation, result, scalar_type, left) = (
        operands.psi_operation,
        operands.result,
        operands.scalar_type,
        operands.left,
    );
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: left,
            scalar_type,
            identity,
        },
        scalar_type.zero(),
    )]
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LiveProofCertifiedIntegerZeroDividendEliminationRule;

impl LiveProofCertifiedIntegerZeroDividendEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

psi_optimization_rule_delegate!(
    LiveProofCertifiedIntegerZeroDividendEliminationRule,
    propose = propose_proof_certified_scalar_identities,
    contract = Self::contract(),
    proof_certified_integer_zero_dividend_shapes
);
