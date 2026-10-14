//! Receiving proof for a placed descriptor's actual allocated backing address.
//! Matching a target row is insufficient: the selected word must derive from
//! that exact unique allocation, with the checked source offset and selectors.
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstruction,
    SelectedInstructionKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{OperationId, ValueId};
use target_operations::{TargetFunction, TargetUnitOperation};
use terminal_psi::{
    PlacedRamCorrespondence, StructuralArgument, StructuralOperationResult, StructuralPathSegment,
    StructuralTypeShape,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Address {
    slot: LocalStorageSlotId,
    offset: u64,
    indices: Vec<(ValueId, u64)>,
}

fn operand(instruction: &SelectedInstruction, position: u16) -> Option<VirtualRegisterId> {
    let mut matches = instruction
        .operands
        .iter()
        .filter(|operand| operand.operand == position);
    let (Some(value), None) = (matches.next(), matches.next()) else {
        return None;
    };
    Some(value.virtual_register)
}

fn all_instructions(selected: &SelectedFunction) -> impl Iterator<Item = &SelectedInstruction> {
    selected.blocks.iter().flat_map(|block| {
        block.instructions.iter().chain(std::iter::once(
            selected_instructions::terminator_instruction(&block.terminator),
        ))
    })
}

fn precedes(
    selected: &SelectedFunction,
    producer: selected_instructions::SelectedInstructionId,
    consumer: selected_instructions::SelectedInstructionId,
) -> bool {
    selected.blocks.iter().any(|block| {
        block
            .instruction_index(producer)
            .zip(block.instruction_index(consumer))
            .is_some_and(|(producer, consumer)| producer < consumer)
    })
}

fn source_value(
    selected: &SelectedFunction,
    register: VirtualRegisterId,
    consumer: &SelectedInstruction,
) -> Option<ValueId> {
    let mut definitions = all_instructions(selected).filter(|row| {
        row.operands.iter().any(|operand| {
            operand.virtual_register == register
                && matches!(
                    operand.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                )
        })
    });
    match selected.virtual_register(register)?.origin {
        VirtualRegisterOrigin::EntryParameter { source_value, .. }
            if definitions.next().is_none() =>
        {
            Some(source_value)
        }
        VirtualRegisterOrigin::BlockParameter {
            source_value,
            block,
            ..
        } if definitions.next().is_none()
            && selected
                .block(block)?
                .instruction_index(consumer.id)
                .is_some() =>
        {
            Some(source_value)
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } if definitions.next()?.id == instruction
            && definitions.next().is_none()
            && precedes(selected, instruction, consumer.id) =>
        {
            Some(source_value)
        }
        _ => None,
    }
}

fn definition<'a>(
    selected: &'a SelectedFunction,
    operation: OperationId,
    register: VirtualRegisterId,
) -> Option<&'a SelectedInstruction> {
    let mut rows = selected.instructions().filter(|row| {
        if !row.provenance.operations.contains(&operation) {
            return false;
        }
        let output = match row.kind {
            SelectedInstructionKind::FrameAddress { .. }
            | SelectedInstructionKind::MaterializeI64 { .. } => 0,
            SelectedInstructionKind::AddressOffset { .. } => 1,
            SelectedInstructionKind::ByteViewAddress
            | SelectedInstructionKind::WrappingMultiplyI64 => 2,
            _ => return false,
        };
        operand(row, output) == Some(register)
    });
    let (Some(row), None) = (rows.next(), rows.next()) else {
        return None;
    };
    let mut definitions = all_instructions(selected).filter(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.virtual_register == register
                && matches!(
                    operand.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                )
        })
    });
    if definitions.next()?.id != row.id || definitions.next().is_some() {
        return None;
    }
    match selected.virtual_register(register)?.origin {
        VirtualRegisterOrigin::AbiTransport { instruction, .. }
        | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
        | VirtualRegisterOrigin::InstructionScratch { instruction, .. }
            if instruction == row.id => {}
        _ => return None,
    }
    Some(row)
}

fn scaled_index(
    selected: &SelectedFunction,
    operation: OperationId,
    register: VirtualRegisterId,
    consumer: &SelectedInstruction,
) -> Option<(ValueId, u64)> {
    let row = definition(selected, operation, register)?;
    if row.kind != SelectedInstructionKind::WrappingMultiplyI64
        || !precedes(selected, row.id, consumer.id)
    {
        return None;
    }
    let value = source_value(selected, operand(row, 0)?, row)?;
    let stride = definition(selected, operation, operand(row, 1)?)?;
    if !precedes(selected, stride.id, row.id) {
        return None;
    }
    let SelectedInstructionKind::MaterializeI64 {
        value: semantic_vocabulary::IntegerValue::Unsigned(stride),
    } = stride.kind
    else {
        return None;
    };
    Some((value, stride.try_into().ok()?))
}

fn address(
    selected: &SelectedFunction,
    operation: OperationId,
    register: VirtualRegisterId,
    remaining: usize,
    consumer: &SelectedInstruction,
) -> Option<Address> {
    if remaining == 0 {
        return None;
    }
    let row = definition(selected, operation, register)?;
    if !precedes(selected, row.id, consumer.id) {
        return None;
    }
    match row.kind {
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => Some(Address {
            slot,
            offset: u64::from(byte_offset),
            indices: Vec::new(),
        }),
        SelectedInstructionKind::AddressOffset { byte_offset } => {
            let mut result = address(selected, operation, operand(row, 0)?, remaining - 1, row)?;
            result.offset = result.offset.checked_add(u64::from(byte_offset))?;
            Some(result)
        }
        SelectedInstructionKind::ByteViewAddress => {
            let mut result = address(selected, operation, operand(row, 0)?, remaining - 1, row)?;
            result
                .indices
                .push(scaled_index(selected, operation, operand(row, 1)?, row)?);
            Some(result)
        }
        _ => None,
    }
}

/// Independently recover an adjacent whole descriptor's current formation.
/// Adjacency in the same target block excludes a store, reseating, escape or
/// call between its exact definition and capture. Other pointer transports
/// remain external address obligations rather than inferred frame origins.
fn frame_source(
    target: &TargetFunction,
    consumer: OperationId,
    source: &StructuralArgument,
    pending: &mut Vec<semantic_vocabulary::PlaceId>,
) -> Option<StructuralArgument> {
    if !source.path.contains(&StructuralPathSegment::Referent) {
        return Some(source.clone());
    }
    if source.path != [StructuralPathSegment::Referent] || pending.contains(&source.place) {
        return None;
    }
    pending.push(source.place);
    let mut definitions = target.graph.operations().filter_map(|row| match row {
        TargetUnitOperation::EstablishReference {
            psi_operation,
            result_home,
            source: prior,
            storage: None,
        } if result_home.place() == source.place => Some((*psi_operation, result_home, prior)),
        _ => None,
    });
    let (Some((producer, home, prior)), None) = (definitions.next(), definitions.next()) else {
        return None;
    };
    let (_, result) = home.operation_result()?;
    let declaration =
        terminal_psi::find_structural_type(&target.graph.structural_types, result.structural_type)?;
    let StructuralTypeShape::Reference { access, .. } = declaration.shape else {
        return None;
    };
    if !source.access.admits(access)
        || !access.admits(prior.access)
        || !result.is_affine_plain()
        || !target.graph.blocks.iter().any(|block| {
            block.operations.windows(2).any(|pair| {
                pair[0]
                    .structural_result_home()
                    .is_some_and(|(operation, _)| operation == producer)
                    && pair[1]
                        .structural_result_home()
                        .is_some_and(|(operation, _)| operation == consumer)
            })
        })
    {
        return None;
    }
    let mut prior_roots = target.graph.operations().filter_map(|row| {
        row.structural_result_home()
            .filter(|(_, home)| home.place() == prior.place)
            .map(|(_, home)| home.structural_type())
    });
    let (Some(prior_root), None) = (prior_roots.next(), prior_roots.next()) else {
        return None;
    };
    let tip = terminal_semantics::runtime_structural_path_tip(
        target.graph.structural_types.iter(),
        prior_root,
        &prior.path,
    )?;
    if !terminal_semantics::reference_source_compatible(
        target.graph.structural_types.iter(),
        tip.id,
        result.structural_type,
    ) {
        return None;
    }
    let resolved = frame_source(target, producer, prior, pending);
    pending.pop();
    resolved
}

pub(super) fn retained(
    operation: OperationId,
    result: &StructuralOperationResult,
    source: &StructuralArgument,
    correspondence: &PlacedRamCorrespondence,
    target: &TargetFunction,
    selected: &SelectedFunction,
) -> Option<()> {
    let frame_source = frame_source(target, operation, source, &mut Vec::new())?;
    let types = &target.graph.structural_types;
    let mut roots = target.graph.operations().filter_map(|row| {
        row.structural_result_home()
            .filter(|(_, home)| home.place() == frame_source.place)
            .map(|(producer, home)| (producer, home.structural_type()))
    });
    let (Some((producer, root)), None) = (roots.next(), roots.next()) else {
        return None;
    };
    let mut allocations = selected
        .local_storage_slots
        .iter()
        .filter(|slot| slot.id.structural_place() == Some(frame_source.place));
    let (Some(allocation), None) = (allocations.next(), allocations.next()) else {
        return None;
    };
    if allocation.id
        != (LocalStorageSlotId::Structural {
            operation: producer,
            place: frame_source.place,
        })
    {
        return None;
    }
    let mut destinations = selected
        .local_storage_slots
        .iter()
        .filter(|slot| slot.id.structural_place() == Some(result.place));
    let (Some(destination_slot), None) = (destinations.next(), destinations.next()) else {
        return None;
    };
    if destination_slot.id
        != (LocalStorageSlotId::Structural {
            operation,
            place: result.place,
        })
        || destination_slot.byte_size != 8
        || destination_slot.alignment != 8
    {
        return None;
    }
    let mut path = Vec::new();
    let mut indices = Vec::new();
    let mut maximum_offset = 0_u64;
    for segment in &frame_source.path {
        if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
            let (_, array) =
                terminal_semantics::canonical_static_projection(types.iter(), root, &path)?;
            let StructuralTypeShape::FixedArray { element, length } = array.shape else {
                return None;
            };
            let (size, alignment) =
                terminal_semantics::reference_storage_layout(types.iter(), element)?;
            let stride = size.checked_add(alignment.checked_sub(1)?)? / alignment * alignment;
            maximum_offset =
                maximum_offset.checked_add(length.checked_sub(1)?.checked_mul(stride)?)?;
            indices.push((*index, stride));
            path.push(StructuralPathSegment::FixedIndex(0));
        } else {
            if matches!(
                segment,
                StructuralPathSegment::Referent | StructuralPathSegment::RepresentationView { .. }
            ) {
                return None;
            }
            path.push(segment.clone());
        }
    }
    let (_, tip) = terminal_semantics::canonical_static_projection(types.iter(), root, &path)?;
    if tip.id != correspondence.schema {
        return None;
    }
    let region = terminal_semantics::reference_storage_region(types.iter(), root, &path)?;
    let modulus = correspondence.required_base.modulus();
    if region.byte_length != correspondence.source_byte_length
        || u64::from(allocation.alignment) % modulus != 0
        || region.byte_offset % modulus != correspondence.required_base.residue()
        || indices.iter().any(|(_, stride)| stride % modulus != 0)
        || maximum_offset
            .checked_add(region.byte_offset)?
            .checked_add(region.byte_length)?
            > u64::from(allocation.byte_size)
    {
        return None;
    }
    let expected = Address {
        slot: allocation.id,
        offset: region.byte_offset,
        indices,
    };
    let mut stores = selected.memory_accesses.iter().filter(|row| {
        row.origin == SelectedMemoryAccessOrigin::Operation(operation)
            && row.place == result.place
            && row.byte_offset == 0
            && row.byte_count == 8
            && row.role == SelectedMemoryAccessRole::WritePlace
    });
    let (Some(store), None) = (stores.next(), stores.next()) else {
        return None;
    };
    let instruction = selected
        .instructions()
        .find(|row| row.id == store.instruction)?;
    if instruction.kind
        != (SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        })
    {
        return None;
    }
    let (store_block, store_position) = selected.blocks.iter().find_map(|block| {
        block
            .instruction_index(instruction.id)
            .map(|position| (block, position))
    })?;
    // Every compiler-owned address/stride temporary has exactly one definition,
    // with its original definition ID, in this block before capture. Scalar
    // selectors are independently tied to their source identity below.
    for row in selected
        .instructions()
        .filter(|row| row.provenance.operations.contains(&operation))
    {
        if matches!(
            row.kind,
            SelectedInstructionKind::FrameAddress { .. }
                | SelectedInstructionKind::AddressOffset { .. }
                | SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::WrappingMultiplyI64
                | SelectedInstructionKind::MaterializeI64 { .. }
        ) {
            if store_block
                .instruction_index(row.id)
                .is_none_or(|position| position >= store_position)
            {
                return None;
            }
        }
    }
    let captured = address(
        selected,
        operation,
        operand(instruction, 1)?,
        frame_source.path.len() + 4,
        instruction,
    )?;
    if captured != expected {
        return None;
    }
    let destination = address(
        selected,
        operation,
        operand(instruction, 0)?,
        4,
        instruction,
    )?;
    if destination.offset != 0
        || !destination.indices.is_empty()
        || destination.slot != destination_slot.id
    {
        return None;
    }
    let mut source_addresses = selected.memory_accesses.iter().filter(|row| {
        row.origin == SelectedMemoryAccessOrigin::Operation(operation)
            && row.place == frame_source.place
            && row.byte_offset == 0
            && u64::from(row.byte_count) == correspondence.source_byte_length
            && row.role
                == SelectedMemoryAccessRole::AddressLocal {
                    slot: allocation.id,
                }
    });
    let (Some(source_address), None) = (source_addresses.next(), source_addresses.next()) else {
        return None;
    };
    let frame = selected
        .instructions()
        .find(|row| row.id == source_address.instruction)?;
    if frame.kind
        != (SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(allocation.id),
            byte_offset: 0,
        })
    {
        return None;
    }
    let mut exact = target.graph.operations().filter(|row| {
        matches!(row,
        TargetUnitOperation::EstablishPlacedReference { psi_operation, result_home,
            source: captured, correspondence: retained } if *psi_operation == operation
            && result_home.operation_result() == Some((operation, result))
            && captured == source && retained == correspondence)
    });
    exact.next()?;
    if exact.next().is_some() {
        return None;
    }
    Some(())
}
