//! Exact U64 entry-parameter inequality with one authored zero constant.

use super::*;
use crate::legalization::source::leaves::exact_operation_fuel;

pub(super) fn derive<'a>(
    function: usize,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition_source,
        condition:
            TargetBooleanExpression::Not {
                psi_operation: boolean_not_operation,
                operand,
            },
        scalar_type: result_type,
        when_true,
        when_false,
    } = &target.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetBooleanExpression::IntegerEqual {
        psi_operation: equality_operation,
        scalar_type,
        left,
        right,
    } = operand.as_ref()
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer);
    if target.attachment.is_some()
        || abstracted.attachment.is_some()
        || optimized.attachment.is_some()
        || abstracted
            .block_entries
            .iter()
            .any(|entry| !entry.parameters.is_empty())
        || optimized
            .blocks
            .iter()
            .any(|block| !block.parameters.is_empty())
        || *scalar_type != u64_integer
        || *result_type != u64_integer
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let TargetIntegerExpression::Parameter {
        source_value: parameter_value,
        parameter_index,
        location: ScalarParameterLocation::Register(register),
    } = left.as_ref()
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    let TargetIntegerExpression::Immediate {
        source_value: zero_value,
        value: psi_core::IntegerValue::Unsigned(0),
    } = right.as_ref()
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(parameter) = optimized.parameters.get(*parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(abstract_parameter) = abstracted.parameters.get(*parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    if *parameter_index != 0
        || optimized.parameters.len() != 1
        || abstracted.parameters.len() != 1
        || parameter.value != *parameter_value
        || parameter.scalar_type != u64_type
        || abstract_parameter.value != *parameter_value
        || abstract_parameter.scalar_type != u64_type
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let [zero_node, equality_node, boolean_not_node, ..] = optimized.blocks[0].nodes.as_slice()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_zero, abstract_equality, abstract_boolean_not, ..] =
        abstracted.operations.as_slice()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if abstract_zero != &zero_node.operation
        || abstract_equality != &equality_node.operation
        || abstract_boolean_not != &boolean_not_node.operation
        || abstracted
            .operations
            .get(3)
            .is_some_and(|operation| !matches!(operation, AbstractOperation::Conditional { .. }))
        || optimized.blocks[0]
            .nodes
            .get(3)
            .is_some_and(|node| !matches!(&node.operation, AbstractOperation::Conditional { .. }))
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let AbstractOperation::IntegerConstant {
        psi_operation: zero_operation,
        result: source_zero,
        scalar_type: zero_type,
        value: zero,
    } = &zero_node.operation
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if source_zero != zero_value
        || *zero_type != u64_type
        || *zero != psi_core::IntegerValue::Unsigned(0)
        || zero_node.definitions.len() != 1
        || zero_node.definitions[0].value != *zero_value
        || zero_node.definitions[0].scalar_type != u64_type
        || zero_node.provenance != vec![PsiProvenance::Operation(*zero_operation)]
        || !zero_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let AbstractOperation::IntegerEqual {
        psi_operation: source_equality_operation,
        result: equality_result,
        left: source_left,
        right: source_right,
    } = &equality_node.operation
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if source_equality_operation != equality_operation
        || source_left != parameter_value
        || source_right != zero_value
        || equality_node.definitions.len() != 1
        || equality_node.definitions[0].value != *equality_result
        || equality_node.definitions[0].scalar_type != ScalarType::Boolean
        || equality_node.provenance != vec![PsiProvenance::Operation(*equality_operation)]
        || !equality_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let AbstractOperation::BooleanNot {
        psi_operation: source_boolean_not_operation,
        result: boolean_not_result,
        operand: source_operand,
    } = &boolean_not_node.operation
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if source_boolean_not_operation != boolean_not_operation
        || source_operand != equality_result
        || boolean_not_result != condition_source
        || boolean_not_node.definitions.len() != 1
        || boolean_not_node.definitions[0].value != *boolean_not_result
        || boolean_not_node.definitions[0].scalar_type != ScalarType::Boolean
        || boolean_not_node.provenance != vec![PsiProvenance::Operation(*boolean_not_operation)]
        || !boolean_not_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let zero_fuel = exact_operation_fuel(zero_node, *zero_operation, function)?;
    let equality_fuel = exact_operation_fuel(equality_node, *equality_operation, function)?;
    let boolean_not_fuel =
        exact_operation_fuel(boolean_not_node, *boolean_not_operation, function)?;
    Ok(DerivedCondition {
        source: *condition_source,
        legalized: LegalizedCondition::U64NotEqualZeroParameterV1 {
            equality_operation: *equality_operation,
            equality_result: *equality_result,
            equality_result_definition_site: equality_node.definitions[0].site,
            equality_fuel,
            boolean_not_operation: *boolean_not_operation,
            boolean_not_result: *boolean_not_result,
            boolean_not_result_definition_site: boolean_not_node.definitions[0].site,
            boolean_not_fuel,
            parameter: LegalizedConditionParameter {
                source_value: *parameter_value,
                parameter_index: *parameter_index,
                register: *register,
                definition_site: parameter.site,
            },
            zero: SourceImmediate {
                source_value: *zero_value,
                value: psi_core::IntegerValue::Unsigned(0),
                constant_operation: *zero_operation,
                definition_site: zero_node.definitions[0].site,
                fuel: zero_fuel,
            },
        },
        shape: ScalarConditionShape::U64NotEqualZeroParameter,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 3,
        provenance_operations: vec![*zero_operation, *equality_operation, *boolean_not_operation],
    })
}
