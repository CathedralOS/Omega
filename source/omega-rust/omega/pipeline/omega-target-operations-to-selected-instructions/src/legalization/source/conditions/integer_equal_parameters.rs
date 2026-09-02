//! Exact ordered U64 entry-parameter equality condition reconstruction.

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
            TargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type: condition_type,
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
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *condition_type != u64_type {
        return Err(Error::UnsupportedCondition { function });
    }
    let [left, right] = [left.as_ref(), right.as_ref()].map(|expression| {
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
            || parameter.scalar_type != ScalarType::Integer(u64_type)
            || abstract_parameter.value != *source_value
            || abstract_parameter.scalar_type != ScalarType::Integer(u64_type)
        {
            return Err(Error::UnsupportedCondition { function });
        }
        Ok(LegalizedConditionParameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            register: *register,
            definition_site: parameter.site,
        })
    });
    let left = left?;
    let right = right?;
    if left.parameter_index == right.parameter_index {
        return Err(Error::UnsupportedCondition { function });
    }
    let Some(equal_node) = optimized
        .blocks
        .first()
        .and_then(|block| block.nodes.first())
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::IntegerEqual {
        psi_operation: abstract_operation,
        result,
        left: abstract_left,
        right: abstract_right,
    } = &equal_node.operation
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if abstract_operation != psi_operation
        || result != condition_source
        || *abstract_left != left.source_value
        || *abstract_right != right.source_value
        || equal_node.definitions.len() != 1
        || equal_node.definitions[0].value != *condition_source
        || equal_node.definitions[0].scalar_type != ScalarType::Boolean
        || equal_node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
        || !equal_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let fuel = exact_operation_fuel(equal_node, *psi_operation, function)?;
    Ok(DerivedCondition {
        source: *condition_source,
        legalized: LegalizedCondition::IntegerEqualParametersV1 {
            operation: *psi_operation,
            result_definition_site: equal_node.definitions[0].site,
            fuel,
            left,
            right,
        },
        shape: ScalarConditionShape::IntegerEqualU64Parameters,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 1,
        provenance_operation: Some(*psi_operation),
    })
}
