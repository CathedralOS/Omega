//! Optimizer module role: executable entrance. Independently classifies and replays one proposed scalar condition.

mod direct_parameter;
mod integer_equal_parameters;
mod integer_less_or_equal_parameters;
mod integer_less_than_parameters;
mod integer_parameter_comparison;

use super::shared::*;
use crate::legalization::catalog::ScalarConditionShape;

pub(in crate::legalization) struct ReplayedCondition<'a> {
    pub source: ValueId,
    pub shape: ScalarConditionShape,
    pub result_type: IntegerType,
    pub when_true: &'a TargetConditionalIntegerArm,
    pub when_false: &'a TargetConditionalIntegerArm,
    pub conditional_node_index: usize,
    pub provenance_operation: Option<OperationId>,
}

pub(super) fn replay<'a>(
    function: usize,
    architecture: omega_target::Architecture,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError> {
    match (&target.operation, proposed) {
        (
            TargetOperation::ReturnIntegerConditionalControl { .. },
            LegalizedCondition::DirectParameter { .. },
        ) => direct_parameter::replay(
            function,
            architecture,
            target,
            abstracted,
            optimized,
            proposed_source,
            proposed,
        ),
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerEqual { .. },
                ..
            },
            LegalizedCondition::IntegerEqualParametersV1 { .. },
        ) => integer_equal_parameters::replay(
            function,
            architecture,
            target,
            abstracted,
            optimized,
            proposed_source,
            proposed,
        ),
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerLessThan { .. },
                ..
            },
            LegalizedCondition::IntegerLessThanParametersV1 { .. },
        ) => integer_less_than_parameters::replay(
            function,
            architecture,
            target,
            abstracted,
            optimized,
            proposed_source,
            proposed,
        ),
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerLessOrEqual { .. },
                ..
            },
            LegalizedCondition::IntegerLessOrEqualParametersV1 { .. },
        ) => integer_less_or_equal_parameters::replay(
            function,
            architecture,
            target,
            abstracted,
            optimized,
            proposed_source,
            proposed,
        ),
        _ => Err(Error::NonCanonicalLegalizedPlan),
    }
}
