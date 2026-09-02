//! Independent replay of ordered U64 entry-parameter strict-less-than custody.

use super::*;

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
            TargetBooleanExpression::IntegerLessThan {
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
    let LegalizedCondition::IntegerLessThanParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left: proposed_left,
        right: proposed_right,
    } = proposed
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *condition_type != u64_type {
        return Err(Error::UnsupportedCondition { function });
    }
    for (expression, proposed_parameter) in [
        (left.as_ref(), proposed_left),
        (right.as_ref(), proposed_right),
    ] {
        replay_parameter(
            function,
            architecture,
            expression,
            proposed_parameter,
            abstracted,
            optimized,
            u64_type,
        )?;
    }
    if proposed_left.parameter_index == proposed_right.parameter_index {
        return Err(Error::UnsupportedCondition { function });
    }
    let Some(less_than_node) = optimized
        .blocks
        .first()
        .and_then(|block| block.nodes.first())
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::IntegerLessThan {
        psi_operation: abstract_operation,
        result,
        left: abstract_left,
        right: abstract_right,
    } = &less_than_node.operation
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    if abstract_operation != psi_operation
        || result != condition_source
        || *abstract_left != proposed_left.source_value
        || *abstract_right != proposed_right.source_value
        || less_than_node.definitions.len() != 1
        || less_than_node.definitions[0].value != *condition_source
        || less_than_node.definitions[0].scalar_type != ScalarType::Boolean
        || less_than_node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
        || !less_than_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let expected_fuel = less_than_node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Operation(*psi_operation))
        .collect::<Vec<_>>();
    if expected_fuel.is_empty() || expected_fuel.len() != less_than_node.fuel.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed_source != *condition_source
        || *operation != *psi_operation
        || *result_definition_site != less_than_node.definitions[0].site
        || *fuel != expected_fuel
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(ReplayedCondition {
        source: *condition_source,
        shape: ScalarConditionShape::IntegerLessThanU64Parameters,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 1,
        provenance_operation: Some(*psi_operation),
    })
}

fn replay_parameter(
    function: usize,
    architecture: omega_target::Architecture,
    expression: &TargetIntegerExpression,
    proposed: &LegalizedConditionParameter,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    integer_type: IntegerType,
) -> Result<(), LegalizationError> {
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
    if register.architecture() != architecture
        || parameter.value != *source_value
        || parameter.scalar_type != ScalarType::Integer(integer_type)
        || abstract_parameter.value != *source_value
        || abstract_parameter.scalar_type != ScalarType::Integer(integer_type)
    {
        return Err(Error::UnsupportedCondition { function });
    }
    if proposed.source_value != *source_value
        || proposed.parameter_index != *parameter_index
        || proposed.register != *register
        || proposed.definition_site != parameter.site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}
