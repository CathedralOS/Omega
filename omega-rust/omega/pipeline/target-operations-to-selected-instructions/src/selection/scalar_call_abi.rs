//! Input-only join from the exact CallPlan to register operands and pointer slots.

use super::shared::*;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction};
use register_environment::ValidatedTargetRegisterEnvironment;

use crate::structural_reference_input::stack_pointer_offset;

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
        ) && matches!(*byte_size, 4 | 8)
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
    let views = order
        .iter()
        .map(|index| {
            environment.fixed_register_view(placement_register(call.arguments[*index].placement())?)
        })
        .collect::<Option<Vec<_>>>()?;
    let keys = environment.selected_keys();
    let mut matches = keys
        .call_unit
        .iter()
        .chain(&keys.call_unit_mixed)
        .filter(|key| {
            environment.constraint(**key).is_some_and(|row| {
                row.operands.len() == views.len()
                    && row.operands.iter().zip(&views).all(|(operand, view)| {
                        operand.access == RegisterOperandAccess::Use
                            && operand.fixed_view == Some(*view)
                    })
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
    let selected_keys = environment.selected_keys();
    let keys = if result.is_some() {
        &selected_keys.call_i64
    } else {
        &selected_keys.call_unit
    };
    if (if result.is_some() {
        keys.get(register_count).copied()
    } else {
        unit_key(call, environment)
    }) != Some(key)
        || environment.constraint(key) != Some(row)
        || row.key != key
        || call.call_plan.parameters.len() != count
        || row.operands.len() != register_count + usize::from(result.is_some())
    {
        return Err(invalid());
    }
    if result != call.result_placement.as_ref() {
        return Err(invalid());
    }
    let order = register_argument_order(call);
    for (index, placement) in call.call_plan.parameters.iter().chain(result).enumerate() {
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

/// Rejoin the original parameter or earlier literal before pointer transport.
/// Callee declarations and source-call correspondence remain legalization inputs.
fn validate_borrowed_argument(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
) -> Option<()> {
    let signature = source.structural.as_ref()?;
    let (last, scalars) = call.arguments.split_last()?;
    let LegalizedScalarArgument::Structural { semantic, target } = last else {
        return None;
    };
    // Only incoming structural parameters participate in the caller's ABI.
    // Local literal descriptors are established in the activation instead.
    if source
        .parameters
        .len()
        .checked_add(signature.parameters.len())?
        != source.call_plan.parameters.len()
        || source
            .parameters
            .iter()
            .zip(&source.call_plan.parameters)
            .any(|(parameter, placement)| {
                scalar_shape(parameter.scalar_type) != Some(placement.shape)
                    || parameter.placement != *placement
            })
    {
        return None;
    }
    let scalar_shapes = scalars
        .iter()
        .map(|argument| {
            let LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } = argument
            else {
                return None;
            };
            let shape = scalar_shape(scalar_value_type(source, *value)?)?;
            (placement.shape == shape).then_some(shape)
        })
        .collect::<Option<Vec<_>>>()?;
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| crate::structural_unit_input::Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect::<Vec<_>>();
    let exclusive = matches!(
        semantic.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    );
    let local = crate::selection::primitive_local_input::local(source, semantic.place);
    let shape = if let Some((_, result, _, referent)) = local {
        if target.structural_type != result.structural_type
            || target.root_structural_type != result.structural_type
            || !semantic.path.is_empty()
            || target.source_byte_offset != 0
        {
            return None;
        }
        ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
    } else if exclusive
        || signature.structural_types.iter().any(|declaration| {
            declaration.id == target.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                )
        })
    {
        exclusive_projection_shape(source, semantic, target)?
    } else {
        ValueShape::borrowed_reference(16, 8)
    };
    let expected = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: scalar_shapes
                .into_iter()
                .chain(std::iter::once(shape))
                .collect(),
            result: call
                .result_placement
                .as_ref()
                .map(|_| ValueShape::integer(8, 8)),
        },
    )
    .ok()?;
    // Unit entry attachments and service ceilings are retained declaration
    // metadata; they do not add ABI arguments. The scalar-result attachment
    // family remains outside this transport contract.
    if (source.attachment.is_some() && !exclusive && source.call_plan.result.is_some())
        || source.ranked.is_some()
        || !signature.entry_claims.is_empty()
        || (source.call_plan.result.is_some() && !signature.published_service_ceiling.is_empty())
        || (!parameters.is_empty()
            && !crate::unobserved_owned_input::accepts(source)
            && !crate::structural_unit_input::accepts_borrowed_view(
                &source.call_plan,
                &parameters,
                &signature.structural_types,
            )
            && !crate::structural_unit_input::accepts_write_borrow(
                &source.call_plan,
                &parameters,
                &signature.structural_types,
            ))
        || call.source != legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit
        || !call.claim_transfers.is_empty()
        || !call.requirement_obligations.is_empty()
        || !call.crash_continuations.is_empty()
        || call.call_plan != expected
        || (!exclusive
            && (semantic.access != StructuralAccess::SharedBorrow || !semantic.path.is_empty()))
        || target.place != semantic.place
        || target.access != semantic.access
        || target.path != semantic.path
        || (!exclusive && target.root_structural_type != target.structural_type)
        || target.shape != shape
        || (!exclusive && target.source_byte_offset != 0)
        || target.fixed_array_length.is_some()
        || target.element_stride.is_some()
        || Some(&target.destination) != expected.parameters.last()
    {
        return None;
    }
    match &target.source {
        target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
            psi_operation,
        } => {
            let (producer, _, _, _) = local?;
            if *psi_operation != producer || producer == operation {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::Placement(placement) => {
            let parameter = signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == semantic.place)?;
            if target.root_structural_type != parameter.semantic.structural_type
                || *placement != parameter.target.placement
            {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::EstablishedByteView { .. }
        | target_operations::TargetStructuralArgumentSource::BlockParameter { .. } => {
            if exclusive {
                return None;
            }
            crate::selection::established_view_input::accepts(source, operation, target)?;
        }
    }
    Some(())
}

/// Reconstruct the pointer displacement from source layout, never from a
/// coincidentally matching destination ABI or supplied byte offset alone.
fn exclusive_projection_shape(
    source: &LegalizedScalarFunction,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
) -> Option<ValueShape> {
    let signature = source.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == semantic.place)?;
    if !matches!(
        (parameter.semantic.access, semantic.access),
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ) | (
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::WriteOnlyBorrow
        ) | (
            StructuralAccess::SharedBorrow,
            StructuralAccess::SharedBorrow
        )
    ) {
        return None;
    }
    let (referent, offset) = crate::structural_reference_input::project(
        parameter.semantic.structural_type,
        &semantic.path,
        &signature.structural_types,
    )?;
    let shape = crate::structural_reference_input::shape(referent, &signature.structural_types)?;
    (target.root_structural_type == parameter.semantic.structural_type
        && target.structural_type == referent
        && target.source_byte_offset == offset)
        .then_some(ValueShape::borrowed_reference(
            shape.byte_size,
            shape.alignment,
        ))
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
