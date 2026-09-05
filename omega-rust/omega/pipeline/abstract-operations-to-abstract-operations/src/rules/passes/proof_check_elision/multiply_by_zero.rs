//! Proof-certified exact multiplication by zero.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::IntegerValue;

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::PROOF_CHECK_ELISION_PASS_NAME;
use super::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, integer_zero, propose_proof_certified_scalar_identities,
};

fn proof_certified_exact_integer_multiply_by_zero_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let O::ExactIntegerMultiply {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = operation
    else {
        return Vec::new();
    };
    vec![
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *left,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
            },
            integer_zero(*scalar_type),
        ),
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
            },
            integer_zero(*scalar_type),
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

impl PsiOptimizationRule for LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_exact_integer_multiply_by_zero_shapes,
        )
    }
}
