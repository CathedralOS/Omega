//! Independent mechanics for replaying U64 equality followed by Boolean negation.

use super::*;

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
        scalar_type: condition_type,
        left: target_left,
        right: target_right,
    } = operand.as_ref()
    else {
        return Err(Error::UnsupportedCondition { function });
    };
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        equality_operation: proposed_equality_operation,
        equality_result: proposed_equality_result,
        equality_result_definition_site,
        equality_fuel,
        boolean_not_operation: proposed_boolean_not_operation,
        boolean_not_result: proposed_boolean_not_result,
        boolean_not_result_definition_site,
        boolean_not_fuel,
        left,
        right,
    } = proposed
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *condition_type != u64_type {
        return Err(Error::UnsupportedCondition { function });
    }
    replay_parameter(
        function,
        architecture,
        target_left,
        left,
        abstracted,
        optimized,
        u64_type,
    )?;
    replay_parameter(
        function,
        architecture,
        target_right,
        right,
        abstracted,
        optimized,
        u64_type,
    )?;
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
    let expected_equality_fuel =
        exact_operation_fuel(function, equality_node, *equality_operation)?;

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
    let expected_boolean_not_fuel =
        exact_operation_fuel(function, boolean_not_node, *boolean_not_operation)?;

    if proposed_source != *condition_source
        || *proposed_equality_operation != *equality_operation
        || *proposed_equality_result != *equality_result
        || *equality_result_definition_site != equality_node.definitions[0].site
        || *equality_fuel != expected_equality_fuel
        || *proposed_boolean_not_operation != *boolean_not_operation
        || *proposed_boolean_not_result != *boolean_not_result
        || *boolean_not_result_definition_site != boolean_not_node.definitions[0].site
        || *boolean_not_fuel != expected_boolean_not_fuel
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    Ok(ReplayedCondition {
        source: *condition_source,
        shape: ScalarConditionShape::IntegerNotEqualU64Parameters,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 2,
        provenance_operations: vec![*equality_operation, *boolean_not_operation],
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

fn exact_operation_fuel(
    function: usize,
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
) -> Result<Vec<omega_optimization_unit::FuelSettlement>, LegalizationError> {
    let expected = node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Operation(operation))
        .collect::<Vec<_>>();
    if expected.is_empty() || expected.len() != node.fuel.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(expected)
}
