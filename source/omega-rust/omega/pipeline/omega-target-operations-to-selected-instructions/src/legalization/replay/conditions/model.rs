//! Replayed scalar-condition custody returned by one named replay leaf.

use super::super::shared::*;
use crate::legalization::catalog::ScalarConditionShape;

pub(in crate::legalization) struct ReplayedCondition<'a> {
    pub source: ValueId,
    pub shape: ScalarConditionShape,
    pub result_type: IntegerType,
    pub when_true: &'a TargetConditionalIntegerArm,
    pub when_false: &'a TargetConditionalIntegerArm,
    pub conditional_node_index: usize,
    pub provenance_operations: Vec<OperationId>,
}

pub(super) type ReplayLeaf = for<'a> fn(
    usize,
    omega_target::Architecture,
    &'a omega_target_operations::TargetFunction,
    &omega_abstract_operations::AbstractFunction,
    &omega_optimization_unit::PsiOptimizationFunction,
    ValueId,
    &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError>;
