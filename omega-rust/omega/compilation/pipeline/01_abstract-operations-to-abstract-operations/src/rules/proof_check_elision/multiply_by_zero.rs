//! Proof-certified exact multiplication by zero.

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

fn proof_certified_exact_integer_multiply_by_zero_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Multiply)
    else {
        return Vec::new();
    };
    if operands.arithmetic.policy() != IntegerOverflowPolicy::Exact {
        return Vec::new();
    }
    vec![
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: operands.psi_operation,
                result: operands.result,
                replacement: operands.left,
                identity_operand: operands.left,
                scalar_type: operands.scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
            },
            operands.scalar_type.zero(),
        ),
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: operands.psi_operation,
                result: operands.result,
                replacement: operands.right,
                identity_operand: operands.right,
                scalar_type: operands.scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
            },
            operands.scalar_type.zero(),
        ),
    ]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule;

impl LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
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
    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
    propose = propose_proof_certified_scalar_identities,
    contract = Self::contract(),
    proof_certified_exact_integer_multiply_by_zero_shapes
);
