//! Receiving proof for a placed descriptor's actual allocated backing address.
//! Matching a target row is insufficient: the selected word must derive from
//! that exact unique allocation, with the checked source offset and selectors.
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstruction,
    SelectedInstructionKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{OperationId, PlaceId, ValueId};
use target_operations::{TargetFunction, TargetUnitOperation};
use terminal_psi::{StructuralArgument, StructuralOperationResult};

pub(super) mod backing_geometry;

/// Discharge formation geometry once concrete allocations and all callers are
/// available. Selection uses ordinary reference transport, not a special
/// adjacent-definition shortcut. Retained artifact admission independently
/// checks both this physical obligation and the selected pointer expression.
pub(in crate::object::function_fragments) fn establish_geometry(
    source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), super::Error> {
    let current = source.source().source().source().source();
    let abstracted = current.optimized_target().optimized().plan();
    let targeted = current.optimized_target().target_operations();
    let selected = current.selected_plan();
    if !targeted.functions.iter().any(|function| {
        function.graph.operations().any(|operation| {
            matches!(
                operation,
                TargetUnitOperation::EstablishAccessReference { .. }
            )
        })
    }) {
        return Ok(());
    }
    let graph = backing_geometry::completed_graph(abstracted, targeted).ok_or(
        super::Error::Mismatch("placed formation has no complete reference-origin graph"),
    )?;
    let mut geometry =
        backing_geometry::BackingGeometry::new(abstracted, selected, targeted, &graph);
    for function in &targeted.functions {
        for operation in function.graph.operations() {
            if let TargetUnitOperation::EstablishAccessReference {
                psi_operation,
                result_home,
                source,
                correspondence,
            } = operation
            {
                geometry
                    .proves(
                        function.machine,
                        result_home.place(),
                        source,
                        correspondence,
                    )
                    .map_err(|reason| super::Error::AccessBackingGeometry {
                        machine: function.machine,
                        operation: *psi_operation,
                        reason,
                    })?;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AddressBase {
    Allocation(LocalStorageSlotId),
    BorrowedFormal(PlaceId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Address {
    base: AddressBase,
    loads: Vec<(u64, Vec<(ValueId, u64)>)>,
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

pub(super) fn precedes(
    selected: &SelectedFunction,
    producer: selected_instructions::SelectedInstructionId,
    consumer: selected_instructions::SelectedInstructionId,
) -> bool {
    let producer_block = selected
        .blocks
        .iter()
        .find(|block| block.instruction_index(producer).is_some());
    let consumer_block = selected
        .blocks
        .iter()
        .find(|block| block.instruction_index(consumer).is_some());
    let (Some(producer_block), Some(consumer_block)) = (producer_block, consumer_block) else {
        return false;
    };
    if producer_block.id == consumer_block.id {
        return producer_block.instruction_index(producer)
            < consumer_block.instruction_index(consumer);
    }
    // A cross-block definition must dominate use: deleting its block must
    // make the consumer unreachable from entry. No layout-order assumption.
    let mut pending = vec![selected.entry_block];
    let mut visited = Vec::new();
    while let Some(block) = pending.pop() {
        if block == producer_block.id || visited.contains(&block) {
            continue;
        }
        if block == consumer_block.id {
            return false;
        }
        visited.push(block);
        let Some(block) = selected.block(block) else {
            return false;
        };
        pending.extend(
            selected_instructions::terminator_successors(&block.terminator)
                .iter()
                .map(|successor| successor.block),
        );
    }
    true
}

pub(super) fn source_value(
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

fn definition(
    selected: &SelectedFunction,
    operation: OperationId,
    register: VirtualRegisterId,
) -> Option<&SelectedInstruction> {
    let mut rows = selected.instructions().filter(|row| {
        if !row.provenance.operations.contains(&operation)
            && !matches!(
                row.kind,
                SelectedInstructionKind::FrameAddress { .. } | SelectedInstructionKind::CopyI64
            )
        {
            return false;
        }
        let output = match row.kind {
            SelectedInstructionKind::FrameAddress { .. }
            | SelectedInstructionKind::MaterializeI64 { .. } => 0,
            SelectedInstructionKind::AddressOffset { .. }
            | SelectedInstructionKind::Load64 { .. }
            | SelectedInstructionKind::CopyI64 => 1,
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
    target: &TargetFunction,
    operation: OperationId,
    register: VirtualRegisterId,
    remaining: usize,
    consumer: &SelectedInstruction,
) -> Option<Address> {
    if remaining == 0 {
        return None;
    }
    if let VirtualRegisterOrigin::StructuralParameter {
        place,
        parameter_index,
    } = selected.virtual_register(register)?.origin
    {
        let parameter = selected
            .structural
            .as_ref()?
            .parameters
            .get(parameter_index)?;
        if parameter.semantic.place != place
            || parameter.semantic.access == terminal_psi::StructuralAccess::Owned
            || all_instructions(selected).any(|instruction| {
                instruction.operands.iter().any(|operand| {
                    operand.virtual_register == register
                        && matches!(
                            operand.access,
                            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                        )
                })
            })
        {
            return None;
        }
        return Some(Address {
            base: AddressBase::BorrowedFormal(place),
            loads: Vec::new(),
            offset: 0,
            indices: Vec::new(),
        });
    }
    if let Some(address) = block_entry_pointer(target, selected, register, consumer) {
        return Some(address);
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
            base: AddressBase::Allocation(slot),
            loads: Vec::new(),
            offset: u64::from(byte_offset),
            indices: Vec::new(),
        }),
        SelectedInstructionKind::AddressOffset { byte_offset } => {
            let mut result = address(
                selected,
                target,
                operation,
                operand(row, 0)?,
                remaining - 1,
                row,
            )?;
            result.offset = result.offset.checked_add(u64::from(byte_offset))?;
            Some(result)
        }
        SelectedInstructionKind::CopyI64 => address(
            selected,
            target,
            operation,
            operand(row, 0)?,
            remaining - 1,
            row,
        ),
        SelectedInstructionKind::Load64 { byte_offset } => {
            let mut result = address(
                selected,
                target,
                operation,
                operand(row, 0)?,
                remaining - 1,
                row,
            )?;
            result.offset = result.offset.checked_add(u64::from(byte_offset))?;
            result
                .loads
                .push((result.offset, std::mem::take(&mut result.indices)));
            result.offset = 0;
            Some(result)
        }
        SelectedInstructionKind::ByteViewAddress => {
            let mut result = address(
                selected,
                target,
                operation,
                operand(row, 0)?,
                remaining - 1,
                row,
            )?;
            result
                .indices
                .push(scaled_index(selected, operation, operand(row, 1)?, row)?);
            Some(result)
        }
        _ => None,
    }
}

pub(super) fn retained(
    operation: OperationId,
    result: &StructuralOperationResult,
    source: &StructuralArgument,
    correspondence: &abstract_operations::AccessReferenceBacking,
    target: &TargetFunction,
    selected: &SelectedFunction,
    geometry: &mut backing_geometry::BackingGeometry<'_>,
) -> Option<()> {
    geometry
        .proves(target.machine, result.place, source, correspondence)
        .ok()?;
    let types = &target.graph.structural_types;
    let (base, root) = address_root(target, selected, source)?;
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
    let owner = terminal_semantics::runtime_structural_path_tip(types.iter(), root, &source.path)?;
    if owner.id != correspondence.source_type() {
        return None;
    }
    let address_source = correspondence.address_source(source, types.as_slice())?;
    let native = target_operations_to_selected_instructions::reference_formation_geometry(
        root,
        &address_source.path,
        types.as_slice(),
    )?;
    if correspondence.resident_source().is_none()
        && native.selected.as_structural() != Some(correspondence.schema())
    {
        return None;
    }
    let expected = geometry_address(base, &native, correspondence.resident_source().is_some())?;
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
                | SelectedInstructionKind::Load64 { .. }
        ) && store_block
            .instruction_index(row.id)
            .is_none_or(|position| position >= store_position)
        {
            return None;
        }
    }
    let captured = address(
        selected,
        target,
        operation,
        operand(instruction, 1)?,
        selected.virtual_registers.len() + 1,
        instruction,
    );
    let captured = captured?;
    if captured != expected {
        return None;
    }
    let destination = address(
        selected,
        target,
        operation,
        operand(instruction, 0)?,
        4,
        instruction,
    )?;
    if destination.offset != 0
        || !destination.indices.is_empty()
        || !destination.loads.is_empty()
        || destination.base != AddressBase::Allocation(destination_slot.id)
    {
        return None;
    }
    let mut exact = target.graph.operations().filter(|row| {
        matches!(row,
        TargetUnitOperation::EstablishAccessReference { psi_operation, result_home,
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

fn address_root(
    target: &TargetFunction,
    selected: &SelectedFunction,
    source: &StructuralArgument,
) -> Option<(Address, semantic_vocabulary::StructuralTypeId)> {
    use super::structural_homes::{SourceHome, resolve};
    let (root, home) = resolve(target, selected, source.place).ok()?;
    let (base, loads) = match home {
        SourceHome::Allocation { slot, indirect } => (
            AddressBase::Allocation(slot),
            if indirect {
                vec![(0, Vec::new())]
            } else {
                Vec::new()
            },
        ),
        SourceHome::BorrowedFormal(place) => (AddressBase::BorrowedFormal(place), Vec::new()),
    };
    Some((
        Address {
            base,
            loads,
            offset: 0,
            indices: Vec::new(),
        },
        root,
    ))
}

fn unique_definition(
    selected: &SelectedFunction,
    register: VirtualRegisterId,
) -> Option<&SelectedInstruction> {
    let mut definitions = all_instructions(selected).filter(|row| {
        row.operands.iter().any(|operand| {
            operand.virtual_register == register
                && matches!(
                    operand.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                )
        })
    });
    let (Some(row), None) = (definitions.next(), definitions.next()) else {
        return None;
    };
    Some(row)
}

/// Authenticate the block-owned pointer load; an arbitrary source-free load
/// must not gain permission to stand in for a borrowed parameter.
fn block_entry_pointer(
    target: &TargetFunction,
    selected: &SelectedFunction,
    register: VirtualRegisterId,
    consumer: &SelectedInstruction,
) -> Option<Address> {
    let load = unique_definition(selected, register)?;
    if load.kind != (SelectedInstructionKind::Load64 { byte_offset: 0 })
        || load.provenance != Default::default()
        || operand(load, 1) != Some(register)
        || !precedes(selected, load.id, consumer.id)
    {
        return None;
    }
    let pointer = operand(load, 0)?;
    let frame = unique_definition(selected, pointer)?;
    let SelectedInstructionKind::FrameAddress {
        slot:
            FrameStorageSlotId::Local(
                slot @ LocalStorageSlotId::StructuralBlockParameter { block, place },
            ),
        byte_offset: 0,
    } = frame.kind
    else {
        return None;
    };
    if !super::structural_homes::thin_address_join(target, slot)?
        || frame.provenance != Default::default()
        || operand(frame, 0) != Some(pointer)
        || !precedes(selected, frame.id, load.id)
    {
        return None;
    }
    for (register, instruction) in [(register, load.id), (pointer, frame.id)] {
        if selected.virtual_register(register)?.origin
            != (VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset: 0,
            })
        {
            return None;
        }
    }
    let owner = selected
        .blocks
        .iter()
        .find(|row| row.instruction_index(load.id).is_some())?;
    if owner.origin != selected_instructions::SelectedBlockOrigin::Source(block)
        || owner.instruction_index(frame.id).is_none()
    {
        return None;
    }
    let mut allocations = selected
        .local_storage_slots
        .iter()
        .filter(|row| row.id == slot);
    let (Some(allocation), None) = (allocations.next(), allocations.next()) else {
        return None;
    };
    if allocation.byte_size != 8 || allocation.alignment != 8 {
        return None;
    }
    for (instruction, role) in [
        (frame.id, SelectedMemoryAccessRole::AddressLocal { slot }),
        (load.id, SelectedMemoryAccessRole::ReadPlace),
    ] {
        let mut accesses = selected
            .memory_accesses
            .iter()
            .filter(|row| row.instruction == instruction);
        let (Some(access), None) = (accesses.next(), accesses.next()) else {
            return None;
        };
        if access.origin != SelectedMemoryAccessOrigin::Block(block)
            || access.place != place
            || access.byte_offset != 0
            || access.byte_count != 8
            || access.role != role
        {
            return None;
        }
    }
    Some(Address {
        base: AddressBase::Allocation(slot),
        loads: vec![(0, Vec::new())],
        offset: 0,
        indices: Vec::new(),
    })
}

fn geometry_address<T>(
    base: Address,
    native: &target_operations_to_selected_instructions::ReferenceSourceGeometry<T>,
    resident: bool,
) -> Option<Address> {
    let selectors = |range: std::ops::Range<usize>| -> Option<Vec<(ValueId, u64)>> {
        Some(
            native
                .elements
                .get(range)?
                .iter()
                .map(|element| (element.index, u64::from(element.stride)))
                .collect(),
        )
    };
    let mut loads = base.loads;
    let mut first_leaf = 0;
    for slot in &native.pointer_slots {
        loads.push((
            u64::from(slot.byte_offset),
            selectors(slot.indices.clone())?,
        ));
        first_leaf = slot.indices.end;
    }
    let (offset, indices) = if resident {
        loads.push((
            u64::from(native.byte_offset),
            selectors(first_leaf..native.elements.len())?,
        ));
        (0, Vec::new())
    } else {
        (
            u64::from(native.byte_offset),
            selectors(first_leaf..native.elements.len())?,
        )
    };
    Some(Address {
        base: base.base,
        loads,
        offset,
        indices,
    })
}

/// Reconstruct the selected pointer from its original borrowed place and path.
pub(super) fn borrowed_address_matches(
    operation: OperationId,
    source: &StructuralArgument,
    target: &TargetFunction,
    selected: &SelectedFunction,
    consumer: &SelectedInstruction,
    register: VirtualRegisterId,
) -> Result<u32, String> {
    let (base, root) = address_root(target, selected, source).ok_or_else(|| {
        format!("borrowed pointer {:?} for {:?} has no unique source home; parameter={:?}; allocation={:?}", register, source,
            target.graph.parameters.iter().find(|row| row.place == source.place),
            selected.local_storage_slots.iter().find(|row| row.id.structural_place() == Some(source.place)))
    })?;
    let native = target_operations_to_selected_instructions::reference_formation_geometry(
        root,
        &source.path,
        &target.graph.structural_types,
    )
    .ok_or_else(|| {
        format!(
            "borrowed pointer {:?} for {:?} has no address geometry at root {:?}",
            register, source, root
        )
    })?;
    let expected = geometry_address(base, &native, false).ok_or_else(|| {
        format!(
            "borrowed pointer {:?} for {:?} has inconsistent pointer-region indices",
            register, source
        )
    })?;
    let actual = address(
        selected,
        target,
        operation,
        register,
        selected.virtual_registers.len() + 1,
        consumer,
    );
    if actual.as_ref() != Some(&expected) {
        return Err(format!(
            "borrowed pointer {:?} for {:?}: expected {:?}; reconstructed {:?}; definitions: {}",
            register,
            source,
            expected,
            actual,
            pointer_definitions(selected, register, consumer)
        ));
    }
    Ok(native.byte_offset)
}

/// Include only the failed pointer's bounded defining chain in diagnostics.
/// This makes missing transport distinct from a different base or projection.
fn pointer_definitions(
    selected: &SelectedFunction,
    mut register: VirtualRegisterId,
    consumer: &SelectedInstruction,
) -> String {
    let mut chain = Vec::new();
    for _ in 0..8 {
        let origin = selected.virtual_register(register).map(|row| row.origin);
        let mut definitions = all_instructions(selected).filter(|row| {
            row.operands.iter().any(|operand| {
                operand.virtual_register == register
                    && matches!(
                        operand.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    )
            })
        });
        let first = definitions.next();
        let second = definitions.next();
        let Some(row) = first else {
            chain.push(format!(
                "{register:?} origin={origin:?}, no instruction definition"
            ));
            break;
        };
        chain.push(format!("{register:?} origin={origin:?}, definition={:?} {:?}, source={:?}, precedes={}, duplicate={}",
            row.id, row.kind, row.provenance.operations, precedes(selected, row.id, consumer.id), second.is_some()));
        if second.is_some() {
            break;
        }
        if !matches!(
            row.kind,
            SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::AddressOffset { .. }
                | SelectedInstructionKind::Load64 { .. }
                | SelectedInstructionKind::ByteViewAddress
        ) {
            break;
        }
        let Some(input) = operand(row, 0) else {
            break;
        };
        register = input;
    }
    chain.join("; ")
}
