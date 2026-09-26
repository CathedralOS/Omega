//! Exact proof-certified unused scalar elimination: the rule and its closed
//! proof-bearing operation admission.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::ScalarType;

use crate::rules::PROOF_CHECK_ELISION_PASS_NAME;
use crate::rules::support::{DeadScalarShape, propose_proof_certified_dead_scalar_nodes};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct ProofCertifiedDeadScalarEliminationRule;

impl ProofCertifiedDeadScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for ProofCertifiedDeadScalarEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_dead_scalar_nodes(unit, analyses, Self::contract(), classify)
    }
}

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    let (source_operation, result, scalar_type) = match operation {
        O::IntegerExactCast {
            psi_operation,
            result,
            target_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*target_type)),
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            ..
        }
        | O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*value_type)),
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*scalar_type)),
        _ => return None,
    };
    Some(DeadScalarShape {
        source_operation,
        result,
        scalar_type,
    })
}
