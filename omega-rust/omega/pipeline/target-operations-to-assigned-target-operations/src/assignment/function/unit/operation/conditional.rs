//! Integer and Boolean conditional-control source assignment.

use std::collections::BTreeMap;

use super::{
    AssignedUnitOperation, AssignedUnitScalarHome, AssignmentError, NativeTarget,
    ScalarParameterLocation, TargetUnitOperation, ValueId, ValueLocation, ValueShape, scalar_call,
};

pub(super) fn assign(
    operation: &TargetUnitOperation,
    body: &target_operations::TargetUnitBody,
    preceding_operations: &[TargetUnitOperation],
    target: NativeTarget,
    assigned_scalar_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
) -> Result<AssignedUnitOperation, AssignmentError> {
    Ok(match operation {
        TargetUnitOperation::ConditionalIntegerEqual {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            when_true,
            when_false,
        } => AssignedUnitOperation::ConditionalIntegerEqual {
            psi_operation: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: scalar_call::assign_known_unit_scalar_source(
                *left,
                preceding_operations,
                assigned_scalar_homes,
            )
            .ok_or(AssignmentError::UnitScalarCallSourceMismatch(
                left.source_value(),
            ))?,
            right: scalar_call::assign_known_unit_scalar_source(
                *right,
                preceding_operations,
                assigned_scalar_homes,
            )
            .ok_or(AssignmentError::UnitScalarCallSourceMismatch(
                right.source_value(),
            ))?,
            when_true: *when_true,
            when_false: *when_false,
        },
        TargetUnitOperation::ConditionalBoolean {
            condition,
            when_true,
            when_false,
        } => {
            let assigned = assigned_scalar_homes
                .get(&condition.source_value)
                .copied()
                .filter(|home| {
                    condition.scalar_type == semantic_vocabulary::ScalarType::Boolean
                        && condition.shape == ValueShape::integer(1, 1)
                        && home.defining_operation == condition.defining_operation
                        && home.source_value == condition.source_value
                        && home.scalar_type == condition.scalar_type
                        && home.shape == condition.shape
                })
                .ok_or(AssignmentError::UnitScalarCallSourceMismatch(
                    condition.source_value,
                ))?;
            AssignedUnitOperation::ConditionalBoolean {
                condition: assigned,
                when_true: *when_true,
                when_false: *when_false,
            }
        }
        TargetUnitOperation::ConditionalBooleanParameter {
            condition,
            when_true,
            when_false,
        } => {
            let matching_parameters = body
                .scalar_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| *parameter == condition)
                .collect::<Vec<_>>();
            let [(parameter_index, parameter)] = matching_parameters.as_slice() else {
                return Err(AssignmentError::UnitScalarCallSourceMismatch(
                    condition.value,
                ));
            };
            if condition.scalar_type != semantic_vocabulary::ScalarType::Boolean
                || condition.placement.shape != ValueShape::integer(1, 1)
                || body.call_plan.parameters.get(*parameter_index) != Some(&parameter.placement)
            {
                return Err(AssignmentError::UnitScalarCallSourceMismatch(
                    condition.value,
                ));
            }
            let location = match condition.placement.locations.as_slice() {
                [
                    ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size: 1,
                    },
                ] => ScalarParameterLocation::Register(*register),
                [
                    ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset: 0,
                        byte_size: 1,
                        ..
                    },
                ] => ScalarParameterLocation::IncomingStack {
                    byte_offset: *stack_byte_offset,
                },
                _ => {
                    return Err(AssignmentError::UnitScalarCallSourceMismatch(
                        condition.value,
                    ));
                }
            };
            AssignedUnitOperation::ConditionalBooleanParameter {
                condition: condition.clone(),
                location: crate::assignment::placement::assign_direct_location(
                    condition.value,
                    location,
                    target.architecture,
                )?,
                when_true: *when_true,
                when_false: *when_false,
            }
        }
        TargetUnitOperation::ConditionalDispatch { fallthrough_edge } => {
            AssignedUnitOperation::ConditionalDispatch {
                fallthrough_edge: *fallthrough_edge,
            }
        }
        TargetUnitOperation::NonreturningTail { psi_edge } => {
            AssignedUnitOperation::NonreturningTail {
                psi_edge: *psi_edge,
            }
        }
        _ => unreachable!("conditional assignment receives only conditional-control operations"),
    })
}
