//! Shared exact mechanics for U64 equality followed by Boolean negation.

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
        scalar_type: condition_type,
        left: target_left,
        right: target_right,
    } = operand.as_ref()
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *condition_type != u64_type {
        return Err(Error::UnsupportedCondition { function });
    }
    let [left, right] = [target_left.as_ref(), target_right.as_ref()]
        .map(|expression| derive_parameter(function, expression, abstracted, optimized, u64_type));
    let left = left?;
    let right = right?;
    if left.parameter_index == right.parameter_index {
        return Err(Error::UnsupportedCondition { function });
    }

    let Some(entry) = optimized.blocks.first() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [equality_node, boolean_not_node, ..] = entry.nodes.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let Some([abstract_equality, abstract_boolean_not, ..]) = abstracted.operations.get(0..) else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::IntegerEqual {
        psi_operation: abstract_equality_operation,
        result: equality_result,
        left: abstract_left,
        right: abstract_right,
    } = abstract_equality
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if equality_node.operation != *abstract_equality
        || abstract_equality_operation != equality_operation
        || *abstract_left != left.source_value
        || *abstract_right != right.source_value
        || equality_node.definitions.len() != 1
        || equality_node.definitions[0].value != *equality_result
        || equality_node.definitions[0].scalar_type != ScalarType::Boolean
        || equality_node.provenance != vec![PsiProvenance::Operation(*equality_operation)]
        || !equality_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let equality_fuel = exact_operation_fuel(equality_node, *equality_operation, function)?;

    let AbstractOperation::BooleanNot {
        psi_operation: abstract_boolean_not_operation,
        result: boolean_not_result,
        operand: abstract_operand,
    } = abstract_boolean_not
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if boolean_not_node.operation != *abstract_boolean_not
        || abstract_boolean_not_operation != boolean_not_operation
        || *abstract_operand != *equality_result
        || *boolean_not_result != *condition_source
        || boolean_not_node.definitions.len() != 1
        || boolean_not_node.definitions[0].value != *boolean_not_result
        || boolean_not_node.definitions[0].scalar_type != ScalarType::Boolean
        || boolean_not_node.provenance != vec![PsiProvenance::Operation(*boolean_not_operation)]
        || !boolean_not_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let boolean_not_fuel =
        exact_operation_fuel(boolean_not_node, *boolean_not_operation, function)?;

    Ok(DerivedCondition {
        source: *condition_source,
        legalized: LegalizedCondition::IntegerNotEqualParametersV1 {
            equality_operation: *equality_operation,
            equality_result: *equality_result,
            equality_result_definition_site: equality_node.definitions[0].site,
            equality_fuel,
            boolean_not_operation: *boolean_not_operation,
            boolean_not_result: *boolean_not_result,
            boolean_not_result_definition_site: boolean_not_node.definitions[0].site,
            boolean_not_fuel,
            left,
            right,
        },
        shape: ScalarConditionShape::IntegerNotEqualU64Parameters,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 2,
        provenance_operations: vec![*equality_operation, *boolean_not_operation],
    })
}

fn derive_parameter(
    function: usize,
    expression: &TargetIntegerExpression,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    integer_type: IntegerType,
) -> Result<LegalizedConditionParameter, LegalizationError> {
    let TargetIntegerExpression::Parameter {
        source_value,
        parameter_index,
        location: ScalarParameterLocation::Register(register),
    } = expression
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(parameter) = optimized.parameters.get(*parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(abstract_parameter) = abstracted.parameters.get(*parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    if parameter.value != *source_value
        || parameter.scalar_type != ScalarType::Integer(integer_type)
        || abstract_parameter.value != *source_value
        || abstract_parameter.scalar_type != ScalarType::Integer(integer_type)
    {
        return Err(Error::UnsupportedCondition { function });
    }
    Ok(LegalizedConditionParameter {
        source_value: *source_value,
        parameter_index: *parameter_index,
        register: *register,
        definition_site: parameter.site,
    })
}
