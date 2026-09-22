//! Exact unused unconditionally-total scalar elimination: the rule and its
//! closed total-operation admission.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::ScalarType;

use crate::rules::DEAD_PURE_SCALAR_PASS_NAME;
use crate::rules::support::{DeadScalarShape, propose_unproved_dead_scalar_nodes};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadUnconditionallyTotalScalarEliminationRule;

impl DeadUnconditionallyTotalScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(DEAD_PURE_SCALAR_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DeadUnconditionallyTotalScalarEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_unproved_dead_scalar_nodes(unit, analyses, Self::contract(), classify)
    }
}

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    let (source_operation, result, scalar_type) = match operation {
        O::BooleanNot {
            psi_operation,
            result,
            ..
        }
        | O::BooleanEqual {
            psi_operation,
            result,
            ..
        }
        | O::IntegerEqual {
            psi_operation,
            result,
            ..
        }
        | O::IntegerLessThan {
            psi_operation,
            result,
            ..
        }
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result, ScalarType::Boolean),
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*scalar_type)),
        O::IntegerWiden {
            psi_operation,
            result,
            target_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*target_type)),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            ..
        }
        | O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*value_type)),
        _ => return None,
    };
    Some(DeadScalarShape {
        source_operation,
        result,
        scalar_type,
    })
}
