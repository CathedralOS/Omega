use super::super::shared::*;
use super::frame::assign_call_arguments;
use super::integer::assign_expression;

pub(super) fn assign_boolean_expression(
    expression: &TargetBooleanExpression,
    locations: &BTreeMap<usize, AssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<AssignedBooleanExpression, AssignmentError> {
    match expression {
        TargetBooleanExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => Ok(AssignedBooleanExpression::Call {
            psi_operation: *psi_operation,
            source_value: *source_value,
            callee: *callee,
            arguments: assign_call_arguments(arguments, locations, architecture, next_spill)?,
        }),
        TargetBooleanExpression::Immediate {
            source_value,
            value,
        } => Ok(AssignedBooleanExpression::Immediate {
            source_value: *source_value,
            value: *value,
        }),
        TargetBooleanExpression::Parameter {
            source_value,
            parameter_index,
            ..
        } => Ok(AssignedBooleanExpression::Parameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: *locations.get(parameter_index).ok_or(
                AssignmentError::ExpressionParameterAssignmentMissing {
                    value: *source_value,
                    parameter_index: *parameter_index,
                },
            )?,
        }),
        TargetBooleanExpression::StructuralField {
            psi_operation,
            source_value,
            source,
            field,
            source_placement,
            field_byte_offset,
        } => Ok(AssignedBooleanExpression::StructuralField {
            psi_operation: *psi_operation,
            source_value: *source_value,
            source: *source,
            field: *field,
            source_placement: source_placement.clone(),
            field_byte_offset: *field_byte_offset,
        }),
        TargetBooleanExpression::Not {
            psi_operation,
            operand,
        } => Ok(AssignedBooleanExpression::Not {
            psi_operation: *psi_operation,
            operand: Box::new(assign_boolean_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::Equal {
            psi_operation,
            left,
            right,
        } => Ok(AssignedBooleanExpression::Equal {
            psi_operation: *psi_operation,
            left: Box::new(assign_boolean_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_boolean_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::IntegerEqual {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(AssignedBooleanExpression::IntegerEqual {
            psi_operation: *psi_operation,
            scalar_type: *scalar_type,
            left: Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::IntegerLessThan {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(AssignedBooleanExpression::IntegerLessThan {
            psi_operation: *psi_operation,
            scalar_type: *scalar_type,
            left: Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::IntegerLessOrEqual {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(AssignedBooleanExpression::IntegerLessOrEqual {
            psi_operation: *psi_operation,
            scalar_type: *scalar_type,
            left: Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
    }
}
