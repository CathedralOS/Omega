//! Outgoing call arguments. Every scalar-graph call, whatever its result,
//! places each argument in its ABI home here: an owned aggregate as its
//! register fragments, an ordinary borrowed reference as a pointer, and a
//! scalar as a copy or float transfer into the call row's class. A
//! stack-placed argument is stored in its outgoing slot and yields no
//! operand.
//! A boundary borrowed-view operand instead places its descriptor fragments
//! under the retained aggregate ABI, preserving the original backing and loan.
use super::{
    Builder, LegalizedScalarFunction, SelectedInstructionId, SelectedInstructionProvenance,
    VirtualRegisterId,
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
    // Ownership belongs to the complete call. A borrowed argument neither
    // transfers a claim nor forbids an owned sibling from transferring one.
    // Keep the origin's exact transfer/completion event before ABI transport.
    call.validate_source(&operation.ownership)
        .map_err(|_| invalid())?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let (value, placement) = match argument {
            LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
            } => {
                operands.extend(
                    structural_argument(
                        source,
                        operation,
                        argument_index,
                        semantic,
                        target,
                        indices,
                        builder,
                    )?
                    .into_iter()
                    .map(|register| (argument_index, register)),
                );
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

/// Transport a validated structural actual independently of the call mechanism.
/// Ownership selects value bytes versus the original referent; ABI indirection
/// only selects where a private owned copy is placed.
pub(super) fn structural_argument(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    builder: &mut Builder<'_>,
) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
    if semantic.place != target.place
        || semantic.path != target.path
        || semantic.access != target.access
    {
        return Err(SelectedInstructionError::custody());
    }
    if semantic.access == StructuralAccess::Owned {
        return super::aggregate_argument::argument(
            source,
            operation,
            argument_index,
            semantic,
            target,
            builder,
        );
    }
    if target.destination.shape.class != calling_conventions::ValueClass::BorrowedReference
        && source.structural.as_ref().is_some_and(|signature| {
            signature.structural_types.iter().any(|declaration| {
                declaration.id == target.structural_type
                    && declaration.shape.is_borrowed_byte_carrier()
            })
        })
    {
        // Native byte views pass their carrier words, not the address of a
        // call-local descriptor. Fixed views have one pointer word; dynamic
        // views additionally carry an extent. Internal borrowed-descriptor
        // ABIs remain selected by BorrowedReference above.
        if !matches!(target.shape.byte_size, 8 | 16)
            || target.shape.alignment != 8
            || target.destination.shape.byte_size != target.shape.byte_size
            || target.destination.shape.alignment != 8
        {
            return Err(SelectedInstructionError::custody());
        }
        let pointer = referent_pointer(
            builder,
            source,
            operation,
            argument_index,
            semantic,
            target,
            indices,
        )?;
        let block = super::block_containing_operation(source, operation.operation)?;
        return super::aggregate_argument::transport(
            operation,
            argument_index,
            target,
            Some(pointer.pointer),
            target.source_byte_offset,
            pointer.descriptor_words,
            block,
            builder,
        );
    }
    Ok(argument_pointer(
        builder,
        source,
        operation,
        argument_index,
        semantic,
        target,
        indices,
    )?
    .into_iter()
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
    let pointer = referent_pointer(
        builder,
        source,
        operation,
        argument_index,
        semantic,
        target,
        indices,
    )?;
    place_pointer_argument(
        builder,
        operation,
        argument_index,
        semantic.place,
        target,
        pointer.pointer,
    )
}

/// Prepare the exact referent independently of its pointer or descriptor ABI.
struct PreparedReferent {
    pointer: VirtualRegisterId,
    descriptor_words: Option<[VirtualRegisterId; 2]>,
}

fn referent_pointer(
    builder: &mut Builder<'_>,
    source: &LegalizedScalarFunction,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<PreparedReferent, SelectedInstructionError> {
    let reference =
        crate::selection::reference_input::borrowed_argument(source, semantic, target, indices);
    let explicit_reference = semantic
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent));
    if explicit_reference && reference.is_none() {
        return Err(SelectedInstructionError::custody());
    }
    // Stored views cross a pointer at an element selector, without an explicit
    // Referent segment. Geometry owns both crossings and preserves their order.
    let referent = reference.is_some();
    let thin_scalar = if target.shape.class == calling_conventions::ValueClass::BorrowedReference {
        if reference
            .as_ref()
            .is_some_and(|address| address.scalar_descriptor)
        {
            let declarations = &source
                .structural
                .as_ref()
                .ok_or_else(|| SelectedInstructionError::custody())?
                .structural_types;
            crate::structural_inputs::structural_reference_input::scalar_storage_type(
                target.structural_type,
                declarations,
            )
        } else if !referent && semantic.path.is_empty() {
            crate::selection::reference_input::borrowed_scalar_source(source, semantic.place)
        } else {
            None
        }
    } else {
        None
    };
    if let Some(scalar) = thin_scalar
        && terminal_semantics::reference_storage_export_byte_width(scalar, semantic.access)
            .map(u32::from)
            != Some(target.shape.byte_size)
    {
        // A thin ABI carries no codec. Retain the refusal when the original
        // backing of this loan can be narrower than the foreign formal;
        // logical width is not permission to expose adjacent bytes.
        return Err(SelectedInstructionError::custody());
    }
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
            indices,
        )
    };
    let pointer = if let Some(address) = &reference {
        structural::reference::borrowed_argument(builder, operation, semantic.place, address)?
    } else if (!semantic.path.is_empty() || target.fixed_array_length.is_some())
        && (matches!(
            target.source,
            target_operations::TargetStructuralArgumentSource::EstablishedElementView { .. }
        ) || crate::selection::established_view_input::element_view_type(source, pointer_place)
            .is_some()
            || crate::selection::established_view_input::view_type(source, pointer_place).is_some())
    {
        // An element loan transports the view's backing pointer projected to
        // the element, never the descriptor's own address, whether the view
        // was established here or arrived as a parameter or state arrival.
        structural::view_element_pointer(
            source,
            builder,
            operation,
            pointer_place,
            target.source_byte_offset,
        )?
    } else {
        // A whole descriptor, including a single-word fixed byte carrier,
        // keeps its own address under this borrowed ABI. Only an element
        // projection or array presentation lends its backing directly.
        structural::place_pointer(builder, operation, pointer_place, target.source_byte_offset)?
    };
    let pointer = if thin_scalar.is_some() {
        // The proven footprint belongs to the original referent, not the
        // descriptor home. Unwrap its address without copying its contents.
        structural::reference::load_scalar_referent(
            builder,
            operation,
            pointer_place,
            pointer,
            reference.as_ref(),
        )?
    } else {
        pointer
    };
    let pointer = if reference.is_some() || indices.is_empty() {
        pointer
    } else {
        // Contiguous loans apply their selectors here. Reference loans have
        // already consumed each selector in its region around pointer loads;
        // applying the flattened roster again would index the referent twice.
        structural::runtime_address::scale(
            builder,
            operation,
            pointer_place,
            target.source_byte_offset,
            pointer,
            indices,
        )?
    };
    if target.shape.class == calling_conventions::ValueClass::BorrowedScalarReference {
        let pointer = if reference
            .as_ref()
            .is_some_and(|address| address.scalar_descriptor)
            || semantic.path.is_empty()
                && crate::selection::reference_input::borrowed_scalar_source(source, semantic.place)
                    .is_some()
        {
            pointer
        } else {
            let declarations = &source
                .structural
                .as_ref()
                .ok_or_else(|| SelectedInstructionError::custody())?
                .structural_types;
            let codec =
                crate::structural_inputs::structural_reference_input::projected_scalar_codec(
                    target.root_structural_type,
                    &semantic.path,
                    declarations,
                )
                .and_then(runtime_abi::ScalarStorageCodec::encode)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            let codec_register = structural::transport_register(builder, semantic.place, 8)?;
            builder.emit_materialize_i64(
                semantic_vocabulary::IntegerValue::Unsigned(u128::from(codec)),
                codec_register,
                structural::provenance(operation),
            )?;
            structural::stage_descriptor(
                builder,
                operation,
                selected_instructions::LocalStorageSlotId::StructuralCallArgument {
                    operation: operation.operation,
                    argument_index: argument_index
                        .try_into()
                        .map_err(|_| SelectedInstructionError::custody())?,
                },
                pointer,
                codec_register,
            )?
        };
        return Ok(PreparedReferent {
            pointer,
            descriptor_words: None,
        });
    }
    let presentation = if let Some(length) = target.fixed_array_length {
        Some(structural::fixed_array_argument(
            builder,
            operation,
            semantic.place,
            argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
            pointer,
            length,
            target.shape.byte_size,
        )?)
    } else if let Some((field_offset, _)) = byte_field {
        if field_offset != target.source_byte_offset {
            return Err(SelectedInstructionError::custody());
        }
        Some(structural::byte_field_argument(
            builder,
            operation,
            semantic.place,
            argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
            pointer,
            field_offset,
            indices,
        )?)
    } else {
        None
    };
    Ok(match presentation {
        Some(presentation) => PreparedReferent {
            pointer: presentation.address,
            descriptor_words: presentation.words,
        },
        None => PreparedReferent {
            pointer,
            descriptor_words: None,
        },
    })
}

fn place_pointer_argument(
    builder: &mut Builder<'_>,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    pointer_place: semantic_vocabulary::PlaceId,
    target: &target_operations::TargetStructuralArgument,
    pointer: VirtualRegisterId,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let Some(stack_byte_offset) =
        crate::structural_inputs::structural_reference_input::stack_pointer_offset(
            &target.destination,
        )
    else {
        return Ok(Some(pointer));
    };
    let slot = selected_instructions::OutgoingArgumentSlotId {
        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
        invocation_site: operation.operation.into(),
        argument_index: argument_index
            .try_into()
            .map_err(|_| SelectedInstructionError::custody())?,
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
                    .map_err(|_| SelectedInstructionError::custody())?,
            ),
            origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                operation.operation,
            ),
            place: pointer_place,
            byte_offset: 0,
            byte_count: 8,
            role: selected_instructions::SelectedMemoryAccessRole::WriteOutgoing { slot },
        });
    builder.emit_store64(
        selected_instructions::FrameStorageSlotId::Outgoing(slot),
        0,
        pointer,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            ..Default::default()
        },
    )?;
    Ok(None)
}
