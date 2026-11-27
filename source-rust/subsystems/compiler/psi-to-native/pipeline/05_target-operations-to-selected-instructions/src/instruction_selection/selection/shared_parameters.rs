/*
We let a nested state reuse the parameter storage of the state that enters it,
so loops built from nested states stop copying views and records on every
transition.

A row loop such as `step(raw, out, r, i) -> put(raw, out, r, i, ...) ->
step(raw, out, r, i + 1)` gives `step` and `put` separate stack slots for
`raw`, `out` and `r`. Every transition then copies two 16-byte descriptors
and the record from one state's slots into the other's, twice per iteration.

When every edge into block `B` passes parameter `X` of one other block `A`,
unchanged and whole, as `B`'s parameter `Y`, then `Y` always holds the bytes
`X` held when `B` was entered. `Y` can then live in `X`'s slot:

- `X`'s slot is written only by edges entering `A`. Psi scopes `Y` to blocks
  that `B` dominates, and `A` is entered from outside `B`'s region (it is
  `B`'s only source of `Y`), so no use of `Y` is reached through an edge into
  `A` without passing `B` again, which re-establishes `Y` as the current `X`.
- Neither slot may be written in the body. A borrowed view or reference is a
  descriptor the body cannot reassign; an owned value qualifies only when no
  instruction writes its place.

The edges `A -> B` and `B -> A` then copy a slot onto itself, which
`descriptors::store_direct` omits. Any other binding on such an edge that
reads the shared slot while another binding writes it keeps the snapshot copy,
because the disjointness check compares the renamed slots.

An owned record passed on unchanged reaches the edge through a
`StructuralLeafCopy` into a fresh slot, so its edge passes that slot rather
than a parameter. Before sharing, `dissolve` moves the copy's result into the
copied parameter's slot when neither is written: only the copy writes the
result, only the copy and edges take its address, and the parameter passes
`unwritten`. The copy is defined where the parameter is in scope, so the same
dominance argument holds. The copy's loads and stores then copy a slot onto
itself; `drop_copies` removes them once edge transfers are prepared, with the
registers they leave unused.
*/

use crate::instruction_selection::selection::construction::scalar_graph::structural::OwnedCopy;
use crate::instruction_selection::selection::edge_transfers::EdgeWork;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedAddressBase, SelectedFunction,
    SelectedInstructionKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedStructuralTransport, VirtualRegisterId, VirtualRegisterOrigin, terminator_instruction,
    terminator_successors, terminator_successors_mut,
};
use semantic_vocabulary::{BlockId, OperationId, PlaceId};
use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

/// Renames each qualifying block parameter's slot to the slot of the parameter
/// that every incoming edge passes to it, after dissolving whole copies of
/// owned block parameters that nothing writes.
pub(crate) fn share(
    function: &mut SelectedFunction,
    edge_work: &[EdgeWork],
    owned_copies: &[OwnedCopy],
) -> Dissolved {
    let dissolved = dissolve(function, owned_copies);
    let mut shared: Vec<(LocalStorageSlotId, LocalStorageSlotId)> = Vec::new();
    for slot in &function.local_storage_slots {
        let LocalStorageSlotId::StructuralBlockParameter { block, place } = slot.id else {
            continue;
        };
        if let Some(source) = sole_source(
            function,
            edge_work,
            block,
            place,
            slot.byte_size,
            &dissolved.operations,
        ) {
            shared.push((slot.id, source));
        }
    }
    // A source that is itself renamed would leave two hops; keep the first.
    let renamed: Vec<_> = shared.iter().map(|(from, _)| *from).collect();
    shared.retain(|(_, to)| !renamed.contains(to));
    rename(function, &shared);
    dissolved
}

/// Copies whose result now lives in their source's slot. Their loads and
/// stores copy the slot onto itself until `drop_copies` removes them.
#[derive(Default)]
pub(crate) struct Dissolved {
    operations: Vec<OperationId>,
    instructions: Vec<std::ops::Range<u32>>,
}

/// Removes the self-copies of dissolved copies, then the registers and
/// instruction numbers they leave unused.
pub(crate) fn drop_copies(function: &mut SelectedFunction, dissolved: Dissolved) {
    if dissolved.instructions.is_empty() {
        return;
    }
    let gone = |id: u32| {
        dissolved
            .instructions
            .iter()
            .any(|range| range.contains(&id))
    };
    std::sync::Arc::make_mut(&mut function.contracts)
        .memory_accesses
        .retain(|access| !gone(access.instruction.0));
    for block in function.blocks.iter_mut() {
        block
            .instructions
            .retain(|instruction| !gone(instruction.id.0));
    }
    crate::instruction_selection::selection_vector_promotion::prune_dead_metadata(function);
    crate::instruction_selection::selection_inlining::renumber_function(function);
}

/// Dissolves each owned copy whose source parameter and result are never
/// written: the result moves into the parameter's slot, and its loads and
/// stores are left for `drop_copies`.
fn dissolve(function: &mut SelectedFunction, copies: &[OwnedCopy]) -> Dissolved {
    let mut renames: Vec<(LocalStorageSlotId, LocalStorageSlotId)> = Vec::new();
    let mut dissolved = Dissolved::default();
    for copy in copies {
        let Some(
            from @ LocalStorageSlotId::StructuralBlockParameter {
                block,
                place: from_place,
            },
        ) = whole_slot(function, copy.source)
        else {
            continue;
        };
        let owned = declaration(function, block, from_place)
            .is_some_and(|row| row.access == StructuralAccess::Owned);
        let size = |id: LocalStorageSlotId| {
            function
                .local_storage_slots
                .iter()
                .find(|slot| slot.id == id)
                .map(|slot| slot.byte_size)
        };
        if !owned
            || size(from).is_none()
            || size(from) != size(copy.slot)
            || !copying_only(function, copy)
            || !copy_unwritten(function, copy)
            || !unwritten(
                function,
                from,
                from_place,
                StructuralAccess::Owned,
                &dissolved.operations,
            )
        {
            continue;
        }
        renames.push((copy.slot, from));
        dissolved.instructions.push(copy.instructions.clone());
        dissolved.operations.push(copy.operation);
    }
    rename(function, &renames);
    dissolved
}

/// The copy's instructions are the loads and stores of its bytes.
fn copying_only(function: &SelectedFunction, copy: &OwnedCopy) -> bool {
    !copy.instructions.is_empty()
        && function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .filter(|row| copy.instructions.contains(&row.id.0))
            .all(|row| {
                matches!(
                    row.kind,
                    SelectedInstructionKind::Load64 { .. }
                        | SelectedInstructionKind::Load32 { .. }
                        | SelectedInstructionKind::Load16 { .. }
                        | SelectedInstructionKind::Load8 { .. }
                        | SelectedInstructionKind::Store { .. }
                        | SelectedInstructionKind::AddressOffset { .. }
                )
            })
        && copy.instructions.clone().all(|id| {
            function
                .blocks
                .iter()
                .any(|block| block.instructions.iter().any(|row| row.id.0 == id))
        })
}

/// Only the copy writes its slot or place, and only the copy and edges take
/// the slot's address.
fn copy_unwritten(function: &SelectedFunction, copy: &OwnedCopy) -> bool {
    let slot = copy.slot;
    let own = SelectedMemoryAccessOrigin::Operation(copy.operation);
    let stored = function.instructions_and_terminators().any(|row| {
        matches!(row.kind,
            SelectedInstructionKind::Store64 { slot: FrameStorageSlotId::Local(local), .. }
                if local == slot)
    });
    let dispatched = function.blocks.iter().any(|row| {
        terminator_successors(&row.terminator)
            .iter()
            .any(|successor| {
                successor.structural_case.as_ref().is_some_and(|case| {
                    matches!(case.source,
                    selected_instructions::SelectedCaseDispatchSource::Local { slot: local }
                        if local == slot)
                })
            })
    });
    !stored
        && !dispatched
        && function
            .memory_accesses
            .iter()
            .all(|access| match access.role {
                SelectedMemoryAccessRole::AddressLocal { slot: local }
                | SelectedMemoryAccessRole::ReadLocal { slot: local }
                    if local == slot =>
                {
                    access.origin == own
                        || matches!(access.origin, SelectedMemoryAccessOrigin::Edge(_))
                }
                SelectedMemoryAccessRole::WriteLocal { slot: local } => local != slot,
                SelectedMemoryAccessRole::WritePlace
                | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
                | SelectedMemoryAccessRole::WriteIndexedStructural { .. }
                | SelectedMemoryAccessRole::WriteByteSpan { .. }
                | SelectedMemoryAccessRole::WriteByteSequence { .. }
                | SelectedMemoryAccessRole::WriteElementView { .. }
                | SelectedMemoryAccessRole::WriteBacking
                | SelectedMemoryAccessRole::WriteBackingSpan => {
                    access.place != copy.place || copy.instructions.contains(&access.instruction.0)
                }
                _ => true,
            })
}

fn rename(function: &mut SelectedFunction, shared: &[(LocalStorageSlotId, LocalStorageSlotId)]) {
    if shared.is_empty() {
        return;
    }
    let map = |slot: &mut LocalStorageSlotId| {
        if let Some((_, to)) = shared.iter().find(|(from, _)| from == slot) {
            *slot = *to;
        }
    };
    let contracts = std::sync::Arc::make_mut(&mut function.contracts);
    contracts
        .local_storage_slots
        .retain(|slot| !shared.iter().any(|(from, _)| *from == slot.id));
    for access in &mut contracts.memory_accesses {
        if let SelectedMemoryAccessRole::ReadLocal { slot }
        | SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } = &mut access.role
        {
            map(slot);
        }
    }
    for block in function.blocks.iter_mut() {
        for instruction in block.instructions.iter_mut() {
            if let SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(local),
                ..
            }
            | SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(local),
                ..
            } = &mut instruction.kind
            {
                map(local);
            }
        }
        for successor in terminator_successors_mut(&mut block.terminator) {
            for binding in &mut successor.structural_bindings {
                match &mut binding.transport {
                    SelectedStructuralTransport::Address { destination, .. }
                    | SelectedStructuralTransport::WholeValue { destination, .. }
                    | SelectedStructuralTransport::ScalarReference { destination, .. } => {
                        map(destination)
                    }
                    SelectedStructuralTransport::Unused => {}
                }
            }
        }
    }
}

/// The slot of the one parameter every edge into `block` passes whole as
/// `place`, when `place` may share it.
fn sole_source(
    function: &SelectedFunction,
    edge_work: &[EdgeWork],
    block: BlockId,
    place: PlaceId,
    byte_size: u32,
    dissolved: &[OperationId],
) -> Option<LocalStorageSlotId> {
    let destination = LocalStorageSlotId::StructuralBlockParameter { block, place };
    let target = declaration(function, block, place)?;
    let mut source: Option<(BlockId, PlaceId)> = None;
    let mut incoming = 0usize;
    for row in function.blocks.iter() {
        for successor in terminator_successors(&row.terminator) {
            if successor.structural_case.as_ref().is_some_and(|case| {
                matches!(case.source,
                    selected_instructions::SelectedCaseDispatchSource::Local { slot }
                        if slot == destination)
            }) {
                return None;
            }
            for binding in &successor.structural_bindings {
                let writes_here = match binding.transport {
                    SelectedStructuralTransport::Address { destination: d, .. }
                    | SelectedStructuralTransport::WholeValue { destination: d, .. }
                    | SelectedStructuralTransport::ScalarReference { destination: d, .. } => {
                        d == destination
                    }
                    SelectedStructuralTransport::Unused => false,
                };
                if !writes_here {
                    continue;
                }
                let (pointer, byte_count) = match binding.transport {
                    SelectedStructuralTransport::WholeValue {
                        argument,
                        source_byte_offset: 0,
                        byte_size,
                        ..
                    } => (argument, byte_size),
                    SelectedStructuralTransport::Address {
                        base: SelectedAddressBase::Register(pointer),
                        byte_offset: 0,
                        byte_count,
                        ..
                    } => (pointer, byte_count),
                    _ => return None,
                };
                if byte_count != byte_size || !binding.source_indices.is_empty() {
                    return None;
                }
                let edge = successor.psi_edge?;
                if edge_work.iter().any(|work| {
                    work.edge == edge && (!work.preparation.is_empty() || !work.cleanup.is_empty())
                }) {
                    return None;
                }
                let LocalStorageSlotId::StructuralBlockParameter {
                    block: from_block,
                    place: from_place,
                } = whole_slot(function, pointer)?
                else {
                    return None;
                };
                if from_block == block
                    || source.is_some_and(|known| known != (from_block, from_place))
                {
                    return None;
                }
                source = Some((from_block, from_place));
                incoming += 1;
            }
        }
    }
    let (from_block, from_place) = source?;
    if incoming == 0 {
        return None;
    }
    let origin = declaration(function, from_block, from_place)?;
    if origin.structural_type != target.structural_type || origin.access != target.access {
        return None;
    }
    let from = LocalStorageSlotId::StructuralBlockParameter {
        block: from_block,
        place: from_place,
    };
    let same_size = function
        .local_storage_slots
        .iter()
        .find(|slot| slot.id == from)
        .is_some_and(|slot| slot.byte_size == byte_size);
    (same_size
        && unwritten(function, destination, place, target.access, dissolved)
        && unwritten(function, from, from_place, origin.access, dissolved))
    .then_some(from)
}

fn declaration(
    function: &SelectedFunction,
    block: BlockId,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let (_, rows) = function
        .structural_block_parameters
        .iter()
        .find(|(owner, _)| *owner == block)?;
    rows.iter().find(|row| row.place == place)
}

/// The local slot `pointer` addresses at offset 0, through the `FrameAddress`
/// that defines it.
fn whole_slot(
    function: &SelectedFunction,
    pointer: VirtualRegisterId,
) -> Option<LocalStorageSlotId> {
    let (VirtualRegisterOrigin::InstructionResult { instruction, .. }
    | VirtualRegisterOrigin::AbiTransport { instruction, .. }) =
        function.virtual_registers.get(pointer.0 as usize)?.origin
    else {
        return None;
    };
    let defining = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(&block.terminator)))
        })
        .find(|row| row.id == instruction)?;
    let SelectedInstructionKind::FrameAddress {
        slot: FrameStorageSlotId::Local(slot),
        byte_offset: 0,
    } = defining.kind
    else {
        return None;
    };
    Some(slot)
}

/// Only the parameter's own block entry, or a dissolved copy now living in
/// the slot, takes the slot's address, nothing stores to it directly, and an
/// owned value's place is never written.
fn unwritten(
    function: &SelectedFunction,
    slot: LocalStorageSlotId,
    place: PlaceId,
    access: StructuralAccess,
    dissolved: &[OperationId],
) -> bool {
    let LocalStorageSlotId::StructuralBlockParameter { block, .. } = slot else {
        return false;
    };
    let descriptor = matches!(
        access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    );
    let stored = function.instructions_and_terminators().any(|row| {
        matches!(row.kind,
            SelectedInstructionKind::Store64 { slot: FrameStorageSlotId::Local(local), .. }
                if local == slot)
    });
    !stored
        && function
            .memory_accesses
            .iter()
            .all(|access| match access.role {
                SelectedMemoryAccessRole::AddressLocal { slot: local }
                | SelectedMemoryAccessRole::ReadLocal { slot: local }
                    if local == slot =>
                {
                    access.origin == SelectedMemoryAccessOrigin::Block(block)
                        || matches!(access.origin,
                            SelectedMemoryAccessOrigin::Operation(operation)
                                if dissolved.contains(&operation))
                }
                SelectedMemoryAccessRole::WriteLocal { slot: local } => local != slot,
                SelectedMemoryAccessRole::WritePlace
                | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
                | SelectedMemoryAccessRole::WriteIndexedStructural { .. }
                | SelectedMemoryAccessRole::WriteByteSpan { .. }
                | SelectedMemoryAccessRole::WriteByteSequence { .. }
                | SelectedMemoryAccessRole::WriteElementView { .. }
                | SelectedMemoryAccessRole::WriteBacking
                | SelectedMemoryAccessRole::WriteBackingSpan => descriptor || access.place != place,
                _ => true,
            })
}
