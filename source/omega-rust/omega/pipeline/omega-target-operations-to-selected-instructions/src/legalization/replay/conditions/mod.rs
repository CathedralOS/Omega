//! Optimizer module role: executable entrance. Independently classifies and replays one proposed scalar condition.

mod direct_parameter;
mod integer_equal_parameters;
mod integer_less_or_equal_parameters;
mod integer_less_than_parameters;
mod integer_not_equal_parameters;
mod integer_parameter_comparison;
mod integer_parameter_not_equal;

use super::shared::*;
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

type ReplayLeaf = for<'a> fn(
    usize,
    omega_target::Architecture,
    &'a omega_target_operations::TargetFunction,
    &omega_abstract_operations::AbstractFunction,
    &omega_optimization_unit::PsiOptimizationFunction,
    ValueId,
    &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError>;

pub(super) fn replay<'a>(
    function: usize,
    architecture: omega_target::Architecture,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError> {
    let leaf: ReplayLeaf = match (&target.operation, proposed) {
        (
            TargetOperation::ReturnIntegerConditionalControl { .. },
            LegalizedCondition::DirectParameter { .. },
        ) => direct_parameter::replay,
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerEqual { .. },
                ..
            },
            LegalizedCondition::IntegerEqualParametersV1 { .. },
        ) => integer_equal_parameters::replay,
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerLessThan { .. },
                ..
            },
            LegalizedCondition::IntegerLessThanParametersV1 { .. },
        ) => integer_less_than_parameters::replay,
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerLessOrEqual { .. },
                ..
            },
            LegalizedCondition::IntegerLessOrEqualParametersV1 { .. },
        ) => integer_less_or_equal_parameters::replay,
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::Not { operand, .. },
                ..
            },
            LegalizedCondition::IntegerNotEqualParametersV1 { .. },
        ) if matches!(
            operand.as_ref(),
            TargetBooleanExpression::IntegerEqual { .. }
        ) =>
        {
            integer_not_equal_parameters::replay
        }
        _ => return Err(Error::NonCanonicalLegalizedPlan),
    };
    leaf(
        function,
        architecture,
        target,
        abstracted,
        optimized,
        proposed_source,
        proposed,
    )
}
