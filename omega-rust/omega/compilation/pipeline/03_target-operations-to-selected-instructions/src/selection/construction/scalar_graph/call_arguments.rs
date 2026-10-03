//! Outgoing call arguments. Every scalar-graph call, whatever its result,
//! places each argument in its ABI home here: an owned aggregate as its
//! register fragments, a borrowed structural argument as a pointer, and a
//! scalar as a copy or float transfer into the call row's class. A
//! stack-placed argument is stored in its outgoing slot and yields no
//! operand.
use super::{
    Builder, LegalizedScalarFunction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::row;
use crate::selection::construction::scalar_graph::structural;
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstruction,
};
use terminal_psi::StructuralAccess;

/// Place every argument of `call` and return the register operands in the
/// call row's register order. `invalid` is the caller's refusal.
pub(super) fn marshal(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    call: &LegalizedScalarCall,
    builder: &mut Builder<'_>,
    invalid: impl Fn() -> SelectedInstructionError,
) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let (value, placement) = match argument {
            LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
            } => {
                if semantic.access == StructuralAccess::Owned {
                    let registers = super::aggregate_argument::argument(
                        source,
                        operation,
                        argument_index,
                        semantic,
                        target,
                        builder,
                    )?;
                    operands.extend(
                        registers
                            .into_iter()
                            .map(|register| (argument_index, register)),
                    );
                } else if let Some(pointer) = argument_pointer(
                    builder,
                    source,
                    operation,
                    argument_index,
                    semantic,
                    target,
                    indices,
                )? {
                    operands.push((argument_index, pointer));
                }
                continue;
            }
            LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } => (*value, placement),
        };
        let (_, input, site, scalar_type) = builder.resolve(value).ok_or_else(&invalid)?;
        if crate::selection::scalar_call_abi::scalar_shape(scalar_type) != Some(placement.shape) {
            return Err(invalid());
        }
        if super::scalar_stack::argument(builder, operation, argument_index, value, placement)? {
            continue;
        }
        let output = if let Some((kind, key)) =
            crate::selection::scalar_call_abi::outgoing_float_transfer(
                scalar_type,
                &builder.constraints.keys,
            ) {
            let output = builder.register(value, site, scalar_type)?;
            builder.registers[output.0 as usize].class = row(builder.catalog, key)?
                .operands
                .get(1)
                .ok_or_else(&invalid)?
                .class;
            builder.emit(
                kind,
                key,
                &[input, output],
                SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else {
            builder.copy(input, value, site, scalar_type)?
        };
        operands.push((argument_index, output));
    }
    // Only a register-placed argument yields operands, and the register order
    // names every such argument.
    let order = crate::selection::scalar_call_abi::register_argument_order(call);
    if operands
        .iter()
        .any(|(argument, _)| !order.contains(argument))
    {
        return Err(invalid());
    }
    Ok(order
        .iter()
        .flat_map(|index| {
            operands
                .iter()
                .filter(move |(argument, _)| argument == index)
                .map(|(_, register)| *register)
        })
        .collect())
}

/// Snapshot the projected pointer, then place its bits in the exact outgoing ABI slot.
fn argument_pointer(
    builder: &mut Builder<'_>,
    source: &LegalizedScalarFunction,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let referent = semantic
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent));
    let pointer_place = semantic.place;
    // Whole and field owners use the same read-only presentation. The exact
    // Placement, block coordinate, or home producer is authenticated by the
    // classifier before this emitter reads the live length word.
    let byte_field = if referent {
        None
    } else {
        crate::selection::byte_owner_input::view_argument(
            source,
            operation.operation,
            semantic,
            target,
        )
    };
    let pointer = if referent {
        let offset = crate::selection::reference_input::borrowed_argument(source, semantic, target)
            .filter(|_| indices.is_empty())
            .ok_or_else(|| SelectedInstructionError::custody())?;
        structural::reference::borrowed_argument(builder, operation, semantic.place, offset)?
    } else if matches!(
        target.source,
        target_operations::TargetStructuralArgumentSource::EstablishedElementView { .. }
    ) && !semantic.path.is_empty()
    {
        // An element loan transports the view's backing pointer projected to
        // the element, never the descriptor's own address.
        structural::view_element_pointer(
            builder,
            operation,
            pointer_place,
            target.source_byte_offset,
        )?
    } else {
        structural::call_pointer(builder, operation, pointer_place, target.source_byte_offset)?
    };
    let pointer = if indices.is_empty() {
        pointer
    } else {
        // Each runtime selector scales onto the element address the borrow
        // actually loans; the static prefix is already folded into `pointer`.
        structural::runtime_address::scale(
            builder,
            operation,
            pointer_place,
            target.source_byte_offset,
            pointer,
            indices,
        )?
    };
    let pointer = if let Some(length) = target.fixed_array_length {
        structural::fixed_array_argument(
            builder,
            operation,
            semantic.place,
            argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
            pointer,
            length,
            target.shape.byte_size,
        )?
    } else if let Some((field_offset, _)) = byte_field {
        if field_offset != target.source_byte_offset {
            return Err(SelectedInstructionError::custody());
        }
        structural::byte_field_argument(
            builder,
            operation,
            semantic.place,
            argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
            pointer,
            field_offset,
        )?
    } else {
        pointer
    };
    let Some(stack_byte_offset) =
        crate::structural_inputs::structural_reference_input::stack_pointer_offset(
            &target.destination,
        )
    else {
        return Ok(Some(pointer));
    };
    let invalid = || SelectedInstructionError::custody();
    let slot = selected_instructions::OutgoingArgumentSlotId {
        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
        operation: operation.operation,
        argument_index: argument_index.try_into().map_err(|_| invalid())?,
    };
    builder
        .transport
        .slots
        .push(selected_instructions::SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
            abi_stack_byte_offset: stack_byte_offset,
        });
    builder
        .transport
        .memory
        .push(selected_instructions::SelectedMemoryAccess {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                operation.operation,
            ),
            place: pointer_place,
            byte_offset: 0,
            byte_count: 8,
            role: selected_instructions::SelectedMemoryAccessRole::WriteOutgoing { slot },
        });
    builder.emit(
        SelectedInstructionKind::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 0,
        },
        builder.constraints.keys.store64.ok_or_else(invalid)?,
        &[pointer],
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            ..Default::default()
        },
    )?;
    Ok(None)
}
