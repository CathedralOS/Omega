//! Exact unused scalar-literal elimination: the rule and its closed
//! scalar-literal operation admission.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet,
    OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::ScalarType;

use crate::rules::DEAD_PURE_SCALAR_PASS_NAME;
use crate::rules::node_elision::{DeadScalarShape, propose_unproved_dead_scalar_nodes};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    DeadScalarLiteralEliminationRule,
    crate::rules::standard_rule_contract(b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1", DEAD_PURE_SCALAR_PASS_NAME, 1,
                AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
                AnalysisInvalidationSet::new([
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
                OptimizationSafetyClass::ExactOperationSemantics),
    propose = propose_unproved_dead_scalar_nodes,
    classify,
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
