//! Proof-certified self division.

use abstract_operations::{AbstractOperation as O, IntegerArithmeticOperator};
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType, OperationId, ValueId};

use crate::rules::proof_check_elision::identity_rewrite::propose_proof_certified_integer_constants;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    LiveProofCertifiedIntegerSelfDivideEliminationRule,
    super::proof_check_elision_rule_contract(
        b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
        AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
    ),
    propose = propose_proof_certified_integer_constants,
    classify,
    IntegerType::one,
);

fn classify(operation: &O) -> Option<(OperationId, ValueId, IntegerType)> {
    let operands = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Divide)?;
    if operands.left != operands.right
        || operands.scalar_type.carrier() != IntegerCarrier::Fixed
        || (operands.scalar_type.sign() == IntegerSign::Signed && operands.scalar_type.bits() == 1)
    {
        return None;
    }
    Some((
        operands.psi_operation,
        operands.result,
        operands.scalar_type,
    ))
}
