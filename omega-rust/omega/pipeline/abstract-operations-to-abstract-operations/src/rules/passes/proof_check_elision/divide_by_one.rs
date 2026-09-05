//! Proof-certified division by one.

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
    ProofCertifiedScalarIdentityShape, integer_one, propose_proof_certified_scalar_identities,
};

fn proof_certified_integer_divide_by_one_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, left, right, identity) = match operation {
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: right,
            scalar_type,
            identity,
        },
        integer_one(scalar_type),
    )]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerDivideByOneEliminationRule;

impl LiveProofCertifiedIntegerDivideByOneEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
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

impl PsiOptimizationRule for LiveProofCertifiedIntegerDivideByOneEliminationRule {
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
            proof_certified_integer_divide_by_one_shapes,
        )
    }
}
