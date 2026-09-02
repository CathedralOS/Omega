//! Shared exact envelope for ordered U64 entry-parameter comparisons.

use super::*;
use crate::legalization::source::leaves::exact_operation_fuel;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub(super) fn derive<'a>(
    kind: Kind,
    function: usize,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
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
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *condition_type != u64_type {
        return Err(Error::UnsupportedCondition { function });
    }
    let [left, right] = [target_left, target_right].map(|expression| {
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
        || *abstract_left != left.source_value
        || *abstract_right != right.source_value
        || comparison_node.definitions.len() != 1
        || comparison_node.definitions[0].value != *condition_source
        || comparison_node.definitions[0].scalar_type != ScalarType::Boolean
        || comparison_node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
        || !comparison_node.successors.is_empty()
    {
        return Err(Error::UnsupportedCondition { function });
    }
    let fuel = exact_operation_fuel(comparison_node, *psi_operation, function)?;
    let result_definition_site = comparison_node.definitions[0].site;
    let (legalized, shape) = match kind {
        Kind::Equal => (
            LegalizedCondition::IntegerEqualParametersV1 {
                operation: *psi_operation,
                result_definition_site,
                fuel,
                left,
                right,
            },
            ScalarConditionShape::IntegerEqualU64Parameters,
        ),
        Kind::LessThan => (
            LegalizedCondition::IntegerLessThanParametersV1 {
                operation: *psi_operation,
                result_definition_site,
                fuel,
                left,
                right,
            },
            ScalarConditionShape::IntegerLessThanU64Parameters,
        ),
        Kind::LessOrEqual => (
            LegalizedCondition::IntegerLessOrEqualParametersV1 {
                operation: *psi_operation,
                result_definition_site,
                fuel,
                left,
                right,
            },
            ScalarConditionShape::IntegerLessOrEqualU64Parameters,
        ),
    };
    Ok(DerivedCondition {
        source: *condition_source,
        legalized,
        shape,
        result_type: *result_type,
        when_true,
        when_false,
        conditional_node_index: 1,
        provenance_operations: vec![*psi_operation],
    })
}
