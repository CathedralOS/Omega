//! General proof-certified integer identities.

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
    ProofCertifiedScalarIdentityShape, integer_one, integer_zero,
    propose_proof_certified_scalar_identities,
};

fn proof_certified_scalar_identity_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    match operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *right,
                    identity_operand: *left,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
                },
                integer_zero(*scalar_type),
            ),
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *left,
                    identity_operand: *right,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
                },
                integer_zero(*scalar_type),
            ),
        ],
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
            },
            integer_zero(*scalar_type),
        )],
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *right,
                    identity_operand: *left,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
                },
                integer_one(*scalar_type),
            ),
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *left,
                    identity_operand: *right,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
                },
                integer_one(*scalar_type),
            ),
        ],
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count,
            value,
            count_type,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
            },
            integer_zero(*count_type),
        )],
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count,
            value,
            count_type,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
            },
            integer_zero(*count_type),
        )],
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerIdentityEliminationRule;

impl LiveProofCertifiedIntegerIdentityEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
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

impl PsiOptimizationRule for LiveProofCertifiedIntegerIdentityEliminationRule {
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
            proof_certified_scalar_identity_shapes,
        )
    }
}
