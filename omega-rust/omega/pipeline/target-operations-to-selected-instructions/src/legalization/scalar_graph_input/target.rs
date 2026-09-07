//! Target trees witness source values along each checked edge; they never supply executable rows.
use super::*;
use semantic_vocabulary::BlockId;
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression as Boolean, TargetIntegerControl as Control,
    TargetIntegerExpression as Expression, TargetScalarExpression, TargetUnitOperation,
};
mod expressions;
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
    if let TargetOperation::UnitBody(body) = &target.operation {
        if optimized.blocks.len() != 1 {
            return Err(invalid);
        }
        return unit::validate(body, abstracted, optimized, native, plan, unit);
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
    matches!(placement.locations.as_slice(),[ValueLocation::Register {register,value_byte_offset:0,byte_size:8}] if location == ScalarParameterLocation::Register(*register))
}
