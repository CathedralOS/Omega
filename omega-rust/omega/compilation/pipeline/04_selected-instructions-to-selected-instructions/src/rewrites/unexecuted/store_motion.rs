//! Borrow-aware mutation motion on the selected CFG.
//!
//! A `Store { byte_offset, byte_size }` writes the low exact-width bits of
//! its value operand through the referent pointer — or through the
//! materialized address of the place's own local storage, its
//! `StructuralParameter`/`StructuralBlockParameter` slot or the producing
//! operation's `Structural` home, which a `Store64` also writes directly. A
//! `StorePacked { byte_offset, width }` takes the same pointer routes for
//! the odd fragment widths the plain store cannot encode. A `Store { 0, 1 }`
//! through a fully computed view address carries a `WriteByteSequence` row
//! instead: its written byte sits at the row's `byte_offset + index` for the
//! runtime `index`, so the moved extent is unbounded upward from that fixed
//! offset — unless the `index` resolves to a clean `MaterializeI64`, when
//! admission collapses the extent to the one byte `byte_offset + index`
//! before the walk.
//! A `Structural` slot the place's declaration does not charge to the
//! slot's operation only stages bytes that name the place under its own
//! slot coordinates: a store into it — the `Store`/`StorePacked` through
//! its materialized address or the direct `Store64` — is the moved store
//! of the slot's own bytes, and no place-named row can reach them. The
//! staging store sinks under the same walk, bounded by the first roster row
//! naming that very slot — a `WriteLocal` rewriting the staged bytes or an
//! `AddressLocal` materializing their address — rather than by the place's
//! own accesses.
//! Sinking the store later along the control-flow path defers the write
//! inside the window where nothing can observe the written bytes' old
//! contents: every instruction the store slides past must leave the
//! relative order of the write and every later access on the same storage
//! unchanged. The motion
//! lands the store immediately before the first position that must stay
//! ordered after it — the first roster access on the moved storage, a call or
//! hosted effect, a redefinition of a carried register, an unaccounted
//! memory-capable instruction, or a boundary settlement — or at the end of
//! the last block the walk can prove it still reaches on every path. The
//! packed form's early-clobber scratch `Def` and its declared clobbers move
//! with the instruction, so the same coupling that guards the carried reads
//! also proves their custody: a window instruction may not read or rewrite
//! the scratch, nor read or publish a condition-state unit the moved row
//! clobbers, and neither may a crossed edge transport or terminator.
//!
//! The alias decision is borrow-aware: it comes from the validated
//! `memory_accesses` roster, not from pointer-register equality. Each access
//! row names the semantic `PlaceId` and the exact byte range the instruction
//! touches. Two simultaneous accesses that can write cannot share one
//! referent under different place identities — exclusivity rejects
//! overlapping exclusive custody before selection — so rows for other places
//! and exact rows on disjoint ranges of the moved place cannot observe the
//! slide. A `WriteLocal` or `AddressLocal` row names a slot: the moved
//! bytes' storage — a parameter, block-parameter, or producer-declared
//! `Structural` home for a place subject, the staging slot itself for a
//! staging subject — interferes on them like a place row, while a slot
//! holding other bytes never reaches them. A dynamic-extent
//! row reaches only upward from its fixed byte offset — a span covers
//! `length` bytes there and a sequence row touches `offset + index` — so
//! one starting at or past the moved range's end is provably disjoint and
//! slides past like any disjoint row. A sequence row whose `index` resolves
//! to a clean `MaterializeI64` touches exactly the byte `offset + index`
//! instead, so it stops the slide only by landing inside the moved extent.
//! When the moved extent is itself
//! dynamic the direction mirrors: an exact or local row still reaches the
//! moved byte once its own extent ends past the row's fixed offset, and a
//! dynamic-extent row on the place always can — unless the moved `index`
//! resolved the same way, collapsing the extent to that one byte before the
//! walk. Only a
//! potentially overlapping access on the moved place, a dynamic-extent row
//! on it still able to reach the moved bytes, a write to or materialized
//! address of the place's own storage, or
//! a call/hosted effect bounds the window.
//!
//! The walk is not confined to one block: reaching a block's end without a
//! stop continues through its terminator's successor edges when every edge
//! names one block and that block's only predecessor is the crossed block.
//! A fork crosses too when its edges reconverge: each lands on one join
//! directly or enters a region fed only through the fork's own edges —
//! every region block fed only from the crossed block or the region
//! itself, successor-bearing, and exiting nowhere but the join — a chain
//! or a nested reconvergent arm under the same contract as the
//! single-block diamond. The join's only predecessor blocks are the
//! crossed block and the region, and every region block stays walkable
//! end to end — a stop inside the region could only land the store inside
//! it, dropping the write on the paths the other blocks carry. Every
//! traversal of the crossed block then reaches the join exactly once, so
//! the write runs once there
//! as it ran once here: no path gains or drops it. The
//! terminator's roster rows and register definitions decide before each
//! crossed edge's transports, which may not redefine the carried registers or
//! write the moved storage. Returns and hosted exits, forks whose targets
//! never reconverge on one fed join, joined targets fed from outside the
//! region, and re-entered blocks each bound the motion at the crossed
//! block's end.
//!
//! The store's roster row names the instruction by identity, not position, so
//! the access roster is retained unchanged. Boundary settlements are ordinal
//! positions: a settlement inside the crossed interval would change which
//! side of the write it observes, so the walk stops before it; settlements at
//! or after the landing position in the target block shift one ordinal later
//! when a crossed-edge move inserts there, while a same-block move leaves
//! every settlement position unchanged.
//!
//! Proposal and validation share only the contract: the validator never
//! calls the producer's admission. It re-derives the motion's legality from
//! the source records — the store's roster row, the operand surface, the
//! forward walk and its landing, the crossed-edge audits, and the
//! settlement remap — rebuilds the demanded function itself, requires the
//! proposed program to equal it, and restores the complete source by
//! content.

mod admission;
mod motion;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    FrameStorageSlotId, SelectedBlockId, SelectedBoundarySettlement, SelectedFunction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedMemoryAccessRole, terminator_instruction,
};
use semantic_vocabulary::FuelScheduleIdentity;

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{member_position, terminator_successors};
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, local_slot_is_place_storage, local_store_shape, staging_slot,
    structural_place_declarations,
};
use crate::rewrites::window_hazards::coupled;

use motion::{
    Moved, arm_stops, carried_surface, edge_stops, instruction_stops, interferes,
    packed_store_shape, place_store_shape, reaches_visited, reconvergent_join, settlement_before,
    sole_predecessor,
};

pub(crate) use validation::validate_store_mutation_motion;

/// An accepted store mutation motion with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedStoreMutationMotion {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: StoreMutationMotionReceipt,
}

validated_transformed_receipt!(ValidatedStoreMutationMotion, StoreMutationMotionReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoreMutationMotionReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(StoreMutationMotionReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreMutationMotionError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedPair,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for StoreMutationMotionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid store mutation motion: {self:?}")
    }
}

impl std::error::Error for StoreMutationMotionError {}

/// The validator's own reconstruction of the motion the contract permits:
/// the admitted store's coordinates, the landing position its own walk
/// proves, and the walked interval the family's measured-step contract
/// charges. It shares no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    block: SelectedBlockId,
    store_index: usize,
    /// The block the store lands in.
    target_block_index: usize,
    /// The moved store's index in the target block after the move: for a
    /// same-block move this is the stopped-before ordinal minus one (the
    /// removal shifts it left); for a crossed move it is the insertion
    /// index in the target's own vector.
    insert_index: usize,
    /// The walked positions the step contract charges: scanned body
    /// instructions including the stopping one, the after-body settlement
    /// slot when it bounds the slide, and each terminator and successor
    /// edge the walk crossed.
    interval: usize,
}

/// The validation work this audit performs, in the measured-step
/// contract the family publishes: the shared rewrite scan (one step per
/// block and instruction of the function, one per plan function), one per walked interval position and
/// crossed edge, and one per roster row.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, StoreMutationMotionError> {
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| total.checked_add(reconstructed.interval))
        .and_then(|total| total.checked_add(function.memory_accesses.len()))
        .ok_or(StoreMutationMotionError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| StoreMutationMotionError::IdentityOverflow)
}

/// The function's boundary settlements after the move — the validator's
/// own remap, not the producer's `admission` helper. Positions at or
/// after the landing index in a crossed target block shift one ordinal
/// later to stay before the same instruction; every other settlement —
/// and every settlement in a same-block move — keeps its position. A
/// settlement inside the crossed interval cannot exist after the walk
/// proved it empty; finding one means the position contract was broken.
fn moved_boundary_settlements(
    function: &SelectedFunction,
    source_block: SelectedBlockId,
    source_index: usize,
    target_block: SelectedBlockId,
    insert_index: usize,
) -> Result<Vec<SelectedBoundarySettlement>, StoreMutationMotionError> {
    let source_body = function
        .block(source_block)
        .ok_or(StoreMutationMotionError::SourceMismatch)?
        .instructions
        .len();
    let target_body = function
        .block(target_block)
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
            // Positions at or before the store keep their ordinal;
            // positions inside the crossed interval cannot exist;
            // positions past the landing already name instructions that
            // stay put.
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

/// Re-derive the motion's legality from the source records, without the
/// producer's `admission` routine: locate the named store, prove its
/// single write row and operand surface, then walk forward to the latest
/// position that keeps the write ordered before every access that could
/// observe it. A legality error surfaces here even when the proposal
/// matches the edit the producer emitted.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, StoreMutationMotionError> {
    let (_, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            StoreMutationMotionError::SourceMismatch
        })?;
    let (block_index, store_index) =
        member_position(function, store).ok_or(StoreMutationMotionError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let moved_store = &block.instructions[store_index];
    // The moved store's encoded byte range and, for the direct slot
    // route, the slot it writes — the validator's own decode of the
    // instruction kind against the produced grammar.
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
    // The write's semantic identity, re-decided by the validator: exactly
    // one roster row, one place root, and the same bytes the instruction
    // encodes. The row's role must match the route the instruction takes
    // to its storage: `WritePlace` for the referent-pointer store, or
    // `WriteLocal` on a local slot for the local-storage routes — the
    // direct `Store64`'s row naming the same slot the instruction
    // encodes. The slot decides which storage the row moves: the place's
    // own storage when the place's declaration charges the slot to the
    // place — a parameter home or the producing operation's `Structural`
    // home — or the staging slot's own bytes when it does not. The row
    // names the store by instruction identity, so the move retains the
    // roster unchanged.
    let structural_places = structural_place_declarations(function);
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == store);
    let write = rows
        .next()
        .ok_or(StoreMutationMotionError::UnsupportedInstruction)?;
    // A byte-sequence store takes the dynamic route: `Store { 0, 1 }`
    // writes one byte through a fully computed view address, and its
    // `WriteByteSequence` row carries the payload base as `byte_offset`
    // plus the runtime `index` — so the row's offset is a lower bound the
    // index extends, not the encoded range, and the written byte's
    // position is decided at runtime.
    let sequence_index = match write.role {
        SelectedMemoryAccessRole::WriteByteSequence { index, .. } => Some(index),
        _ => None,
    };
    // The `WriteLocal` routes can also name a staging slot: when the slot
    // is not the row place's own storage it stages bytes that merely name
    // the place, and the write moves the slot's own bytes — the staging
    // subject `Moved::storage` records below. `staging_slot` still
    // requires the row's place to be the slot's staged place: a
    // `WriteLocal` naming a different place than the slot stages is no
    // coherent row.
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
    // decides on a fixed position — a row that cannot reach the moved
    // byte walks past — while an unresolved index, or a position no u32
    // names, leaves the extent unbounded upward from `byte_offset`.
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
    // scratch]` row, or the direct slot store's single `[use value]` row.
    // An exotic surface would make the motion contract unclear.
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
    // The validator's own forward walk to the latest provable position.
    // Scanning a block stops before the first instruction or settlement
    // position that must stay ordered after the store; reaching a block's
    // end cleanly crosses a unique-successor, unique-predecessor, acyclic
    // edge, or a fork whose edges reconverge on one join the crossed block
    // and a region entered through the fork edges alone feed — every
    // region block fed only from inside, successor-bearing, and walkable
    // end to end — so every traversal then reaches the join exactly once
    // and no path gains or drops the write. Every failed crossing lands
    // the store at the crossed
    // block's end instead of rejecting: an earlier landing on the proven
    // path is still a real motion.
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
        // The after-body settlement slot sits between the last
        // instruction and the terminator; a settlement there stops the
        // walk at the end.
        let end = current.instructions.len();
        if settlement_before(function, current.id, end) {
            interval = interval
                .checked_add(1)
                .ok_or(StoreMutationMotionError::IdentityOverflow)?;
            let insert = if cursor == block_index { end - 1 } else { end };
            break (cursor, insert);
        }
        // The terminator instruction sits between the body and the
        // crossed edges, so a moved-place row or a hazard against the
        // moved store's reads or writes on it lands the store at the
        // block's end before it runs.
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
            // A terminator without successors leaves no later position;
            // the write still runs, just at the block's end.
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        };
        // Edges to distinct blocks fork: the write can still cross when
        // every edge lands on one join directly or enters a region fed
        // only through the fork's own edges whose edges all reach that
        // join — a single-block arm is the smallest such region, a chain
        // or a nested reconvergent shape a grown one — and the join's
        // predecessors are exactly the crossed block and the region.
        // Every traversal of the crossed block then reaches
        // the join exactly once, so the write runs once there as it ran
        // once here — no path gains or drops it. Any other shape lands
        // the store at the fork's end.
        if edges.iter().any(|edge| edge.block != first.block) {
            // A dangling edge target is malformed source, as on the
            // linear path.
            if edges
                .iter()
                .any(|edge| !function.block(edge.block).is_some())
            {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
            let Some((join, region)) = reconvergent_join(function, cursor, &edges, &visited) else {
                break land_at_end(if cursor == block_index { end - 1 } else { end });
            };
            // Every region block must stay walkable end to end: a stop
            // inside the region could only land the store inside it,
            // dropping the write on the paths its other blocks carry — so
            // one blocked block bounds the motion at the forked block's
            // end, not inside the region.
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
        let Some(next) = function.block_index(first.block) else {
            return Err(StoreMutationMotionError::SourceMismatch);
        };
        // The successor must see the crossed block as its only
        // predecessor — a join adds the write to paths that never carried
        // it — and must not reach any walked block: a return path would
        // close a cycle whose unverified interval could reorder an access
        // across the store.
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
    // A move must land somewhere later: a same-block landing at the
    // store's own index means the next position was already a stop.
    if target_block_index == block_index && insert_index == store_index {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        block: block.id,
        store_index,
        target_block_index,
        insert_index,
        interval,
    })
}
