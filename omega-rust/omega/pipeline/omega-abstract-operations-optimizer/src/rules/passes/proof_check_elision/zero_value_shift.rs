//! Proof-certified exact shifts of a zero value.

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

fn proof_certified_exact_integer_zero_value_shift_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, value, identity) = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            value,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *value,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            value,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *value,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: value,
            identity_operand: value,
            scalar_type,
            identity,
        },
        integer_zero(scalar_type),
    )]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule;

impl LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
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

impl PsiOptimizationRule for LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule {
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
            proof_certified_exact_integer_zero_value_shift_shapes,
        )
    }
}
