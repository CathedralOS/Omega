//! Optimizer module role: executable entrance. Classifies and reconstructs the exact scalar condition before leaf selection.

mod direct_parameter;
mod i64_less_or_equal_parameters;
mod i64_less_than_parameters;
mod integer_equal_parameters;
mod integer_less_or_equal_parameters;
mod integer_less_than_parameters;
mod integer_not_equal_parameters;
mod integer_parameter_comparison;
mod integer_parameter_not_equal;
mod model;
mod u64_equal_zero_parameter;
mod u64_not_equal_zero_parameter;

use super::shared::*;
use crate::legalization::catalog::ScalarConditionShape;
pub(in crate::legalization) use model::DerivedCondition;

pub(super) fn derive<'a>(
    function: usize,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    match &target.operation {
        TargetOperation::ReturnIntegerConditionalControl { .. } => {
            direct_parameter::derive(function, target, abstracted, optimized)
        }
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerEqual { left, right, .. },
            ..
        } if matches!(
            (left.as_ref(), right.as_ref()),
            (
                TargetIntegerExpression::Parameter { .. },
                TargetIntegerExpression::Immediate {
                    value: semantic_vocabulary::IntegerValue::Unsigned(0),
                    ..
                }
            )
        ) =>
        {
            u64_equal_zero_parameter::derive(function, target, abstracted, optimized)
        }
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerEqual { .. },
            ..
        } => integer_equal_parameters::derive(function, target, abstracted, optimized),
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerLessThan { scalar_type, .. },
            ..
        } if *scalar_type == IntegerType::new(IntegerSign::Signed, 64).expect("i64") => {
            i64_less_than_parameters::derive(function, target, abstracted, optimized)
        }
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerLessThan { .. },
            ..
        } => integer_less_than_parameters::derive(function, target, abstracted, optimized),
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerLessOrEqual { scalar_type, .. },
            ..
        } if *scalar_type == IntegerType::new(IntegerSign::Signed, 64).expect("i64") => {
            i64_less_or_equal_parameters::derive(function, target, abstracted, optimized)
        }
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerLessOrEqual { .. },
            ..
        } => integer_less_or_equal_parameters::derive(function, target, abstracted, optimized),
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::Not { operand, .. },
            ..
        } if matches!(
            operand.as_ref(),
            TargetBooleanExpression::IntegerEqual { left, right, .. }
                if matches!(left.as_ref(), TargetIntegerExpression::Parameter { .. })
                    && matches!(right.as_ref(), TargetIntegerExpression::Immediate {
                        value: semantic_vocabulary::IntegerValue::Unsigned(0), ..
                    })
        ) =>
        {
            u64_not_equal_zero_parameter::derive(function, target, abstracted, optimized)
        }
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::Not { operand, .. },
            ..
        } if matches!(
            operand.as_ref(),
            TargetBooleanExpression::IntegerEqual { .. }
        ) =>
        {
            integer_not_equal_parameters::derive(function, target, abstracted, optimized)
        }
        _ => Err(Error::UnsupportedSourceShape { function }),
    }
}
