use super::super::shared::*;
use super::frame::assign_call_arguments;

pub(super) fn assign_expression(
    expression: &TargetIntegerExpression,
    locations: &BTreeMap<usize, AssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<AssignedIntegerExpression, AssignmentError> {
    fn binary(
        psi_operation: OperationId,
        left: &TargetIntegerExpression,
        right: &TargetIntegerExpression,
        locations: &BTreeMap<usize, AssignedScalarLocation>,
        architecture: Architecture,
        next_spill: &mut u32,
        constructor: impl FnOnce(
            OperationId,
            Box<AssignedIntegerExpression>,
            Box<AssignedIntegerExpression>,
        ) -> AssignedIntegerExpression,
    ) -> Result<AssignedIntegerExpression, AssignmentError> {
        Ok(constructor(
            psi_operation,
            Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        ))
    }
    match expression {
        TargetIntegerExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => Ok(AssignedIntegerExpression::Call {
            psi_operation: *psi_operation,
            source_value: *source_value,
            callee: *callee,
            arguments: assign_call_arguments(arguments, locations, architecture, next_spill)?,
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        }),
        TargetIntegerExpression::Immediate {
            source_value,
            value,
        } => Ok(AssignedIntegerExpression::Immediate {
            source_value: *source_value,
            value: *value,
        }),
        TargetIntegerExpression::Parameter {
            source_value,
            parameter_index,
            ..
        } => Ok(AssignedIntegerExpression::Parameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: *locations.get(parameter_index).ok_or(
                AssignmentError::ExpressionParameterAssignmentMissing {
                    value: *source_value,
                    parameter_index: *parameter_index,
                },
            )?,
        }),
        TargetIntegerExpression::StructuralField {
            psi_operation,
            source_value,
            source,
            field,
            source_placement,
            field_byte_offset,
            integer_type,
        } => Ok(AssignedIntegerExpression::StructuralField {
            psi_operation: *psi_operation,
            source_value: *source_value,
            source: *source,
            field: *field,
            source_placement: source_placement.clone(),
            field_byte_offset: *field_byte_offset,
            integer_type: *integer_type,
        }),
        TargetIntegerExpression::BitwiseNot {
            psi_operation,
            operand,
        } => Ok(AssignedIntegerExpression::BitwiseNot {
            psi_operation: *psi_operation,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::IntegerWiden {
            psi_operation,
            source_type,
            operand,
        } => Ok(AssignedIntegerExpression::IntegerWiden {
            psi_operation: *psi_operation,
            source_type: *source_type,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::IntegerExactCast {
            psi_operation,
            obligation,
            source_type,
            operand,
        } => Ok(AssignedIntegerExpression::IntegerExactCast {
            psi_operation: *psi_operation,
            obligation: *obligation,
            source_type: *source_type,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::BitwiseAnd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::BitwiseAnd {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::BitwiseOr {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::BitwiseOr {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::BitwiseXor {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::WrappingShiftLeft {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::WrappingShiftLeft {
            psi_operation: *psi_operation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::WrappingShiftRight {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::WrappingShiftRight {
            psi_operation: *psi_operation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::ExactShiftRight {
            psi_operation,
            obligation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::ExactShiftRight {
            psi_operation: *psi_operation,
            obligation: *obligation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::ExactShiftLeft {
            psi_operation,
            obligation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::ExactShiftLeft {
            psi_operation: *psi_operation,
            obligation: *obligation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::WrappingAdd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactAdd {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactAdd {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingAdd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::SaturatingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::WrappingSubtract {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::WrappingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactSubtract {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactSubtract {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingSubtract {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::WrappingMultiply {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::WrappingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactMultiply {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactMultiply {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingMultiply {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactDivide {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactDivide {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::ExactRemainder {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::WrappingDivide {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::WrappingDivide {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::WrappingRemainder {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::WrappingRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingDivide {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::SaturatingDivide {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingRemainder {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::SaturatingRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
    }
}
