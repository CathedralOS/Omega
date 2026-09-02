//! Optimizer module role: executable entrance. Independently classifies and replays one proposed scalar condition.

mod direct_parameter;
mod i64_less_than_parameters;
mod integer_equal_parameters;
mod integer_less_or_equal_parameters;
mod integer_less_than_parameters;
mod integer_not_equal_parameters;
mod integer_parameter_comparison;
mod integer_parameter_not_equal;
mod model;
mod u64_equal_zero_parameter;

use super::shared::*;
use crate::legalization::catalog::ScalarConditionShape;
use model::ReplayLeaf;
pub(in crate::legalization) use model::ReplayedCondition;

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
            LegalizedCondition::U64EqualZeroParameterV1 { .. },
        ) => u64_equal_zero_parameter::replay,
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerEqual { .. },
                ..
            },
            LegalizedCondition::IntegerEqualParametersV1 { .. },
        ) => integer_equal_parameters::replay,
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition: TargetBooleanExpression::IntegerLessThan { scalar_type, .. },
                ..
            },
            LegalizedCondition::I64LessThanParametersV1 { .. },
        ) if *scalar_type == IntegerType::new(IntegerSign::Signed, 64).expect("i64") => {
            i64_less_than_parameters::replay
        }
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
