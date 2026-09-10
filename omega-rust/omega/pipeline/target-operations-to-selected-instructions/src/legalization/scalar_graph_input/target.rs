//! Target trees witness source values along each checked edge; they never supply executable rows.
use super::*;
use semantic_vocabulary::BlockId;
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression as Boolean, TargetIntegerControl as Control,
    TargetIntegerExpression as Expression, TargetScalarExpression, TargetUnitOperation,
};
mod boolean_parameters;
mod boolean_return;
mod byte_view;
pub(in crate::legalization::scalar_graph_input) mod control_flow;
mod expressions;
mod hosted_scalar;
mod scalar_definitions;
mod structural_call;
mod unit;
pub(super) fn validate_target(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    super::hosted_scalar::validate_tails(native, optimized)?;
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
            | AbstractOperation::ReturnStructural { psi_edge, .. }
            | AbstractOperation::ReturnUnit { psi_edge, .. }
            | AbstractOperation::Jump { psi_edge, .. } => vec![*psi_edge],
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.psi_edge, when_false.psi_edge],
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.psi_edge).collect()
            }
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
        return unit::validate(target, body, abstracted, optimized, native, plan, unit);
    }
    if let TargetOperation::ControlGraph(graph) = &target.operation {
        return control_flow::validate(target, graph, abstracted, optimized, native, plan, unit);
    }
    if boolean_return::uses(&target.operation) {
        return boolean_return::validate(target, abstracted, optimized, native, plan, unit);
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
        available: None,
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
    available: Option<&'a [(ValueId, target_operations::TargetUnitScalarArgumentSource)]>,
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
        if let AbstractOperation::Jump {
            target, bindings, ..
        } = &terminator.operation
        {
            return self.control(*target, control, &bind(aliases, bindings), &path);
        }
        if let Control::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true: true_arm,
            when_false: false_arm,
        } = control
        {
            let AbstractOperation::Conditional {
                condition,
                when_true,
                when_false,
            } = &terminator.operation
            else {
                return false;
            };
            let Some(parameter) = self.scalar_parameters().get(*condition_parameter_index) else {
                return false;
            };
            let Some((base, inverted)) =
                self.parameter_predicate(*condition, aliases, &mut Vec::new())
            else {
                return false;
            };
            if condition_source != condition
                || parameter.value != base
                || parameter.scalar_type != ScalarType::Boolean
                || !location_matches(*condition_location, &parameter.placement)
            {
                return false;
            }
            let (true_arm, false_arm) = if inverted {
                (false_arm, true_arm)
            } else {
                (true_arm, false_arm)
            };
            return when_true.psi_edge == true_arm.psi_edge
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
                );
        }
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
        || (super::scalar_stack(placement)
            && matches!(placement.locations.as_slice(),
                [ValueLocation::Stack { stack_byte_offset, .. }]
                    if location == ScalarParameterLocation::IncomingStack { byte_offset: *stack_byte_offset }))
}
