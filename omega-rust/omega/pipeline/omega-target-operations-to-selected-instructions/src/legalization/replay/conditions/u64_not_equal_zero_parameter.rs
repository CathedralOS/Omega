//! Independent replay of exact U64 parameter-not-equals-zero custody.

use super::*;
use crate::legalization::replay::leaf::replay_operation_fuel;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay<'a>(
    function: usize,
    architecture: omega_target::Architecture,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError> {
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
    let LegalizedCondition::U64NotEqualZeroParameterV1 {
        equality_operation: proposed_equality_operation,
        equality_result: proposed_equality_result,
        equality_result_definition_site,
        equality_fuel,
        boolean_not_operation: proposed_boolean_not_operation,
        boolean_not_result: proposed_boolean_not_result,
        boolean_not_result_definition_site,
        boolean_not_fuel,
        parameter: proposed_parameter,
        zero: proposed_zero,
    } = proposed
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
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
        || register.architecture() != architecture
        || parameter.value != *parameter_value
        || parameter.scalar_type != u64_type
        || abstract_parameter.value != *parameter_value
        || abstract_parameter.scalar_type != u64_type
    {
        return Err(Error::UnsupportedCondition { function });
    }
    if proposed_parameter.source_value != *parameter_value
        || proposed_parameter.parameter_index != *parameter_index
        || proposed_parameter.register != *register
        || proposed_parameter.definition_site != parameter.site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
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
    if proposed_zero.source_value != *zero_value
        || proposed_zero.value != psi_core::IntegerValue::Unsigned(0)
        || proposed_zero.constant_operation != *zero_operation
        || proposed_zero.definition_site != zero_node.definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(
        function,
        *zero_operation,
        &zero_node.fuel,
        &proposed_zero.fuel,
    )?;
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
    replay_operation_fuel(
        function,
        *equality_operation,
        &equality_node.fuel,
        equality_fuel,
    )?;
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
    replay_operation_fuel(
        function,
        *boolean_not_operation,
        &boolean_not_node.fuel,
        boolean_not_fuel,
    )?;
    if proposed_source != *condition_source
        || *proposed_equality_operation != *equality_operation
        || *proposed_equality_result != *equality_result
        || *equality_result_definition_site != equality_node.definitions[0].site
        || *proposed_boolean_not_operation != *boolean_not_operation
        || *proposed_boolean_not_result != *boolean_not_result
        || *boolean_not_result_definition_site != boolean_not_node.definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(ReplayedCondition {
        source: *condition_source,
        shape: ScalarConditionShape::U64NotEqualZeroParameter,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 3,
        provenance_operations: vec![*zero_operation, *equality_operation, *boolean_not_operation],
    })
}
