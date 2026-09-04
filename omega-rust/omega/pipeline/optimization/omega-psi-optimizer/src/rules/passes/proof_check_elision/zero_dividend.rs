//! Proof-certified zero dividends.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use psi_core::IntegerValue;

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::PROOF_CHECK_ELISION_PASS_NAME;
use super::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, integer_zero, propose_proof_certified_scalar_identities,
};

fn proof_certified_integer_zero_dividend_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, left, identity) = match operation {
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: left,
            scalar_type,
            identity,
        },
        integer_zero(scalar_type),
    )]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerZeroDividendEliminationRule;

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

impl PsiOptimizationRule for LiveProofCertifiedIntegerZeroDividendEliminationRule {
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
            proof_certified_integer_zero_dividend_shapes,
        )
    }
}
