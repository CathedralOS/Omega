//! Exact U64 entry-parameter equality with one authored zero constant.

use super::*;
use crate::legalization::source::leaves::exact_operation_fuel;

pub(super) fn derive<'a>(
    function: usize,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    let TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition_source,
        condition:
            TargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type,
                left,
                right,
            },
        scalar_type: result_type,
        when_true,
        when_false,
    } = &target.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
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
        value: semantic_vocabulary::IntegerValue::Unsigned(0),
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
    let [zero_node, comparison_node, ..] = optimized.blocks[0].nodes.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_zero, abstract_comparison, ..] = abstracted.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if abstract_zero != &zero_node.operation
        || abstract_comparison != &comparison_node.operation
        || abstracted
            .operations
            .get(2)
            .is_some_and(|operation| !matches!(operation, AbstractOperation::Conditional { .. }))
        || optimized.blocks[0]
            .nodes
            .get(2)
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
        || *zero != semantic_vocabulary::IntegerValue::Unsigned(0)
        || zero_node.definitions.len() != 1
        || zero_node.definitions[0].value != *zero_value
        || zero_node.definitions[0].scalar_type != u64_type
        || zero_node.provenance != vec![PsiProvenance::Operation(*zero_operation)]
        || !zero_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let AbstractOperation::IntegerEqual {
        psi_operation: source_operation,
        result,
        left: source_left,
        right: source_right,
    } = &comparison_node.operation
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if source_operation != psi_operation
        || result != condition_source
        || source_left != parameter_value
        || source_right != zero_value
        || comparison_node.definitions.len() != 1
        || comparison_node.definitions[0].value != *condition_source
        || comparison_node.definitions[0].scalar_type != ScalarType::Boolean
        || comparison_node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
        || !comparison_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let zero_fuel = exact_operation_fuel(zero_node, *zero_operation, function)?;
    let comparison_fuel = exact_operation_fuel(comparison_node, *psi_operation, function)?;
    Ok(DerivedCondition {
        source: *condition_source,
        legalized: LegalizedCondition::U64EqualZeroParameterV1 {
            operation: *psi_operation,
            result_definition_site: comparison_node.definitions[0].site,
            fuel: comparison_fuel,
            parameter: LegalizedConditionParameter {
                source_value: *parameter_value,
                parameter_index: *parameter_index,
                register: *register,
                definition_site: parameter.site,
            },
            zero: SourceImmediate {
                source_value: *zero_value,
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
                constant_operation: *zero_operation,
                definition_site: zero_node.definitions[0].site,
                fuel: zero_fuel,
            },
        },
        shape: ScalarConditionShape::U64EqualZeroParameter,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 2,
        provenance_operations: vec![*zero_operation, *psi_operation],
    })
}
