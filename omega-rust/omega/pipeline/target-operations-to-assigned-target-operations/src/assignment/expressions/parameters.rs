use super::super::shared::*;

pub(super) fn expression_parameter_locations(
    expression: &TargetIntegerExpression,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    fn collect(
        expression: &TargetIntegerExpression,
        locations: &mut BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    ) -> Result<(), AssignmentError> {
        match expression {
            TargetIntegerExpression::Call { arguments, .. } => {
                for argument in arguments {
                    let nested = match &argument.expression {
                        TargetScalarExpression::Boolean(expression) => {
                            boolean_expression_parameter_locations(expression)?
                        }
                        TargetScalarExpression::Integer { expression, .. } => {
                            expression_parameter_locations(expression)?
                        }
                    };
                    merge_expression_locations(locations, nested)?;
                }
            }
            TargetIntegerExpression::Immediate { .. }
            | TargetIntegerExpression::StructuralField { .. } => {}
            TargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            } => {
                if let Some((_, established)) = locations.get(parameter_index) {
                    if established != location {
                        return Err(AssignmentError::ExpressionParameterLocationConflict {
                            value: *source_value,
                            parameter_index: *parameter_index,
                        });
                    }
                } else {
                    locations.insert(*parameter_index, (*source_value, *location));
                }
            }
            TargetIntegerExpression::BitwiseNot { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetIntegerExpression::IntegerWiden { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetIntegerExpression::IntegerExactCast { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetIntegerExpression::WrappingAdd { left, right, .. }
            | TargetIntegerExpression::ExactAdd { left, right, .. }
            | TargetIntegerExpression::BitwiseAnd { left, right, .. }
            | TargetIntegerExpression::BitwiseOr { left, right, .. }
            | TargetIntegerExpression::BitwiseXor { left, right, .. }
            | TargetIntegerExpression::WrappingShiftLeft {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::WrappingShiftRight {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::ExactShiftRight {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::ExactShiftLeft {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::SaturatingAdd { left, right, .. }
            | TargetIntegerExpression::WrappingSubtract { left, right, .. }
            | TargetIntegerExpression::ExactSubtract { left, right, .. }
            | TargetIntegerExpression::SaturatingSubtract { left, right, .. }
            | TargetIntegerExpression::WrappingMultiply { left, right, .. }
            | TargetIntegerExpression::ExactMultiply { left, right, .. }
            | TargetIntegerExpression::SaturatingMultiply { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::ExactDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::ExactRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::WrappingDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::WrappingRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::SaturatingDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::SaturatingRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
        }
        Ok(())
    }
    let mut locations = BTreeMap::new();
    collect(expression, &mut locations)?;
    Ok(locations)
}

pub(super) fn boolean_expression_parameter_locations(
    expression: &TargetBooleanExpression,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    fn collect(
        expression: &TargetBooleanExpression,
        locations: &mut BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    ) -> Result<(), AssignmentError> {
        match expression {
            TargetBooleanExpression::Call { arguments, .. } => {
                for argument in arguments {
                    let nested = match &argument.expression {
                        TargetScalarExpression::Boolean(expression) => {
                            boolean_expression_parameter_locations(expression)?
                        }
                        TargetScalarExpression::Integer { expression, .. } => {
                            expression_parameter_locations(expression)?
                        }
                    };
                    merge_expression_locations(locations, nested)?;
                }
            }
            TargetBooleanExpression::Immediate { .. } => {}
            TargetBooleanExpression::StructuralField { .. } => {}
            TargetBooleanExpression::Parameter {
                source_value,
                parameter_index,
                location,
            } => {
                if let Some((_, established)) = locations.get(parameter_index) {
                    if established != location {
                        return Err(AssignmentError::ExpressionParameterLocationConflict {
                            value: *source_value,
                            parameter_index: *parameter_index,
                        });
                    }
                } else {
                    locations.insert(*parameter_index, (*source_value, *location));
                }
            }
            TargetBooleanExpression::Not { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetBooleanExpression::Equal { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetBooleanExpression::IntegerEqual { left, right, .. }
            | TargetBooleanExpression::IntegerLessThan { left, right, .. }
            | TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
                merge_expression_locations(locations, expression_parameter_locations(left)?)?;
                merge_expression_locations(locations, expression_parameter_locations(right)?)?;
            }
        }
        Ok(())
    }

    let mut locations = BTreeMap::new();
    collect(expression, &mut locations)?;
    Ok(locations)
}

pub(crate) fn integer_control_arms_parameter_locations(
    when_true: &target_operations::TargetConditionalIntegerArm,
    when_false: &target_operations::TargetConditionalIntegerArm,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = integer_control_parameter_locations(&when_true.control)?;
    merge_expression_locations(
        &mut locations,
        integer_control_parameter_locations(&when_false.control)?,
    )?;
    Ok(locations)
}

fn integer_control_parameter_locations(
    control: &TargetIntegerControl,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = BTreeMap::new();
    match control {
        TargetIntegerControl::Crash { .. } => {}
        TargetIntegerControl::Return { expression, .. } => {
            locations = expression_parameter_locations(expression)?;
        }
        TargetIntegerControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => {
            locations.insert(
                *condition_parameter_index,
                (*condition_source, *condition_location),
            );
            merge_expression_locations(
                &mut locations,
                integer_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
        TargetIntegerControl::ConditionalExpression {
            condition,
            when_true,
            when_false,
            ..
        } => {
            locations = boolean_expression_parameter_locations(condition)?;
            merge_expression_locations(
                &mut locations,
                integer_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
    }
    Ok(locations)
}

pub(crate) fn boolean_control_arms_parameter_locations(
    when_true: &target_operations::TargetConditionalBooleanArm,
    when_false: &target_operations::TargetConditionalBooleanArm,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = boolean_control_parameter_locations(&when_true.control)?;
    merge_expression_locations(
        &mut locations,
        boolean_control_parameter_locations(&when_false.control)?,
    )?;
    Ok(locations)
}

fn boolean_control_parameter_locations(
    control: &TargetBooleanControl,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = BTreeMap::new();
    match control {
        TargetBooleanControl::Crash { .. } | TargetBooleanControl::ReturnImmediate { .. } => {}
        TargetBooleanControl::ReturnParameter {
            source_value,
            parameter_index,
            location,
            ..
        }
        | TargetBooleanControl::ReturnNotParameter {
            source_value,
            parameter_index,
            location,
            ..
        } => {
            locations.insert(*parameter_index, (*source_value, *location));
        }
        TargetBooleanControl::ReturnExpression { expression, .. } => {
            locations = boolean_expression_parameter_locations(expression)?;
        }
        TargetBooleanControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => {
            locations.insert(
                *condition_parameter_index,
                (*condition_source, *condition_location),
            );
            merge_expression_locations(
                &mut locations,
                boolean_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
        TargetBooleanControl::ConditionalExpression {
            condition,
            when_true,
            when_false,
            ..
        } => {
            locations = boolean_expression_parameter_locations(condition)?;
            merge_expression_locations(
                &mut locations,
                boolean_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
    }
    Ok(locations)
}

pub(super) fn merge_expression_locations(
    locations: &mut BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    nested: BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
) -> Result<(), AssignmentError> {
    for (parameter_index, (source_value, location)) in nested {
        if let Some((_, established)) = locations.get(&parameter_index) {
            if established != &location {
                return Err(AssignmentError::ExpressionParameterLocationConflict {
                    value: source_value,
                    parameter_index,
                });
            }
        } else {
            locations.insert(parameter_index, (source_value, location));
        }
    }
    Ok(())
}
