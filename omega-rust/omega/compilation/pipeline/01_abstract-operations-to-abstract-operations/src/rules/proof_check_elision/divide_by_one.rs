//! Proof-certified division by one.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::IntegerValue;

use crate::rules::proof_check_elision::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, propose_proof_certified_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

fn proof_certified_integer_divide_by_one_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Divide)
    else {
        return Vec::new();
    };
    let (source_operation, result, scalar_type, left, right, identity) = (
        operands.psi_operation,
        operands.result,
        operands.scalar_type,
        operands.left,
        operands.right,
        match operands.arithmetic.policy() {
            IntegerOverflowPolicy::Exact => {
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight
            }
            IntegerOverflowPolicy::Wrapping => {
                ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight
            }
            IntegerOverflowPolicy::Saturating => {
                ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight
            }
        },
    );
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: right,
            scalar_type,
            identity,
        },
        scalar_type.one(),
    )]
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LiveProofCertifiedIntegerDivideByOneEliminationRule;

impl LiveProofCertifiedIntegerDivideByOneEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        super::proof_check_elision_rule_contract(
            b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
        )
    }
}

psi_optimization_rule_delegate!(
    LiveProofCertifiedIntegerDivideByOneEliminationRule,
    propose = propose_proof_certified_scalar_identities,
    contract = Self::contract(),
    proof_certified_integer_divide_by_one_shapes
);
