//! Shared admission for store mutation motion: locate the named store,
//! prove its single exact write row and operand surface, then walk forward
//! to the latest position that keeps the write ordered before every access
//! that could observe it.
//!
//! The moved store writes the place's storage by one of the routes the other
//! memory rewrites admit: the exact-width `Store` through the referent
//! pointer carrying `WritePlace`, the `StorePacked` an odd fragment width
//! selects through the same pointer routes, or a write into the place's own
//! local storage carrying `WriteLocal` — a `Store` or `StorePacked` through
//! the slot's materialized address, or the always-eight-byte `Store64` into
//! that slot directly — or the byte-sequence store: a `Store { 0, 1 }`
//! through a fully computed view address carrying one `WriteByteSequence`
//! row, whose written byte sits at the row's `byte_offset + index` for the
//! runtime `index` — unless that `index` resolves to a clean
//! `MaterializeI64`, when `admit` collapses the moved extent to the one
//! byte `byte_offset + index` before the walk. The place's own storage is
//! its
//! `StructuralParameter`/`StructuralBlockParameter` slot or the producing
//! operation's `Structural` home — the place's declaration names that
//! producer. A `Structural` slot the declaration does not charge to that
//! operation only stages bytes that name the place (a call's staged view
//! descriptor) under its own slot coordinates, so its write is not a place
//! write: it is the moved store of the slot's own bytes instead. Staged
//! bytes sit under slot coordinates no place-named row reaches — reads of
//! them would have no honest roster role, and the consuming operation sits
//! behind the call barrier — so only a row naming that very slot can observe
//! or rewrite them, and the staging store sinks under the same walk bounded
//! by the first access on the slot's own range.
//!
//! The packed form's early-clobber scratch `Def` and declared clobbers move
//! with the instruction, so custody is proven as the write side of the same
//! register/condition-state coupling the walk already enforces for the
//! store's reads: any use of the scratch inside the window would read the
//! moved definition early, any second write would be overtaken by it, and a
//! flag read or write would cross the moved flag clobber — each bounds the
//! motion at the position that mentions it.
//!
//! Sinking the store delays the write inside the window where the delay is
//! unobservable. The scan stops before the first position that must stay
//! ordered after the store: a roster row touching the moved byte range — an
//! exact row intersecting it, or a dynamic-extent row whose fixed offset
//! still starts below the moved end, its reach being unbounded upward —
//! while a byte-sequence row whose `index` resolves to a clean
//! `MaterializeI64` touches exactly the byte `byte_offset + index`, so it
//! stops the slide only by landing inside the moved extent and a landing
//! anywhere off it walks past. Then a
//! call or hosted effect, a redefinition of a
//! carried register, a memory-capable instruction with no roster row, or a
//! boundary settlement. The store lands immediately before that position;
//! every kept event still observes the write in the same relative order.
//! When the moved store is itself a byte-sequence write the reach direction
//! mirrors: the written byte can sit anywhere at or past the row's fixed
//! offset, so a later exact or local row still reaches it once its own
//! extent ends past that offset, and a later dynamic-extent row on the
//! place always meets it — unless the moved `index` resolved the same way,
//! collapsing the extent to that one byte before the walk, so a resolved
//! row's landing decides and an unresolved row still reaches it only from
//! a payload base at or below it.
//!
//! The walk is not confined to one block: reaching a block's end without a
//! stop continues through its terminator's successor edges when every edge
//! names one block, that block's only predecessor is the crossed block, and
//! the successor cannot reach any block already walked. A fork crosses too
//! when its edges reconverge — each lands on one join directly or enters a
//! region fed only through the fork's own edges: a block joins the region
//! once every predecessor leads from the crossed block or the region
//! itself, so entry happens at the fork and the region stays acyclic, every
//! region block carries at least one successor, and every edge leaving the
//! region lands on that join — a chain or a nested reconvergent shape
//! inside one arm crosses under the same count. The join's only
//! predecessor blocks are the crossed block and the region, and every
//! region block stays walkable end to end — so every traversal of the
//! crossed block reaches the join exactly once and the write runs once
//! there as it ran once here: no path gains or drops it. A fork whose
//! targets never reconverge would drop the write from the paths that leave
//! it; a join or a region block fed from outside the region would add the
//! write to paths that never carried it; and a successor that can return
//! to the walked chain
//! would close a cycle in
//! which an access before the store's original position now runs after it —
//! the walked interval only proves the forward half of that order. Each
//! crossed terminator's roster rows and register definitions decide before
//! the edge's transports, which may not redefine the carried registers or
//! write the moved place's storage. Every failed crossing lands the store at
//! the crossed block's end instead of rejecting: an earlier landing on the
//! proven path is still a real motion.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    FrameStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstructionId,
    SelectedInstructionKind, SelectedMemoryAccessRole,
};

use super::StoreMutationMotionError;
use super::motion::{
    Moved, arm_stops, carried_surface, edge_stops, interferes, instruction_stops, packed_store_shape,
    place_store_shape, reaches_visited, reconvergent_join, settlement_before, sole_predecessor,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, local_slot_is_place_storage, local_store_shape, staging_slot,
    structural_place_declarations,
};
use crate::rewrites::window_hazards::coupled;

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
    // the slot it writes. `StorePacked` carries its range on the kind like
    // the plain store; its extra early-clobber scratch `Def` and declared
    // clobbers move with the instruction, so the window walk below must
    // prove their custody — no read or second write of either inside the
    // walked span.
    let (encoded_offset, encoded_size, direct_slot, packed) = match moved_store.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(StoreMutationMotionError::UnsupportedInstruction);
            }
            (byte_offset, u32::from(byte_size), None, false)
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            (byte_offset, u32::from(width.byte_size()), None, true)
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => (byte_offset, 8, Some(slot), false),
        _ => return Err(StoreMutationMotionError::UnsupportedInstruction),
    };
    // The write's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction encodes. The row's role must match
    // the route the instruction takes to its storage: `WritePlace`
    // for the referent-pointer store, or `WriteLocal` on a local
    // slot for the local-storage routes — the direct `Store64`'s row
    // naming the same slot the instruction encodes. The slot decides
    // which storage the row moves: the place's own storage when the
    // place's declaration charges the slot to the place — a parameter
    // home or the producing operation's `Structural` home — or the
    // staging slot's own bytes when it does not. The row names the store by
    // instruction identity, so the move retains the roster unchanged.
    let structural_places = structural_place_declarations(function);
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == store);
    let write = rows
        .next()
        .ok_or(StoreMutationMotionError::UnsupportedInstruction)?;
    // A byte-sequence store takes the dynamic route: `Store { 0, 1 }` writes
    // one byte through a fully computed view address, and its
    // `WriteByteSequence` row carries the payload base as `byte_offset` plus
    // the runtime `index` — so the row's offset is a lower bound the index
    // extends, not the encoded range, and the written byte's position is
    // decided at runtime.
    let sequence_index = match write.role {
        SelectedMemoryAccessRole::WriteByteSequence { index, .. } => Some(index),
        _ => None,
    };
    // The `WriteLocal` routes can also name a staging slot: when the slot is
    // not the row place's own storage it stages bytes that merely name the
    // place, and the write moves the slot's own bytes — the staging subject
    // `Moved::storage` records below. `staging_slot` still requires the
    // row's place to be the slot's staged place: a `WriteLocal` naming a
    // different place than the slot stages is no coherent row.
    let mut staging = None;
    let storage_route = match (write.role, direct_slot) {
        (SelectedMemoryAccessRole::WritePlace, None) => true,
        (SelectedMemoryAccessRole::WriteLocal { slot }, None) => {
            local_slot_is_place_storage(slot, write.place, structural_places) || {
                staging = staging_slot(slot, write.place);
                staging.is_some()
            }
        }
        (SelectedMemoryAccessRole::WriteLocal { slot }, Some(encoded)) => {
            slot == encoded
                && (local_slot_is_place_storage(slot, write.place, structural_places) || {
                    staging = staging_slot(slot, write.place);
                    staging.is_some()
                })
        }
        (SelectedMemoryAccessRole::WriteByteSequence { .. }, None) => {
            encoded_offset == 0 && encoded_size == 1 && write.byte_count == 1
        }
        _ => false,
    };
    if rows.next().is_some()
        || !storage_route
        || (sequence_index.is_none()
            && (write.byte_offset != encoded_offset || write.byte_count != encoded_size))
    {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    let mut moved = Moved {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
        sequence_index,
        storage: staging.map_or(SubjectStorage::Place, SubjectStorage::Staging),
    };
    // A byte-sequence moved store whose own `index` resolves through the
    // carrier audit — sole `InstructionResult` carrier, clean
    // `MaterializeI64` definition, no edge-transport or case-payload
    // redefinition — writes one fixed byte at `byte_offset + index`: the
    // moved extent collapses to that exact byte. Every check below then
    // decides on a fixed position — a row that cannot reach the moved byte
    // walks past — while an unresolved index, or a position no u32 names,
    // leaves the extent unbounded upward from `byte_offset`.
    if let Some(index) = moved.sequence_index
        && let Some(landed) = constant_index(function, index)
        && let Some(position) = u64::from(moved.byte_offset).checked_add(landed)
        && let Ok(position) = u32::try_from(position)
    {
        moved.byte_offset = position;
        moved.byte_count = 1;
        moved.sequence_index = None;
    }
    // The moved instruction must carry the operand surface its route
    // declares: the plain `[use pointer, use value]` place store — the
    // pointer being the referent pointer or the slot's materialized
    // address — the packed store's `[use pointer, use packed value, def
    // scratch]` row, or the direct slot store's single `[use value]` row. An
    // exotic surface would make the motion contract unclear.
    if packed {
        packed_store_shape(moved_store, environment)?;
    } else if direct_slot.is_some() {
        local_store_shape(moved_store, environment, || {
            StoreMutationMotionError::ConstraintMismatch
        })?;
    } else {
        place_store_shape(moved_store, environment)?;
    }
    let carried = carried_surface(moved_store, packed)?;
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
            if instruction_stops(candidate, &moved, function, moved_store, structural_places)
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
        // edges, so a moved-place row or a hazard against the moved store's
        // reads or writes on it lands the store at the block's end before it
        // runs.
        let terminator = terminator_instruction(&current.terminator);
        if function.memory_accesses.iter().any(|access| {
            access.instruction == terminator.id
                && interferes(&moved, access, structural_places, function)
        }) || coupled(moved_store, terminator)
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
        // Edges to distinct blocks fork: the write can still cross when every
        // edge lands on one join directly or enters a region fed only
        // through the fork's own edges whose edges all reach that join — a
        // single-block arm is the smallest such region, a chain or a nested
        // reconvergent shape a grown one — and the join's predecessors are
        // exactly the crossed block and the region. Every traversal of the
        // crossed block then reaches the join exactly once, so the write
        // runs once there as it ran once here — no path gains or drops it.
        // Any other shape lands the store at the fork's end.
        if edges.iter().any(|edge| edge.block != first.block) {
            // A dangling edge target is malformed source, as on the
            // linear path.
            if edges.iter().any(|edge| {
                !function
                    .blocks
                    .iter()
                    .any(|candidate| candidate.id == edge.block)
            }) {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
            let Some((join, region)) = reconvergent_join(function, cursor, &edges, &visited) else {
                break land_at_end(if cursor == block_index { end - 1 } else { end });
            };
            // Every region block must stay walkable end to end: a stop
            // inside the region could only land the store inside it,
            // dropping the write on the paths the other blocks carry — so
            // one blocked block bounds the motion at the forked block's end,
            // not inside the region.
            if edges.iter().any(|edge| edge_stops(edge, &moved, &carried))
                || region.iter().any(|&arm| {
                    arm_stops(
                        function,
                        &function.blocks[arm],
                        &moved,
                        &carried,
                        moved_store,
                        structural_places,
                    )
                })
            {
                break land_at_end(if cursor == block_index { end - 1 } else { end });
            }
            for &arm in &region {
                visited[arm] = true;
                interval = interval
                    .checked_add(function.blocks[arm].instructions.len())
                    .and_then(|total| {
                        total.checked_add(
                            terminator_successors(&function.blocks[arm].terminator).len(),
                        )
                    })
                    .and_then(|total| total.checked_add(1))
                    .ok_or(StoreMutationMotionError::IdentityOverflow)?;
            }
            interval = interval
                .checked_add(edges.len())
                .and_then(|total| total.checked_add(1))
                .ok_or(StoreMutationMotionError::IdentityOverflow)?;
            cursor = join;
            start = 0;
            continue;
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
