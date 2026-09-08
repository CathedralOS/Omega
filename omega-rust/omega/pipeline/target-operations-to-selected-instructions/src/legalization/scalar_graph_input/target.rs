//! Target trees witness source values along each checked edge; they never supply executable rows.
use super::*;
use semantic_vocabulary::BlockId;
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression as Boolean, TargetIntegerControl as Control,
    TargetIntegerExpression as Expression, TargetScalarExpression, TargetUnitOperation,
};
mod byte_output;
mod byte_view;
mod expressions;
mod scalar_definitions;
mod structural_call;
mod unit;
mod unit_graph;
pub(super) fn validate_target(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let operations = optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| instruction(node).map(|row| row.0))
        .collect::<Vec<_>>();
    let mut edges = optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| match &node.operation {
            AbstractOperation::Return { psi_edge, .. }
            | AbstractOperation::ReturnUnit { psi_edge, .. }
            | AbstractOperation::Jump { psi_edge, .. } => vec![*psi_edge],
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.psi_edge, when_false.psi_edge],
            _ => vec![],
        })
        .collect::<Vec<_>>();
    let mut target_edges = target.provenance.edges.clone();
    edges.sort();
    target_edges.sort();
    if target.provenance.operations != operations || target_edges != edges {
        return Err(invalid);
    }
    if matches!(
        target.operation,
        TargetOperation::ReturnStructuralScalarCall { .. }
    ) {
        return super::structural_call::validate_target(
            target, abstracted, optimized, native, plan, unit,
        );
    }
    if let TargetOperation::UnitBody(body) = &target.operation {
        if optimized.blocks.len() != 1 {
            return Err(invalid);
        }
        return unit::validate(body, abstracted, optimized, native, plan, unit);
    }
    if let TargetOperation::UnitGraph(graph) = &target.operation {
        return unit_graph::validate(graph, optimized, native, plan, unit);
    }
    let (scalar_type, control) = match &target.operation {
        TargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        } => (
            *scalar_type,
            Control::Return {
                psi_return_edge: *psi_edge,
                source_value: *source_value,
                expression: Expression::Immediate {
                    source_value: *source_value,
                    value: *value,
                },
            },
        ),
        TargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } => (
            *scalar_type,
            Control::Return {
                psi_return_edge: *psi_edge,
                source_value: *source_value,
                expression: Expression::Parameter {
                    source_value: *source_value,
                    parameter_index: *parameter_index,
                    location: *location,
                },
            },
        ),
        TargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression,
        } => (
            *scalar_type,
            Control::Return {
                psi_return_edge: *psi_edge,
                source_value: *source_value,
                expression: expression.clone(),
            },
        ),
        TargetOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            scalar_type,
            when_true,
            when_false,
        } => (
            *scalar_type,
            Control::Conditional {
                condition_source: *condition_source,
                condition_parameter_index: *condition_parameter_index,
                condition_location: *condition_location,
                when_true: when_true.clone(),
                when_false: when_false.clone(),
            },
        ),
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition_source,
            condition,
            scalar_type,
            when_true,
            when_false,
        } => (
            *scalar_type,
            Control::ConditionalExpression {
                condition_source: *condition_source,
                condition: condition.clone(),
                when_true: when_true.clone(),
                when_false: when_false.clone(),
            },
        ),
        _ => return Err(invalid),
    };
    if !matches!(abstracted.result,AbstractFunctionResult::Scalar(result) if result.scalar_type == ScalarType::Integer(scalar_type))
    {
        return Err(invalid);
    }
    let checker = Checker {
        function: target,
        optimized,
        native,
        plan,
        unit,
    };
    if !checker.control(optimized.entry, &control, &[], &[]) {
        return Err(invalid);
    }
    Ok(())
}
struct Checker<'a> {
    function: &'a TargetFunction,
    optimized: &'a PsiOptimizationFunction,
    native: &'a TargetOperationPlan,
    plan: &'a AbstractOperationPlan,
    unit: &'a PsiOptimizationUnit,
}
impl Checker<'_> {
    fn control(
        &self,
        block: BlockId,
        control: &Control,
        aliases: &[(ValueId, ValueId)],
        path: &[BlockId],
    ) -> bool {
        if path.contains(&block) {
            return false;
        }
        if let Control::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } = control
        {
            let Some(parameter) = self
                .function
                .scalar_abi
                .as_ref()
                .and_then(|abi| abi.parameters.get(*condition_parameter_index))
            else {
                return false;
            };
            if parameter.scalar_type != ScalarType::Boolean
                || !location_matches(*condition_location, &parameter.placement)
            {
                return false;
            }
            let parameter_expression = Boolean::Parameter {
                source_value: parameter.value,
                parameter_index: *condition_parameter_index,
                location: *condition_location,
            };
            // The target's direct-parameter form encodes Not(Parameter) by
            // swapping its arms, while retaining the authored Not result ID.
            let (expression, true_arm, false_arm) = if *condition_source == parameter.value {
                (parameter_expression, when_true, when_false)
            } else {
                let Some(operation) = self
                    .optimized
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .find_map(|node| match node.operation {
                        AbstractOperation::BooleanNot {
                            psi_operation,
                            result,
                            operand,
                        } if result == *condition_source && operand == parameter.value => {
                            Some(psi_operation)
                        }
                        _ => None,
                    })
                else {
                    return false;
                };
                (
                    Boolean::Not {
                        psi_operation: operation,
                        operand: Box::new(parameter_expression),
                    },
                    when_false,
                    when_true,
                )
            };
            return self.control(
                block,
                &Control::ConditionalExpression {
                    condition_source: *condition_source,
                    condition: expression,
                    when_true: true_arm.clone(),
                    when_false: false_arm.clone(),
                },
                aliases,
                path,
            );
        }
        let Some(source) = self
            .optimized
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
        else {
            return false;
        };
        let Some(terminator) = source.nodes.last() else {
            return false;
        };
        let mut path = path.to_vec();
        path.push(block);
        match (&terminator.operation, control) {
            (
                AbstractOperation::Jump {
                    target, bindings, ..
                },
                _,
            ) => {
                let bindings = bind(aliases, bindings);
                self.control(*target, control, &bindings, &path)
            }
            (
                AbstractOperation::Return {
                    psi_edge, value, ..
                },
                Control::Return {
                    psi_return_edge,
                    source_value,
                    expression,
                },
            ) => {
                psi_edge == psi_return_edge
                    && value == source_value
                    && self.expression(expression, *value, aliases)
            }
            (
                AbstractOperation::Conditional {
                    condition,
                    when_true,
                    when_false,
                },
                Control::ConditionalExpression {
                    condition_source,
                    condition: expression,
                    when_true: true_arm,
                    when_false: false_arm,
                },
            ) => {
                condition == condition_source
                    && self.boolean(expression, *condition, aliases)
                    && when_true.psi_edge == true_arm.psi_edge
                    && when_false.psi_edge == false_arm.psi_edge
                    && self.control(
                        when_true.target,
                        &true_arm.control,
                        &bind(aliases, &when_true.bindings),
                        &path,
                    )
                    && self.control(
                        when_false.target,
                        &false_arm.control,
                        &bind(aliases, &when_false.bindings),
                        &path,
                    )
            }
            _ => false,
        }
    }
}
fn resolve(value: ValueId, aliases: &[(ValueId, ValueId)]) -> ValueId {
    aliases
        .iter()
        .rev()
        .find(|(parameter, _)| *parameter == value)
        .map_or(value, |(_, source)| *source)
}
fn bind(
    aliases: &[(ValueId, ValueId)],
    bindings: &[abstract_operations::ValueBinding],
) -> Vec<(ValueId, ValueId)> {
    let mut result = aliases.to_vec();
    result.extend(
        bindings
            .iter()
            .map(|binding| (binding.parameter, resolve(binding.argument, aliases))),
    );
    result
}
fn location_matches(location: ScalarParameterLocation, placement: &ValuePlacement) -> bool {
    matches!(placement.locations.as_slice(),[ValueLocation::Register {register,value_byte_offset:0,byte_size}] if *byte_size == placement.shape.byte_size && location == ScalarParameterLocation::Register(*register))
}
