//! Proof-certified self remainder.

use abstract_operations::{AbstractOperation as O, IntegerArithmeticOperator};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::{IntegerType, OperationId, ValueId};

use crate::rules::PROOF_CHECK_ELISION_PASS_NAME;
use crate::rules::proof_check_elision::identity_rewrite::{
    integer_zero, propose_proof_certified_integer_constants,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerSelfRemainderEliminationRule;

impl LiveProofCertifiedIntegerSelfRemainderEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerSelfRemainderEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_integer_constants(
            unit,
            analyses,
            Self::contract,
            classify,
            integer_zero,
        )
    }
}

fn classify(operation: &O) -> Option<(&OperationId, &ValueId, &IntegerType)> {
    let O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        arithmetic,
        left,
        right,
    } = operation
    else {
        return None;
    };
    if arithmetic.operator() != IntegerArithmeticOperator::Remainder || left != right {
        return None;
    }
    Some((psi_operation, result, scalar_type))
}
