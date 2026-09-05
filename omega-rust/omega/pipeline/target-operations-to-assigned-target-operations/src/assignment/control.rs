use super::expressions::frame::{
    assign_boolean_expression_frame, assign_boolean_expression_frame_preserving,
    assign_integer_expression_frame,
};
use super::expressions::parameters::{
    boolean_control_arms_parameter_locations, integer_control_arms_parameter_locations,
};
use super::placement::assign_direct_location;
use super::shared::*;

pub(super) fn assign_boolean_control_arm(
    arm: &target_operations::TargetConditionalBooleanArm,
    architecture: Architecture,
) -> Result<AssignedConditionalBooleanArm, AssignmentError> {
    Ok(AssignedConditionalBooleanArm {
        psi_edge: arm.psi_edge,
        control: Box::new(assign_boolean_control(&arm.control, architecture)?),
    })
}

pub(super) fn assign_boolean_control(
    control: &TargetBooleanControl,
    architecture: Architecture,
) -> Result<AssignedBooleanControl, AssignmentError> {
    Ok(match control {
        TargetBooleanControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => AssignedBooleanControl::Crash {
            psi_crash_edge: *psi_crash_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TargetBooleanControl::ReturnImmediate {
            psi_return_edge,
            source_value,
            value,
        } => AssignedBooleanControl::ReturnImmediate {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            value: *value,
        },
        TargetBooleanControl::ReturnParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedBooleanControl::ReturnParameter {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetBooleanControl::ReturnNotParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedBooleanControl::ReturnNotParameter {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetBooleanControl::ReturnExpression {
            psi_return_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_boolean_expression_frame(expression, architecture)?;
            AssignedBooleanControl::ReturnExpression {
                psi_return_edge: *psi_return_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TargetBooleanControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => AssignedBooleanControl::Conditional {
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
        TargetBooleanControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = boolean_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedBooleanControl::ConditionalExpression {
                condition_source: *condition_source,
                condition_frame,
                condition,
                when_true: assign_boolean_control_arm(when_true, architecture)?,
                when_false: assign_boolean_control_arm(when_false, architecture)?,
            }
        }
    })
}

pub(super) fn assign_control_arm(
    arm: &target_operations::TargetConditionalIntegerArm,
    architecture: Architecture,
) -> Result<AssignedConditionalIntegerArm, AssignmentError> {
    Ok(AssignedConditionalIntegerArm {
        psi_edge: arm.psi_edge,
        control: Box::new(assign_integer_control(&arm.control, architecture)?),
    })
}

fn assign_integer_control(
    control: &TargetIntegerControl,
    architecture: Architecture,
) -> Result<AssignedIntegerControl, AssignmentError> {
    Ok(match control {
        TargetIntegerControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => AssignedIntegerControl::Crash {
            psi_crash_edge: *psi_crash_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TargetIntegerControl::Return {
            psi_return_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_integer_expression_frame(expression, architecture)?;
            AssignedIntegerControl::Return {
                psi_return_edge: *psi_return_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TargetIntegerControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => AssignedIntegerControl::Conditional {
            condition_source: *condition_source,
            condition_parameter_index: *condition_parameter_index,
            condition_location: assign_direct_location(
                *condition_source,
                *condition_location,
                architecture,
            )?,
            when_true: assign_control_arm(when_true, architecture)?,
            when_false: assign_control_arm(when_false, architecture)?,
        },
        TargetIntegerControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = integer_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedIntegerControl::ConditionalExpression {
                condition_source: *condition_source,
                condition_frame,
                condition,
                when_true: assign_control_arm(when_true, architecture)?,
                when_false: assign_control_arm(when_false, architecture)?,
            }
        }
    })
}
