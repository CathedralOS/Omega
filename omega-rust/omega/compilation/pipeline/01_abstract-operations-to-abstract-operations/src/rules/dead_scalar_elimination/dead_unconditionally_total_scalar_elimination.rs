//! Exact unused unconditionally-total scalar elimination: the rule and its
//! closed total-operation admission.

use abstract_operations::{AbstractOperation as O, IntegerShiftPolicy};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::ScalarType;

use crate::rules::DEAD_PURE_SCALAR_PASS_NAME;
use crate::rules::node_elision::{DeadScalarShape, propose_unproved_dead_scalar_nodes};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    DeadUnconditionallyTotalScalarEliminationRule,
    crate::rules::standard_rule_contract(b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1", DEAD_PURE_SCALAR_PASS_NAME, 1,
                AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
                AnalysisInvalidationSet::new([
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
                OptimizationSafetyClass::ExactOperationSemantics),
);

psi_optimization_rule_delegate!(
    DeadUnconditionallyTotalScalarEliminationRule,
    propose = propose_unproved_dead_scalar_nodes,
    contract = Self::contract(),
    classify
);

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    let (source_operation, result, scalar_type) = match operation {
        abstract_operations::boolean_not_or_equal_pair_pattern!(psi_operation,
            result, ..)
        | abstract_operations::integer_equal_less_pair_pattern!(psi_operation,
            result, ..)
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result, ScalarType::Boolean),
        abstract_operations::bitwise_not_and_pair_pattern!(psi_operation,
            result,
            scalar_type, ..)
        | abstract_operations::bitwise_or_xor_pair_pattern!(psi_operation,
            result,
            scalar_type, ..) => (*psi_operation, *result, ScalarType::Integer(*scalar_type)),
        O::IntegerWiden {
            psi_operation,
            result,
            target_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*target_type)),
        // Integer arithmetic is total exactly when it carries no obligation:
        // wrapping and saturating add, subtract and multiply.
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            ..
        } if arithmetic.obligation().is_none() => {
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
        }
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            policy: IntegerShiftPolicy::Wrapping,
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
