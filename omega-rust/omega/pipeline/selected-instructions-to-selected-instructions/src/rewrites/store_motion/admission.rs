//! Shared admission for store mutation motion: locate the named store,
//! prove its single exact write row and operand surface, then walk forward
//! to the latest position that keeps the write ordered before every access
//! that could observe it.
//!
//! The moved store writes the place's storage by one of the routes the other
//! memory rewrites admit: the exact-width `Store` through the referent
//! pointer carrying `WritePlace`, or a write into the place's own local
//! storage carrying `WriteLocal` — a `Store` through the slot's materialized
//! address, or the always-eight-byte `Store64` into that slot directly. The
//! place's own storage is its `StructuralParameter`/`StructuralBlockParameter`
//! slot or the producing operation's `Structural` home — the place's
//! declaration names that producer. A `Structural` slot the declaration does
//! not charge to that operation only stages bytes that name the place (a
//! call's staged view descriptor) under its own slot coordinates, so its
//! write is not a place write and stays inadmissible as the moved store.
//! A `StorePacked` stays inadmissible too: moving it moves the early-clobber
//! scratch `Def`, whose custody the window walk does not prove.
//!
//! Sinking the store delays the write inside the window where the delay is
//! unobservable. The scan stops before the first position that must stay
//! ordered after the store: a roster row touching the moved byte range or the
//! place's dynamic storage, a call or hosted effect, a redefinition of a
//! carried register, a memory-capable instruction with no roster row, or a
//! boundary settlement. The store lands immediately before that position;
//! every kept event still observes the write in the same relative order.
//!
//! The walk is not confined to one block: reaching a block's end without a
//! stop continues through its terminator's successor edges when every edge
//! names one block, that block's only predecessor is the crossed block, and
//! the successor cannot reach any block already walked. A fork out of the
//! crossed block would drop the write from the paths that leave it; a join
//! into the successor would add the write to paths that never carried it; and
//! a successor that can return to the walked chain would close a cycle in
//! which an access before the store's original position now runs after it —
//! the walked interval only proves the forward half of that order. Each
//! crossed terminator's roster rows and register definitions decide before
//! the edge's transports, which may not redefine the carried registers or
//! write the moved place's storage. Every failed crossing lands the store at
//! the crossed block's end instead of rejecting: an earlier landing on the
//! proven path is still a real motion.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedCasePayloadTransport,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedMemoryAccess, SelectedMemoryAccessRole, SelectedStructuralTransport, SelectedSuccessor,
    SelectedValueTransport, VirtualRegisterId,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use super::StoreMutationMotionError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::place_storage::{local_slot_is_place_storage, structural_place_declarations};
use crate::rewrites::window_hazards::is_barrier;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub store_index: usize,
    /// The block the store lands in.
    pub target_block_index: usize,
    /// The moved store's index in the target block after the move: for a
    /// same-block move this is the stopped-before ordinal minus one (the
    /// removal shifts it left); for a crossed move it is the insertion index
    /// in the target's own vector.
    pub insert_index: usize,
}

/// One exact byte range within one place root.
struct Moved {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
}

impl Moved {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + u64::from(self.byte_count)
            && u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count)
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, StoreMutationMotionError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(StoreMutationMotionError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(StoreMutationMotionError::SourceMismatch)?;
    let (block_index, store_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == store)
                .map(|store_index| (block_index, store_index))
        })
        .ok_or(StoreMutationMotionError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let moved_store = &block.instructions[store_index];
    // The moved store's encoded byte range and, for the direct slot route,
    // the slot it writes. `StorePacked` stays unsupported: its early-clobber
    // scratch `Def` would move with the store, and this admission proves no
    // scratch custody across the window.
    let (encoded_offset, encoded_size, direct_slot) = match moved_store.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(StoreMutationMotionError::UnsupportedInstruction);
            }
            (byte_offset, u32::from(byte_size), None)
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => (byte_offset, 8, Some(slot)),
        _ => return Err(StoreMutationMotionError::UnsupportedInstruction),
    };
    // The write's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction encodes. The row's role must match
    // the route the instruction takes to the place's storage: `WritePlace`
    // for the referent-pointer store, or `WriteLocal` on the place's own
    // storage slot for the local-storage routes — the direct `Store64`'s row
    // naming the same slot the instruction encodes. An operation-owned
    // `Structural` slot is that storage only when the place's declaration
    // names the operation as its producer; a slot that merely stages bytes
    // naming the place never moves the place's bytes, so a store through it
    // is not the moved place write. The row names the store by instruction
    // identity, so the move retains the roster unchanged.
    let structural_places = structural_place_declarations(function);
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == store);
    let write = rows
        .next()
        .ok_or(StoreMutationMotionError::UnsupportedInstruction)?;
    let storage_route = match (write.role, direct_slot) {
        (SelectedMemoryAccessRole::WritePlace, None) => true,
        (SelectedMemoryAccessRole::WriteLocal { slot }, None) => {
            local_slot_is_place_storage(slot, write.place, structural_places)
        }
        (SelectedMemoryAccessRole::WriteLocal { slot }, Some(encoded)) => {
            slot == encoded && local_slot_is_place_storage(slot, write.place, structural_places)
        }
        _ => false,
    };
    if rows.next().is_some()
        || !storage_route
        || write.byte_offset != encoded_offset
        || write.byte_count != encoded_size
    {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    let moved = Moved {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
    };
    // The moved instruction must carry the operand surface its route
    // declares: the plain `[use pointer, use value]` place store — the
    // pointer being the referent pointer or the slot's materialized
    // address — or the direct slot store's single `[use value]` row. An
    // exotic surface would make the motion contract unclear.
    if direct_slot.is_some() {
        local_store_shape(moved_store, environment)?;
    } else {
        place_store_shape(moved_store, environment)?;
    }
    let carried = carried_surface(moved_store)?;
    // Walk forward to the latest provable position. Scanning a block stops
    // before the first instruction or settlement position that must stay
    // ordered after the store; reaching a block's end cleanly crosses only a
    // unique-successor, unique-predecessor, acyclic edge.
    let mut visited = vec![false; function.blocks.len()];
    let mut interval = 0usize;
    let mut cursor = block_index;
    let mut start = store_index + 1;
    let (target_block_index, insert_index) = loop {
        visited[cursor] = true;
        let current = &function.blocks[cursor];
        let mut stop = None;
        for (index, candidate) in current.instructions.iter().enumerate().skip(start) {
            if instruction_stops(candidate, &moved, function, &carried, structural_places)
                || settlement_before(function, current.id, index)
            {
                stop = Some(index);
                break;
            }
        }
        if let Some(index) = stop {
            interval = interval
                .checked_add(index + 1 - start)
                .ok_or(StoreMutationMotionError::IdentityOverflow)?;
            let insert = if cursor == block_index {
                index - 1
            } else {
                index
            };
            break (cursor, insert);
        }
        interval = interval
            .checked_add(current.instructions.len() - start)
            .ok_or(StoreMutationMotionError::IdentityOverflow)?;
        // The after-body settlement slot sits between the last instruction
        // and the terminator; a settlement there stops the walk at the end.
        let end = current.instructions.len();
        if settlement_before(function, current.id, end) {
            interval = interval
                .checked_add(1)
                .ok_or(StoreMutationMotionError::IdentityOverflow)?;
            let insert = if cursor == block_index { end - 1 } else { end };
            break (cursor, insert);
        }
        // The terminator instruction sits between the body and the crossed
        // edges, so a moved-place row or a carried-register definition on it
        // lands the store at the block's end before it runs.
        let terminator = terminator_instruction(&current.terminator);
        if function.memory_accesses.iter().any(|access| {
            access.instruction == terminator.id && interferes(&moved, access, structural_places)
        }) || defines_carried(terminator, &carried)
        {
            let insert = if cursor == block_index { end - 1 } else { end };
            break (cursor, insert);
        }
        let edges = terminator_successors(&current.terminator);
        let land_at_end = |insert: usize| (cursor, insert);
        let Some(first) = edges.first() else {
            // A terminator without successors leaves no later position; the
            // write still runs, just at the block's end.
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        };
        // Every path forward must reach one block: edges to distinct blocks
        // admit a path the moved write never runs on.
        if edges.iter().any(|edge| edge.block != first.block) {
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        }
        let Some(next) = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == first.block)
        else {
            return Err(StoreMutationMotionError::SourceMismatch);
        };
        // The successor must see the crossed block as its only predecessor —
        // a join adds the write to paths that never carried it — and must not
        // reach any walked block: a return path would close a cycle whose
        // unverified interval could reorder an access across the store.
        if !sole_predecessor(function, next, cursor)
            || visited[next]
            || reaches_visited(function, next, &visited)
        {
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        }
        if edges.iter().any(|edge| edge_stops(edge, &moved, &carried)) {
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        }
        interval = interval
            .checked_add(edges.len())
            .and_then(|total| total.checked_add(1))
            .ok_or(StoreMutationMotionError::IdentityOverflow)?;
        cursor = next;
        start = 0;
    };
    // A move must land somewhere later: a same-block landing at the store's
    // own index means the next position was already a stop.
    if target_block_index == block_index && insert_index == store_index {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(interval))
        .and_then(|total| total.checked_add(function.memory_accesses.len()))
        .ok_or(StoreMutationMotionError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| StoreMutationMotionError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(StoreMutationMotionError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        store_index,
        target_block_index,
        insert_index,
    })
}

/// The moved store's operand surface: the target's `[use pointer, use value]`
/// row with nothing implicit attached.
fn place_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), StoreMutationMotionError> {
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(StoreMutationMotionError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
        || row.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(StoreMutationMotionError::ConstraintMismatch);
    }
    Ok(())
}

/// The direct slot store's operand surface: the target's declared `store64`
/// row — exactly `[use value]` — and the instruction carrying just that one
/// use. The move defines nothing, so no custody check is needed.
fn local_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), StoreMutationMotionError> {
    if environment.selected_keys().store64 != Some(instruction.constraint) {
        return Err(StoreMutationMotionError::ConstraintMismatch);
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(StoreMutationMotionError::ConstraintMismatch)?;
    if row.operands.len() != 1
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || instruction.operands.len() != 1
        || instruction.operands[0].operand != 0
        || instruction.operands[0].access != RegisterOperandAccess::Use
    {
        return Err(StoreMutationMotionError::ConstraintMismatch);
    }
    Ok(())
}

/// The locations the move keeps identical: the virtual registers the store
/// reads — the referent pointer or materialized slot address plus the stored
/// value for a place store, or just the stored value for the direct
/// `Store64` — plus the physical units its implicit uses read, like the
/// frame base a `Store64` resolves its slot against. An implicit definition
/// or clobber on the moved store would itself write state inside the window,
/// so it stays inadmissible.
struct Carried {
    registers: Vec<VirtualRegisterId>,
    units: Vec<RegisterUnitId>,
}

/// Collect the moved store's read surface. Every operand must be the use its
/// position declares — a definition carried inside the window would change
/// what the moved store reads at its new position — and the instruction may
/// carry implicit uses as reads, but never implicit writes or clobbers.
fn carried_surface(instruction: &SelectedInstruction) -> Result<Carried, StoreMutationMotionError> {
    if !instruction.implicit_defs.is_empty() || !instruction.clobbers.is_empty() {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    let mut registers = Vec::with_capacity(instruction.operands.len());
    for (index, operand) in instruction.operands.iter().enumerate() {
        if operand.access != RegisterOperandAccess::Use || operand.operand != index as u16 {
            return Err(StoreMutationMotionError::UnsupportedPair);
        }
        registers.push(operand.virtual_register);
    }
    Ok(Carried {
        registers,
        units: instruction.implicit_uses.clone(),
    })
}

/// Whether one roster row touches the moved bytes or the place's dynamic
/// storage. Exact rows must intersect the moved range; dynamic extents
/// always reach it. A `WriteLocal` row names an exact range on a slot: when
/// the slot is the moved place's own storage, range intersection decides; a
/// slot that only stages bytes naming the place holds none of the place's
/// bytes at any offset, so its writes never stop the walk. A materialized
/// local address could reach the same storage by a route the roster does
/// not bound, so it stops the walk when its slot is the place's storage; a
/// staged slot's address reaches only the staged bytes. Outgoing-area
/// storage never aliases a referent place.
fn interferes(
    moved: &Moved,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            access.place == moved.place && moved.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. } => access.place == moved.place,
        SelectedMemoryAccessRole::WriteLocal { slot } => {
            local_slot_is_place_storage(slot, moved.place, structural_places)
                && moved.intersects(access)
        }
        SelectedMemoryAccessRole::AddressLocal { slot } => {
            local_slot_is_place_storage(slot, moved.place, structural_places)
        }
        SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// Whether sliding the store past this instruction would change the observed
/// order of the write. Any of the stop conditions bound the window.
fn instruction_stops(
    candidate: &SelectedInstruction,
    moved: &Moved,
    function: &SelectedFunction,
    carried: &Carried,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    let mut has_row = false;
    for access in function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == candidate.id)
    {
        has_row = true;
        if interferes(moved, access, structural_places) {
            return true;
        }
    }
    is_barrier(candidate)
        || defines_carried(candidate, carried)
        || (!has_row && unaccounted_kind(candidate))
}

/// An interval instruction without a roster row must be unable to reach any
/// semantic or place-backed storage: private-slot frame accesses touch
/// compiler-owned spill/boundary slots that no referent place aliases, and
/// pure register work has no memory side at all. Any other memory-capable
/// kind without a row is an unaccounted access.
fn unaccounted_kind(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    !matches!(
        instruction.kind,
        Store64 {
            slot: FrameStorageSlotId::Local(
                LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
            ),
            ..
        } | Load8 { .. }
            | Load16 { .. }
            | Load32 { .. }
            | Load64 { .. }
            | Load8Indexed
            | LoadPacked { .. }
            | FrameAddress { .. }
            | AddressOffset { .. }
            | ByteViewAddress
            | CopyI64
            | MaterializeI64 { .. }
            | CompareI64
            | CompareI64Zero
            | CompareI64Immediate { .. }
            | ExactAddI64 { .. }
            | ExactSubtractI64 { .. }
            | ExactAddI64Immediate { .. }
            | ExactSubtractI64Immediate { .. }
            | WrappingAddI64
            | ExactDivideU64 { .. }
            | WrappingRemainderI64 { .. }
            | SaturatingAdd { .. }
            | SaturatingSubtract { .. }
            | SaturatingDivide { .. }
            | BitwiseAndI64
            | BitwiseXorI64
            | ZeroExtendU8
            | ZeroExtendU16
            | ZeroExtendU32
            | SignExtendI8
            | SignExtendI16
            | SignExtendI32
            | Float32ToBits
            | Float64ToBits
            | BitsToFloat32
            | BitsToFloat64
            | MaterializeBooleanEqual
            | MaterializeBooleanU64LessThan
            | MaterializeBooleanI64LessThan
            | MaterializeBooleanU64LessOrEqual
            | MaterializeBooleanI64LessOrEqual
    )
}

/// A write to anything the moved store reads anywhere in the interval would
/// change what it observes at its new position: a definition of a carried
/// register, or an implicit definition or clobber of a carried unit — the
/// same read/write meeting the sibling walks enforce between paired
/// instructions.
fn defines_carried(instruction: &SelectedInstruction, carried: &Carried) -> bool {
    instruction.operands.iter().any(|operand| {
        operand.access != RegisterOperandAccess::Use
            && carried.registers.contains(&operand.virtual_register)
    }) || instruction
        .implicit_defs
        .iter()
        .chain(instruction.clobbers.iter())
        .any(|unit| carried.units.contains(unit))
}

/// A boundary settlement at a position in the crossed interval would observe
/// the place before the moved write instead of after it.
fn settlement_before(function: &SelectedFunction, block: SelectedBlockId, position: usize) -> bool {
    function.boundary_settlements.iter().any(|settlement| {
        settlement.block == block && settlement.instruction_index as usize == position
    })
}

/// Whether `block`'s only predecessor block is `expected`: every other
/// block's terminator is scanned for an edge naming it, so a second incoming
/// edge or a self-loop keeps the join visible.
fn sole_predecessor(function: &SelectedFunction, block: usize, expected: usize) -> bool {
    let mut predecessors = function
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            terminator_successors(&candidate.terminator)
                .iter()
                .any(|edge| edge.block == function.blocks[block].id)
                .then_some(index)
        });
    predecessors.next() == Some(expected) && predecessors.next().is_none()
}

/// Whether `start` can reach any block the walk already crossed. A return
/// path closes a cycle containing both the store's old and new positions;
/// the unverified half of that cycle could reorder a same-place access
/// across the moved write.
fn reaches_visited(function: &SelectedFunction, start: usize, visited: &[bool]) -> bool {
    let mut seen = vec![false; function.blocks.len()];
    let mut pending = vec![start];
    while let Some(index) = pending.pop() {
        if visited[index] {
            return true;
        }
        if seen[index] {
            continue;
        }
        seen[index] = true;
        for edge in terminator_successors(&function.blocks[index].terminator) {
            if let Some(target) = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == edge.block)
            {
                pending.push(target);
            }
        }
    }
    false
}

/// A crossed edge must not move the carried registers or touch the moved
/// place's storage. Register transports and case payload parameters name the
/// one register the edge defines; structural destinations, the case custody
/// slot, and custody discards write or retire place storage on the edge —
/// the same conservative structural-place test the sibling memory walks keep
/// on edges, since an edge transport's destination role is not decided here.
fn edge_stops(successor: &SelectedSuccessor, moved: &Moved, carried: &Carried) -> bool {
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers { parameter, .. } = binding.transport
            && carried.registers.contains(&parameter)
        {
            return true;
        }
    }
    for binding in &successor.structural_bindings {
        let destination = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue { destination, .. }
            | SelectedStructuralTransport::Descriptor { destination, .. } => destination,
        };
        if destination.structural_place() == Some(moved.place) {
            return true;
        }
    }
    if let Some(case) = &successor.structural_case {
        if case.slot.structural_place() == Some(moved.place)
            || case.trivial_affine_discards.contains(&moved.place)
        {
            return true;
        }
        for payload in &case.payloads {
            let parameter = match payload.transport {
                SelectedCasePayloadTransport::Unused => continue,
                SelectedCasePayloadTransport::Unmaterialized { parameter }
                | SelectedCasePayloadTransport::Registers { parameter, .. } => parameter,
            };
            if carried.registers.contains(&parameter) {
                return true;
            }
        }
    }
    false
}

/// The function's boundary settlements after the move: positions at or after
/// the landing index in a crossed target block shift one ordinal later to
/// stay before the same instruction; every other settlement — and every
/// settlement in a same-block move — keeps its position. Shared by proposal
/// and replay so both compute the same roster from the source, never from
/// each other. A settlement inside the crossed interval cannot exist after
/// admission; finding one means the position contract was broken.
pub(super) fn shifted_boundary_settlements(
    function: &SelectedFunction,
    source_block: SelectedBlockId,
    source_index: usize,
    target_block: SelectedBlockId,
    insert_index: usize,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, StoreMutationMotionError> {
    let source_body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == source_block)
        .ok_or(StoreMutationMotionError::SourceMismatch)?
        .instructions
        .len();
    let target_body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == target_block)
        .ok_or(StoreMutationMotionError::SourceMismatch)?
        .instructions
        .len();
    let same_block = source_block == target_block;
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        let position = settlement.instruction_index as usize;
        if settlement.block == source_block {
            if position > source_body {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
            // Positions at or before the store keep their ordinal; positions
            // inside the crossed interval cannot exist; positions past the
            // landing already name instructions that stay put.
            if position > source_index && (!same_block || position <= insert_index) {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
        } else if settlement.block == target_block {
            if position > target_body {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
            if position >= insert_index {
                settlement.instruction_index = u32::try_from(position + 1)
                    .map_err(|_| StoreMutationMotionError::IdentityOverflow)?;
            } else {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
        }
    }
    Ok(shifted)
}
