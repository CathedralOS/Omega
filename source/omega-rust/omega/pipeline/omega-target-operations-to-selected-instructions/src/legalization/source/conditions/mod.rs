//! Optimizer module role: executable entrance. Classifies and reconstructs the exact scalar condition before leaf selection.

mod direct_parameter;
mod integer_equal_parameters;
mod integer_less_or_equal_parameters;
mod integer_less_than_parameters;
mod integer_not_equal_parameters;
mod integer_parameter_comparison;
mod integer_parameter_not_equal;

use super::shared::*;
use crate::legalization::catalog::ScalarConditionShape;

pub(in crate::legalization) struct DerivedCondition<'a> {
    pub source: ValueId,
    pub legalized: LegalizedCondition,
    pub shape: ScalarConditionShape,
    pub result_type: IntegerType,
    pub when_true: &'a TargetConditionalIntegerArm,
    pub when_false: &'a TargetConditionalIntegerArm,
    pub conditional_node_index: usize,
    pub provenance_operations: Vec<OperationId>,
}

pub(super) fn derive<'a>(
    function: usize,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    match &target.operation {
        TargetOperation::ReturnIntegerConditionalControl { .. } => {
            direct_parameter::derive(function, target, abstracted, optimized)
        }
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerEqual { .. },
            ..
        } => integer_equal_parameters::derive(function, target, abstracted, optimized),
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerLessThan { .. },
            ..
        } => integer_less_than_parameters::derive(function, target, abstracted, optimized),
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition: TargetBooleanExpression::IntegerLessOrEqual { .. },
            ..
        } => integer_less_or_equal_parameters::derive(function, target, abstracted, optimized),
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
