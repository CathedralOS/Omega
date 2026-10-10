//! Proof-certified exact self subtraction.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::{IntegerType, OperationId, ValueId};

use crate::rules::proof_check_elision::identity_rewrite::propose_proof_certified_integer_constants;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    LiveProofCertifiedExactIntegerSelfSubtractEliminationRule,
    super::proof_check_elision_rule_contract(
                b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
                AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            ),
    propose = propose_proof_certified_integer_constants,
    classify,
    IntegerType::zero,
);

fn classify(operation: &O) -> Option<(OperationId, ValueId, IntegerType)> {
    let operands = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Subtract)?;
    if !operands.arithmetic.is(
        IntegerOverflowPolicy::Exact,
        IntegerArithmeticOperator::Subtract,
    ) || operands.left != operands.right
    {
        return None;
    }
    Some((
        operands.psi_operation,
        operands.result,
        operands.scalar_type,
    ))
}
