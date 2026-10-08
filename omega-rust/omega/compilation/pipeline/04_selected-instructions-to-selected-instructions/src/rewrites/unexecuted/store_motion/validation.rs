//! Independent validation of store mutation motion.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! motion's legality from the source records — the named instruction's
//! store kind against its single write row, the byte extent the row's
//! place and range spell (or the fixed byte a resolved sequence index
//! lands it on), the operand surface the route declares, the forward walk
//! that lands the store immediately before the first position that must
//! stay ordered after it, the crossed-edge audits that keep the carried
//! registers and the moved storage untouched, and the boundary-settlement
//! remap the landing demands — then rebuilds the function the contract
//! demands and requires the proposal to equal it. Moving the store back
//! must reproduce the complete source by content. A producer admission
//! error therefore fails validation even when the proposal is exactly
//! what that producer emitted.

use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    FrameStorageSlotId, SelectedBlockId, SelectedBoundarySettlement, SelectedFunction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedMemoryAccessRole,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion};
use super::motion::{
    Moved, arm_stops, carried_surface, edge_stops, interferes, instruction_stops, packed_store_shape,
    place_store_shape, reaches_visited, reconvergent_join, settlement_before, sole_predecessor,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{function_at, terminator_instruction, terminator_successors};
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, local_slot_is_place_storage, local_store_shape, staging_slot,
    structural_place_declarations,
};
use crate::rewrites::window_hazards::coupled;

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
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(StoreMutationMotionError::SourceMismatch);
    }
    let function = function_at(plan, function_index, StoreMutationMotionError::SourceMismatch)?;
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
        let Some(next) = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == first.block)
        else {
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

/// The validation work this audit performs, in the measured-step
/// contract the family publishes: one step per block plus one per
/// instruction across the plan, one per walked interval position and
/// crossed edge, and one per roster row.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, StoreMutationMotionError> {
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
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

/// Build the function the contract demands from the validator's own
/// record: the named store moved from its source position to the proven
/// landing, the roster retained unchanged — its rows name instructions
/// by identity, so the move keeps them — and the target block's
/// settlements shifted over the inserted ordinal. The producer's
/// `rewrite` is not consulted; both sides derive the same function from
/// the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> Result<SelectedFunction, StoreMutationMotionError> {
    let mut expected = reconstructed.function.clone();
    expected.boundary_settlements = moved_boundary_settlements(
        reconstructed.function,
        reconstructed.block,
        reconstructed.store_index,
        expected.blocks[reconstructed.target_block_index].id,
        reconstructed.insert_index,
    )?;
    let moved = expected.blocks[reconstructed.block_index]
        .instructions
        .remove(reconstructed.store_index);
    expected.blocks[reconstructed.target_block_index]
        .instructions
        .insert(reconstructed.insert_index, moved);
    Ok(expected)
}

/// Move the store back and restore the source settlements: undoing the
/// validator's expected edit must restore the complete source by
/// content — every other function, block, instruction, register, call,
/// and access retained bit-identical.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, StoreMutationMotionError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(StoreMutationMotionError::ReplayMismatch)?;
    let target = function
        .blocks
        .get_mut(reconstructed.target_block_index)
        .ok_or(StoreMutationMotionError::ReplayMismatch)?;
    if reconstructed.insert_index >= target.instructions.len() {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    let moved_back = target.instructions.remove(reconstructed.insert_index);
    let source_block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(StoreMutationMotionError::ReplayMismatch)?;
    if reconstructed.store_index > source_block.instructions.len() {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    source_block
        .instructions
        .insert(reconstructed.store_index, moved_back);
    // The proposed settlements already matched the shifted source, so
    // restoring the source rows restores the source function.
    function.boundary_settlements = reconstructed.function.boundary_settlements.clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator
/// reconstructs the motion's preconditions and landing from the source,
/// requires the proposal to equal the function its own record produces,
/// and restores the complete source by content. The producer's admission
/// routine is never consulted, so a wrong legality decision fails here
/// even when the proposal matches the edit the producer emitted.
pub fn validate_store_mutation_motion(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedStoreMutationMotion, StoreMutationMotionError> {
    let reconstructed = reconstruct(source, function_index, store, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed,
    )? > budget.validation_steps()
    {
        return Err(StoreMutationMotionError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)?) {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    Ok(ValidatedStoreMutationMotion {
        receipt: StoreMutationMotionReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
