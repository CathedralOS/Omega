//! Input-only join from the exact CallPlan to register operands and pointer slots.

use super::shared::*;
mod borrowed_argument;
use borrowed_argument::validate_borrowed_argument;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction};
use register_environment::ValidatedTargetRegisterEnvironment;

use crate::structural_reference_input::stack_pointer_offset;

#[cfg(test)]
mod fixed_array_tests;

/// Incoming scalar slots compose with the admitted primitive-write result ABI.
/// Other result-bearing structural signatures retain their current admission.
pub(super) fn accepts_stack_parameter_entry(source: &LegalizedScalarFunction) -> bool {
    source.call_plan.result.is_none()
        || crate::unobserved_owned_input::accepts(source)
        || source.structural.as_ref().is_some_and(|signature| {
            let parameters = signature
                .parameters
                .iter()
                .map(|parameter| crate::structural_unit_input::Parameter {
                    semantic: &parameter.semantic,
                    target: &parameter.target,
                })
                .collect::<Vec<_>>();
            crate::structural_unit_input::accepts_write_borrow(
                &source.call_plan,
                &parameters,
                &signature.structural_types,
            )
        })
}

/// One complete scalar ABI stack fragment, excluding borrowed-pointer placement.
pub(super) fn scalar_stack_placement(
    placement: &calling_conventions::ValuePlacement,
) -> Option<(u32, u16, u16)> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                alignment,
            },
        ] if matches!(
            placement.shape.class,
            calling_conventions::ValueClass::Float | calling_conventions::ValueClass::Integer
        ) && (matches!(*byte_size, 4 | 8)
            || placement.shape.class == calling_conventions::ValueClass::Integer
                && matches!(*byte_size, 1 | 2))
            && *byte_size == placement.shape.byte_size
            && *alignment >= placement.shape.alignment
            && alignment.is_power_of_two()
            && stack_byte_offset.is_multiple_of(u32::from(*alignment)) =>
        {
            Some((*stack_byte_offset, *byte_size, *alignment))
        }
        _ => None,
    }
}

pub(super) fn register_argument_count(call: &LegalizedScalarCall) -> usize {
    call.arguments
        .iter()
        .filter(|argument| {
            stack_pointer_offset(argument.placement()).is_none()
                && scalar_stack_placement(argument.placement()).is_none()
        })
        .count()
}

/// Preserve ABI register-bank order while keeping source arguments in their authored roster.
pub(super) fn register_argument_order(call: &LegalizedScalarCall) -> Vec<usize> {
    let mut order = call
        .arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| {
            stack_pointer_offset(argument.placement()).is_none()
                && scalar_stack_placement(argument.placement()).is_none()
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if call.result_placement.is_none() {
        order.sort_by_key(|index| {
            call.arguments[*index].placement().shape.class == calling_conventions::ValueClass::Float
        });
    }
    order
}

fn placement_register(
    placement: &calling_conventions::ValuePlacement,
) -> Option<target_operations::MachineRegister> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                ..
            },
        ]
        | [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                ..
            },
        ] => Some(*register),
        _ => None,
    }
}

pub(super) fn unit_key(
    call: &LegalizedScalarCall,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<RegisterConstraintKey> {
    let order = register_argument_order(call);
    let mut views = order
        .iter()
        .map(|index| {
            environment.fixed_register_view(placement_register(call.arguments[*index].placement())?)
        })
        .collect::<Option<Vec<_>>>()?;
    let inputs = views.len();
    if call.structural_result.is_some() {
        for location in &call.result_placement.as_ref()?.locations {
            let ValueLocation::Register { register, .. } = location else {
                return None;
            };
            views.push(environment.fixed_register_view(*register)?);
        }
    }
    let keys = environment.selected_keys();
    let candidate_keys = if call.structural_result.is_some() {
        keys.call_aggregate.iter().collect::<Vec<_>>()
    } else {
        keys.call_unit.iter().chain(&keys.call_unit_mixed).collect()
    };
    let mut matches =
        candidate_keys.into_iter().filter(|key| {
            environment.constraint(**key).is_some_and(|row| {
                row.operands.len() == views.len()
                    && row.operands.iter().zip(&views).enumerate().all(
                        |(position, (operand, view))| {
                            operand.access
                                == if position < inputs {
                                    RegisterOperandAccess::Use
                                } else {
                                    RegisterOperandAccess::Def
                                }
                                && operand.fixed_view == Some(*view)
                        },
                    )
            })
        });
    let result = *matches.next()?;
    matches.next().is_none().then_some(result)
}

pub(super) fn validate(
    function: usize,
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
    key: RegisterConstraintKey,
    row: &RegisterInstructionConstraint,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    call.validate_shape().map_err(|_| invalid())?;
    if call
        .arguments
        .iter()
        .any(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. }))
    {
        if source.call_plan.policy != CallingPolicy::native_for_target(environment.target()) {
            return Err(invalid());
        }
        validate_borrowed_argument(source, call, operation).ok_or_else(invalid)?;
    }
    let count = call.arguments.len();
    let register_count = register_argument_count(call);
    let result = call.call_plan.result.as_ref();
    let aggregate = if call.structural_result.is_some() {
        Some(
            crate::selection::scalar_case_input::call_result(source, call)
                .ok_or_else(invalid)?
                .1,
        )
    } else {
        None
    };
    let selected_keys = environment.selected_keys();
    let keys = if result.is_some() {
        &selected_keys.call_i64
    } else {
        &selected_keys.call_unit
    };
    if (if aggregate.is_some() {
        unit_key(call, environment)
    } else if result.is_some() {
        keys.get(register_count).copied()
    } else {
        unit_key(call, environment)
    }) != Some(key)
        || environment.constraint(key) != Some(row)
        || row.key != key
        || call.call_plan.parameters.len() != count
        || row.operands.len()
            != register_count
                + aggregate.map_or(usize::from(result.is_some()), |placement| {
                    placement.locations.len()
                })
    {
        return Err(invalid());
    }
    if result != call.result_placement.as_ref() {
        return Err(invalid());
    }
    let order = register_argument_order(call);
    for (index, placement) in call.call_plan.parameters.iter().chain(result).enumerate() {
        if index == count && aggregate.is_some() {
            for (fragment, location) in placement.locations.iter().enumerate() {
                let ValueLocation::Register { register, .. } = location else {
                    return Err(invalid());
                };
                let operand = row
                    .operands
                    .get(register_count + fragment)
                    .ok_or_else(invalid)?;
                if operand.access != RegisterOperandAccess::Def
                    || operand.fixed_view.is_none()
                    || operand.fixed_view != environment.fixed_register_view(*register)
                {
                    return Err(invalid());
                }
            }
            continue;
        }
        if scalar_stack_placement(placement).is_some() {
            if !matches!(call.arguments.get(index), Some(LegalizedScalarArgument::Scalar { source: value, placement: actual })
                if actual == placement && scalar_value_shape(source, *value) == Some(placement.shape))
            {
                return Err(invalid());
            }
            continue;
        }
        if stack_pointer_offset(placement).is_some() {
            if !matches!(call.arguments.get(index), Some(LegalizedScalarArgument::Structural { target, .. }) if target.destination == *placement)
            {
                return Err(invalid());
            }
            continue;
        }
        let operand_index = if index == count {
            register_count
        } else {
            order
                .iter()
                .position(|argument| *argument == index)
                .ok_or_else(invalid)?
        };
        let operand = row.operands.get(operand_index).ok_or_else(invalid)?;
        let register = match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] if placement.shape.class == calling_conventions::ValueClass::BorrowedReference
                && matches!(
                    call.arguments.get(index),
                    Some(LegalizedScalarArgument::Structural { .. })
                ) =>
            {
                register
            }
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == placement.shape.byte_size && matches!(*byte_size, 1 | 2 | 4 | 8) => {
                register
            }
            [
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(register),
                    copy_stack_byte_offset: None,
                    byte_size,
                    alignment,
                },
            ] if placement.shape.class == calling_conventions::ValueClass::BorrowedReference
                && *byte_size == placement.shape.byte_size
                && *alignment == placement.shape.alignment
                && matches!(
                    call.arguments.get(index),
                    Some(LegalizedScalarArgument::Structural { .. })
                ) =>
            {
                register
            }
            _ => return Err(invalid()),
        };
        if operand.fixed_view.is_none()
            || operand.fixed_view != environment.fixed_register_view(*register)
            || operand.access
                != if index == count {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                }
        {
            return Err(invalid());
        }
        if let Some(argument) = call.arguments.get(index) {
            if argument.placement() != placement {
                return Err(invalid());
            }
            if let LegalizedScalarArgument::Scalar { source: value, .. } = argument
                && scalar_value_shape(source, *value) != Some(placement.shape)
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

pub(super) fn scalar_shape(scalar_type: ScalarType) -> Option<ValueShape> {
    match scalar_type {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(ValueShape::float(4))
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(ValueShape::float(8))
        }
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer)
            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Some(ValueShape::integer(integer.bits() / 8, integer.bits() / 8))
        }
        _ => None,
    }
}

pub(super) fn incoming_float_transfer(
    scalar_type: ScalarType,
    keys: &SelectedConstraintKeys,
) -> Option<(SelectedInstructionKind, RegisterConstraintKey)> {
    match scalar_type {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some((
            SelectedInstructionKind::Float32ToBits,
            keys.float32_to_bits?,
        )),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some((
            SelectedInstructionKind::Float64ToBits,
            keys.float64_to_bits?,
        )),
        _ => None,
    }
}

pub(super) fn outgoing_float_transfer(
    scalar_type: ScalarType,
    keys: &SelectedConstraintKeys,
) -> Option<(SelectedInstructionKind, RegisterConstraintKey)> {
    match scalar_type {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some((
            SelectedInstructionKind::BitsToFloat32,
            keys.bits_to_float32?,
        )),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some((
            SelectedInstructionKind::BitsToFloat64,
            keys.bits_to_float64?,
        )),
        _ => None,
    }
}

fn scalar_value_shape(source: &LegalizedScalarFunction, value: ValueId) -> Option<ValueShape> {
    scalar_value_type(source, value).and_then(scalar_shape)
}

fn scalar_value_type(source: &LegalizedScalarFunction, value: ValueId) -> Option<ScalarType> {
    source
        .parameters
        .iter()
        .find(|parameter| parameter.value == value)
        .map(|parameter| parameter.scalar_type)
        .or_else(|| {
            source
                .blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .find(|parameter| parameter.value == value)
                .map(|parameter| parameter.scalar_type)
        })
        .or_else(|| {
            source
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| instruction.result)
                .find(|result| result.value == value)
                .map(|result| result.scalar_type)
        })
}
