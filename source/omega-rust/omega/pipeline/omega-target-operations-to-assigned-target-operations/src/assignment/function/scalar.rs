use crate::assignment::control::{
    assign_boolean_control, assign_boolean_control_arm, assign_control_arm,
};
use crate::assignment::expressions::frame::{
    assign_boolean_expression_frame, assign_boolean_expression_frame_preserving,
    assign_integer_expression_frame,
};
use crate::assignment::expressions::parameters::{
    boolean_control_arms_parameter_locations, integer_control_arms_parameter_locations,
};
use crate::assignment::placement::assign_direct_location;
use crate::assignment::shared::*;

pub(super) fn assign(
    operation: &TargetOperation,
    architecture: Architecture,
) -> Result<AssignedOperation, AssignmentError> {
    Ok(match operation {
        TargetOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => AssignedOperation::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        } => AssignedOperation::ReturnIntegerImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            value: *value,
        },
        TargetOperation::ReturnBooleanImmediate {
            psi_edge,
            source_value,
            value,
        } => AssignedOperation::ReturnBooleanImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            value: *value,
        },
        TargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } => AssignedOperation::ReturnIntegerParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetOperation::ReturnBooleanParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedOperation::ReturnBooleanParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetOperation::ReturnBooleanNotParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedOperation::ReturnBooleanNotParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetOperation::ReturnBooleanSharedConvergence { psi_edge, control } => {
            AssignedOperation::ReturnBooleanSharedConvergence {
                psi_edge: *psi_edge,
                control: assign_boolean_control(control, architecture)?,
            }
        }
        TargetOperation::ReturnBooleanExpression {
            psi_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_boolean_expression_frame(expression, architecture)?;
            AssignedOperation::ReturnBooleanExpression {
                psi_edge: *psi_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression,
        } => {
            let (frame, expression) = assign_integer_expression_frame(expression, architecture)?;
            AssignedOperation::ReturnIntegerExpression {
                psi_edge: *psi_edge,
                source_value: *source_value,
                scalar_type: *scalar_type,
                frame,
                expression,
            }
        }
        TargetOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            scalar_type,
            when_true,
            when_false,
        } => AssignedOperation::ReturnIntegerConditionalControl {
            condition_source: *condition_source,
            condition_parameter_index: *condition_parameter_index,
            condition_location: assign_direct_location(
                *condition_source,
                *condition_location,
                architecture,
            )?,
            scalar_type: *scalar_type,
            when_true: assign_control_arm(when_true, architecture)?,
            when_false: assign_control_arm(when_false, architecture)?,
        },
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition_source,
            condition,
            scalar_type,
            when_true,
            when_false,
        } => {
            let preserved = integer_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedOperation::ReturnIntegerExpressionConditionalControl {
                condition_source: *condition_source,
                condition_frame,
                condition,
                scalar_type: *scalar_type,
                when_true: assign_control_arm(when_true, architecture)?,
                when_false: assign_control_arm(when_false, architecture)?,
            }
        }
        TargetOperation::ReturnBooleanConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => AssignedOperation::ReturnBooleanConditionalControl {
            condition_source: *condition_source,
            condition_parameter_index: *condition_parameter_index,
            condition_location: assign_direct_location(
                *condition_source,
                *condition_location,
                architecture,
            )?,
            when_true: assign_boolean_control_arm(when_true, architecture)?,
            when_false: assign_boolean_control_arm(when_false, architecture)?,
        },
        TargetOperation::ReturnBooleanExpressionConditionalControl {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = boolean_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedOperation::ReturnBooleanExpressionConditionalControl {
                condition_source: *condition_source,
                condition_frame,
                condition,
                when_true: assign_boolean_control_arm(when_true, architecture)?,
                when_false: assign_boolean_control_arm(when_false, architecture)?,
            }
        }
        _ => unreachable!("scalar assignment receives a scalar carrier"),
    })
}
