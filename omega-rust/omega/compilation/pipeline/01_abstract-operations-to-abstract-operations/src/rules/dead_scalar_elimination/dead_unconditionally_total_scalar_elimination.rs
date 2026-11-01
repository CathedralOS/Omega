//! Exact unused unconditionally-total scalar elimination: the rule and its
//! closed total-operation admission.

use abstract_operations::{
    AbstractOperation as O, IntegerShiftPolicy, bitwise_not_and_pair_pattern,
    bitwise_or_xor_pair_pattern, boolean_not_or_equal_pair_pattern,
    integer_equal_less_pair_pattern,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::ScalarType;

use crate::rules::DEAD_PURE_SCALAR_PASS_NAME;
use crate::rules::node_elision::{DeadScalarShape, propose_unproved_dead_scalar_nodes};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    DeadUnconditionallyTotalScalarEliminationRule,
    crate::rules::standard_rule_contract(
        b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
        DEAD_PURE_SCALAR_PASS_NAME,
        1,
        AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries,]),
        OptimizationSafetyClass::ExactOperationSemantics
    ),
    propose = propose_unproved_dead_scalar_nodes,
    classify,
);

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    let (source_operation, result, scalar_type) = match operation {
        boolean_not_or_equal_pair_pattern!(psi_operation, result, ..)
        | integer_equal_less_pair_pattern!(psi_operation, result, ..)
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result, ScalarType::Boolean),
        bitwise_not_and_pair_pattern!(psi_operation, result, scalar_type, ..)
        | bitwise_or_xor_pair_pattern!(psi_operation, result, scalar_type, ..) => {
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
        }
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
