//! Independent shared envelope replay for ordered U64 parameter comparisons.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Equal,
    LessThan,
    LessOrEqual,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay<'a>(
    kind: Kind,
    function: usize,
    architecture: omega_target::Architecture,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError> {
    let (
        condition_source,
        psi_operation,
        condition_type,
        target_left,
        target_right,
        result_type,
        when_true,
        when_false,
    ) = match (&target.operation, kind) {
        (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
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
            },
            Kind::Equal,
        )
        | (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source,
                condition:
                    TargetBooleanExpression::IntegerLessThan {
                        psi_operation,
                        scalar_type,
                        left,
                        right,
                    },
                scalar_type: result_type,
                when_true,
                when_false,
            },
            Kind::LessThan,
        )
        | (
            TargetOperation::ReturnIntegerExpressionConditionalControl {
                condition_source,
                condition:
                    TargetBooleanExpression::IntegerLessOrEqual {
                        psi_operation,
                        scalar_type,
                        left,
                        right,
                    },
                scalar_type: result_type,
                when_true,
                when_false,
            },
            Kind::LessOrEqual,
        ) => (
            condition_source,
            psi_operation,
            scalar_type,
            left.as_ref(),
            right.as_ref(),
            result_type,
            when_true,
            when_false,
        ),
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let (operation, result_definition_site, fuel, proposed_left, proposed_right) =
        match (proposed, kind) {
            (
                LegalizedCondition::IntegerEqualParametersV1 {
                    operation,
                    result_definition_site,
                    fuel,
                    left,
                    right,
                },
                Kind::Equal,
            )
            | (
                LegalizedCondition::IntegerLessThanParametersV1 {
                    operation,
                    result_definition_site,
                    fuel,
                    left,
                    right,
                },
                Kind::LessThan,
            )
            | (
                LegalizedCondition::IntegerLessOrEqualParametersV1 {
                    operation,
                    result_definition_site,
                    fuel,
                    left,
                    right,
                },
                Kind::LessOrEqual,
            ) => (operation, result_definition_site, fuel, left, right),
            _ => return Err(Error::NonCanonicalLegalizedPlan),
        };
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *condition_type != u64_type {
        return Err(Error::UnsupportedCondition { function });
    }
    for (expression, proposed_parameter) in
        [(target_left, proposed_left), (target_right, proposed_right)]
    {
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
    let Some(comparison_node) = optimized
        .blocks
        .first()
        .and_then(|block| block.nodes.first())
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let (abstract_operation, result, abstract_left, abstract_right) =
        match (&comparison_node.operation, kind) {
            (
                AbstractOperation::IntegerEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                },
                Kind::Equal,
            )
            | (
                AbstractOperation::IntegerLessThan {
                    psi_operation,
                    result,
                    left,
                    right,
                },
                Kind::LessThan,
            )
            | (
                AbstractOperation::IntegerLessOrEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                },
                Kind::LessOrEqual,
            ) => (psi_operation, result, left, right),
            _ => return Err(Error::UnsupportedCondition { function }),
        };
    if abstract_operation != psi_operation
        || result != condition_source
        || *abstract_left != proposed_left.source_value
        || *abstract_right != proposed_right.source_value
        || comparison_node.definitions.len() != 1
        || comparison_node.definitions[0].value != *condition_source
        || comparison_node.definitions[0].scalar_type != ScalarType::Boolean
        || comparison_node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
        || !comparison_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let expected_fuel = comparison_node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Operation(*psi_operation))
        .collect::<Vec<_>>();
    if expected_fuel.is_empty() || expected_fuel.len() != comparison_node.fuel.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed_source != *condition_source
        || *operation != *psi_operation
        || *result_definition_site != comparison_node.definitions[0].site
        || *fuel != expected_fuel
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    let shape = match kind {
        Kind::Equal => ScalarConditionShape::IntegerEqualU64Parameters,
        Kind::LessThan => ScalarConditionShape::IntegerLessThanU64Parameters,
        Kind::LessOrEqual => ScalarConditionShape::IntegerLessOrEqualU64Parameters,
    };
    Ok(ReplayedCondition {
        source: *condition_source,
        shape,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 1,
        provenance_operations: vec![*psi_operation],
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
