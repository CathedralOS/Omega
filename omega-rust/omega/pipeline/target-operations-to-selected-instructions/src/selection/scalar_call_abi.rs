//! Join the exact CallPlan to register operands, result fragments, and pointer slots.

use super::shared::*;
mod borrowed_argument;
mod owned_argument;
use borrowed_argument::validate_borrowed_argument;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction};
use register_environment::ValidatedTargetRegisterEnvironment;

use crate::structural_reference_input::stack_pointer_offset;

#[cfg(test)]
mod fixed_array_tests;

/// Records and primitive arrays share whole-value transport, not source identity.
pub(super) fn owned_value_shape(
    source: &LegalizedScalarFunction,
    structural_type: semantic_vocabulary::StructuralTypeId,
) -> Option<ValueShape> {
    let types = &source.structural.as_ref()?.structural_types;
    crate::structural_reference_input::plain_record_shape(structural_type, types).or_else(|| {
        crate::structural_reference_input::primitive_array_shape(structural_type, types)
    })
}

/// Incoming scalar slots follow the complete graph ABI; result admission is independent.
pub(super) fn accepts_stack_parameter_entry(source: &LegalizedScalarFunction) -> bool {
    source.call_plan.result.is_none()
        || (source.structural.is_none()
            && source.parameters.len() == source.call_plan.parameters.len()
            && source
                .parameters
                .iter()
                .zip(&source.call_plan.parameters)
                .all(|(parameter, placement)| {
                    parameter.placement == *placement
                        && scalar_shape(parameter.scalar_type) == Some(placement.shape)
                }))
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
            crate::structural_unit_input::accepts_graph(
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
        .map(|argument| placement_register_count(argument.placement()))
        .sum()
}

/// Preserve ABI register-bank order while keeping source arguments in their authored roster.
pub(super) fn register_argument_order(call: &LegalizedScalarCall) -> Vec<usize> {
    let mut order = call
        .arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| placement_register_count(argument.placement()) != 0)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    // Unit and aggregate calls share the mixed input-bank rows. Aggregate
    // results append definitions after those inputs; they do not change their
    // order. Sorting only the operand roster preserves authored argument identity.
    order.sort_by_key(|index| {
        call.arguments[*index].placement().shape.class == calling_conventions::ValueClass::Float
    });
    order
}

fn placement_register_count(placement: &calling_conventions::ValuePlacement) -> usize {
    placement
        .locations
        .iter()
        .filter(|location| {
            matches!(
                location,
                ValueLocation::Register { .. }
                    | ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Register(_),
                        ..
                    }
            )
        })
        .count()
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
    let mut views = Vec::new();
    for index in &order {
        let placement = call.arguments[*index].placement();
        if matches!(&call.arguments[*index], LegalizedScalarArgument::Structural { semantic, .. } if semantic.access == StructuralAccess::Owned)
        {
            for location in &placement.locations {
                let register = match location {
                    ValueLocation::Register { register, .. }
                    | ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Register(register),
                        ..
                    } => register,
                    ValueLocation::Stack { .. }
                    | ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Stack { .. },
                        ..
                    } => continue,
                };
                views.push(environment.fixed_register_view(*register)?);
            }
        } else {
            views.push(environment.fixed_register_view(placement_register(placement)?)?);
        }
    }
    let mut inputs = views.len();
    let hidden = call.result_placement.as_ref().and_then(|placement| {
        crate::selection::aggregate_result_input::indirect_result(placement, call.call_plan.policy)
    });
    if let Some(hidden) = hidden {
        views.push(environment.fixed_register_view(hidden)?);
        inputs += 1;
    } else if let Some(result) = &call.result_placement {
        for location in &result.locations {
            let ValueLocation::Register { register, .. } = location else {
                return None;
            };
            views.push(environment.fixed_register_view(*register)?);
        }
    }
    let keys = environment.selected_keys();
    let empty_result = call.structural_result.is_some()
        && call
            .result_placement
            .as_ref()
            .is_some_and(empty_aggregate_placement)
        && call.result_placement == call.call_plan.result;
    let candidate_keys = if call.structural_result.is_some() && !empty_result {
        keys.call_aggregate.iter().collect::<Vec<_>>()
    } else if call.result_placement.is_some() && !empty_result {
        keys.call_scalar.iter().collect::<Vec<_>>()
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

/// Physical classification only; source/result admission still owns structural identity.
pub(super) fn empty_aggregate_placement(placement: &calling_conventions::ValuePlacement) -> bool {
    placement.shape == calling_conventions::ValueShape::integer(0, 1)
        && placement.locations.is_empty()
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
        if call.arguments.iter().any(|argument| matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.access == StructuralAccess::Owned)) {
            owned_argument::validate_owned_arguments(source, call, operation).ok_or_else(invalid)?;
        }
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            if matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.access != StructuralAccess::Owned)
            {
                validate_borrowed_argument(source, call, operation, argument_index)
                    .ok_or_else(invalid)?;
            }
        }
    }
    if call.result_placement.as_ref().is_some_and(|placement| {
        crate::selection::aggregate_result_input::indirect_result(placement, call.call_plan.policy)
            .is_some()
    }) {
        crate::selection::aggregate_result_input::call_result(source, call).ok_or_else(invalid)?;
        let canonical = evaluate_call_plan(
            call.call_plan.policy,
            &CallSignature {
                parameters: call
                    .arguments
                    .iter()
                    .map(|argument| argument.placement().shape)
                    .collect(),
                result: call
                    .result_placement
                    .as_ref()
                    .map(|placement| placement.shape),
            },
        )
        .map_err(|_| invalid())?;
        if call.call_plan != canonical
            || unit_key(call, environment) != Some(key)
            || environment.constraint(key) != Some(row)
            || row.key != key
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let count = call.arguments.len();
    let register_count = register_argument_count(call);
    let result = call.call_plan.result.as_ref();
    let aggregate = if call.structural_result.is_some() {
        Some(
            crate::selection::aggregate_result_input::call_result(source, call)
                .ok_or_else(invalid)?
                .1,
        )
    } else {
        None
    };
    if unit_key(call, environment) != Some(key)
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
        if matches!(call.arguments.get(index), Some(LegalizedScalarArgument::Structural { semantic, .. }) if semantic.access == StructuralAccess::Owned)
        {
            let start = order
                .iter()
                .take_while(|argument| **argument != index)
                .map(|argument| placement_register_count(call.arguments[*argument].placement()))
                .sum::<usize>();
            if call.arguments[index].placement() != placement {
                return Err(invalid());
            }
            for (fragment, location) in placement
                .locations
                .iter()
                .filter(|location| {
                    matches!(
                        location,
                        ValueLocation::Register { .. }
                            | ValueLocation::Indirect {
                                pointer: IndirectPointerLocation::Register(_),
                                ..
                            }
                    )
                })
                .enumerate()
            {
                let register = match location {
                    ValueLocation::Register { register, .. }
                    | ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Register(register),
                        ..
                    } => register,
                    _ => return Err(invalid()),
                };
                let operand = row.operands.get(start + fragment).ok_or_else(invalid)?;
                if operand.access != RegisterOperandAccess::Use
                    || operand.fixed_view.is_none()
                    || operand.fixed_view != environment.fixed_register_view(*register)
                {
                    return Err(invalid());
                }
            }
            continue;
        }
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
            if !order.contains(&index) {
                return Err(invalid());
            }
            order
                .iter()
                .take_while(|argument| **argument != index)
                .map(|argument| placement_register_count(call.arguments[*argument].placement()))
                .sum()
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

/// ABI homes only promise the scalar's low bits, not the rest of the GPR.
pub(super) fn integer_abi_normalization(scalar_type: ScalarType) -> SelectedInstructionKind {
    match scalar_type {
        ScalarType::Boolean => SelectedInstructionKind::ZeroExtendU8,
        ScalarType::Integer(integer) => match (integer.sign(), integer.bits()) {
            (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
            (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
            (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
            (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
            (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
            (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
            _ => SelectedInstructionKind::CopyI64,
        },
        _ => SelectedInstructionKind::CopyI64,
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
