//! Proof-certified exact signed right shifts of a negative-one value.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use psi_core::{IntegerCarrier, IntegerSign, IntegerValue};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::PROOF_CHECK_ELISION_PASS_NAME;
use super::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, propose_proof_certified_scalar_identities,
};

fn proof_certified_exact_signed_integer_negative_one_shift_right_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let O::ExactIntegerShiftRight {
        psi_operation,
        result,
        value_type,
        value,
        ..
    } = operation
    else {
        return Vec::new();
    };
    if value_type.carrier() != IntegerCarrier::Fixed || value_type.sign() != IntegerSign::Signed {
        return Vec::new();
    }
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            identity_operand: *value,
            scalar_type: *value_type,
            identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue,
        },
        IntegerValue::Signed(-1),
    )]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule;

impl LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1",
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

impl PsiOptimizationRule
    for LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule
{
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
            proof_certified_exact_signed_integer_negative_one_shift_right_shapes,
        )
    }
}
