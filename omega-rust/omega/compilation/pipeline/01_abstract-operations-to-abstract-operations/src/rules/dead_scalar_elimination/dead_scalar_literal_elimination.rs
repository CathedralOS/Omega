//! Exact unused scalar-literal elimination: the rule and its closed
//! scalar-literal operation admission.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::ScalarType;

use crate::rules::DEAD_PURE_SCALAR_PASS_NAME;
use crate::rules::node_elision::{DeadScalarShape, propose_unproved_dead_scalar_nodes};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeadScalarLiteralEliminationRule;

impl DeadScalarLiteralEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
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

psi_optimization_rule_delegate!(
    DeadScalarLiteralEliminationRule,
    propose = propose_unproved_dead_scalar_nodes,
    contract = Self::contract(),
    classify
);

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    match operation {
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            ..
        } => Some(DeadScalarShape {
            source_operation: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
        }),
        O::BooleanConstant {
            psi_operation,
            result,
            ..
        } => Some(DeadScalarShape {
            source_operation: *psi_operation,
            result: *result,
            scalar_type: ScalarType::Boolean,
        }),
        _ => None,
    }
}
