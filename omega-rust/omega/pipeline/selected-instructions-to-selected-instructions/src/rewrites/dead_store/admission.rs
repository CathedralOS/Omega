//! Shared admission for dead-store elimination: locate the named `Store`,
//! `StorePacked`, or own-storage `Store64`, prove its single exact write row,
//! then walk forward to the first access on the dead place and require it to
//! be a write of the dead place's storage whose own row covers the dead range
//! entirely.
//!
//! Interference is decided from the validated access roster. A row naming the
//! dead place interferes when it can observe the stored bytes or leave them
//! observable: any overlapping read, any write that is not
//! the exact covering write, or a materialized local address. A
//! dynamic-extent row interferes the same way while its fixed offset starts
//! below the dead range's end — its reach is unbounded upward, so only a row
//! beginning at or past that end is provably disjoint. Rows for other
//! places are safe under place exclusivity. A `WriteLocal` on the dead
//! place's own storage interferes exactly like a `WritePlace` on that place:
//! an overlapping row decides coverage below, a disjoint row walks past. The
//! place's own storage is its `StructuralParameter`/`StructuralBlockParameter`
//! slot or the producing operation's `Structural` home — the place's
//! declaration names that producer. Any other `Structural` slot only stages
//! bytes that name the place (a call's staged view descriptor) under its own
//! slot coordinates, so its writes and materialized address never touch the
//! dead bytes at all.
//! Instructions without a row are admitted only when their kind cannot reach
//! semantic storage: private-slot frame accesses and pure register work.
//! Calls, hosted effects, and unaccounted writers reject.
//!
//! The walk is not confined to one block: reaching a block's end without
//! interference continues through its terminator's successor edges — every
//! edge out of a walked block is crossed, not only a single-successor
//! chain's, since a covering write further down still rewrites the dead
//! bytes when each path forward reaches one before any observer. A block is
//! only ever entered uncovered, so its first interfering access decides
//! every path through it at once: a covering write resolves the block for
//! all of them, anything else rejects. A block scanned clear defers coverage
//! to its distinct successors — a fork's legs may cover through different
//! writes or reconverge on one — and a join at a crossed block stays
//! harmless because coverage looks forward. The terminator's roster rows
//! decide first; each crossed edge's transports may not write or retire the
//! dead place's storage. A terminator with no successors lets the bytes
//! escape to the boundary, and an edge back into the store's own block
//! re-executes the removed store — which cannot be its own covering write —
//! so each ends the walk in rejection.
//!
//! A walked region needs no all-paths coverage proof to still eliminate:
//! the region is closed under successors, so a path that never reaches a
//! covering write is confined to clear blocks forever — no interfering
//! access, barrier, quiet-violating transport, or boundary escape exists on
//! it — and can never observe the dead bytes. Fork legs that disagree about
//! coverage, self-loops, and writerless cycles therefore admit: the store is
//! dead on every path, whether the path's bytes are rewritten or merely
//! never read again. Boundary settlements inside the walked region still
//! reject, since a boundary event positioned where the bytes remain current
//! could observe them.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedStructuralTransport, SelectedSuccessor,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use super::DeadStoreEliminationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::place_storage::{local_slot_is_place_storage, structural_place_declarations};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub store_index: usize,
    /// Index of the store's single write row in `memory_accesses`; replay
    /// requires the proposed roster to drop exactly this row.
    pub store_access: usize,
}

/// One exact byte range within one place root.
struct Dead {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
}

impl Dead {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + u64::from(self.byte_count)
            && u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count)
    }

    /// A dynamic-extent row's reach is unbounded only upward: a span row
    /// covers `length` bytes starting at `byte_offset` and a sequence row
    /// touches the single byte `byte_offset + index`, so every byte the row
    /// can touch lies at or after `byte_offset`. It still reaches this range
    /// exactly while its fixed offset starts below the range's end; an
    /// offset at or past the end is provably disjoint however far the reach
    /// extends.
    fn reached_by(&self, access: &SelectedMemoryAccess) -> bool {
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + u64::from(self.byte_count)
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, DeadStoreEliminationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(DeadStoreEliminationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(DeadStoreEliminationError::SourceMismatch)?;
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
        .ok_or(DeadStoreEliminationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let dead_store = &block.instructions[store_index];
    // The dead store writes the place's storage by one of the routes the
    // covering write can also take: the plain exact-width `Store`, or the
    // `StorePacked` an odd fragment width selects, either through the
    // referent pointer or through the materialized address of the place's
    // own parameter storage; or the always-eight-byte `Store64` into that
    // slot directly. The packed form's extra early-clobber scratch `Def`
    // drops with the instruction, so its admission additionally proves that
    // register occurs nowhere else in the function — a surviving mention
    // would lose its definition to the removal.
    let (encoded_offset, encoded_size, packed, direct_slot) = match dead_store.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(DeadStoreEliminationError::UnsupportedInstruction);
            }
            (byte_offset, u32::from(byte_size), false, None)
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            (byte_offset, u32::from(width.byte_size()), true, None)
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => (byte_offset, 8, false, Some(slot)),
        _ => return Err(DeadStoreEliminationError::UnsupportedInstruction),
    };
    // The write's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction encodes. The row's role must match
    // the route the instruction takes to the place's storage: `WritePlace`
    // for the referent-pointer place stores, or `WriteLocal` on the place's
    // own storage slot for the local-storage routes — the direct `Store64`'s
    // row naming the same slot the instruction encodes. An operation-owned
    // `Structural` slot is that storage only when the place's declaration
    // names the operation as its producer; a slot that merely stages bytes
    // naming the place never moves the place's bytes and cannot be the dead
    // store of them. Neither admitted role carries an obligation payload, so
    // dropping the row loses no proof receipt.
    let structural_places = structural_place_declarations(function);
    let mut rows = function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| access.instruction == store);
    let (store_access, write) = rows
        .next()
        .ok_or(DeadStoreEliminationError::UnsupportedInstruction)?;
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
        return Err(DeadStoreEliminationError::UnsupportedPair);
    }
    let dead = Dead {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
    };
    // The removed instruction must keep the operand surface its kind
    // declares: the plain two-use place store, the packed store's
    // two-use-plus-dead-scratch row, or the direct slot store's single-use
    // row. An exotic operand surface would make the removal contract
    // unclear.
    if packed {
        packed_store_shape(dead_store, function, environment)?;
    } else if direct_slot.is_some() {
        local_store_shape(dead_store, environment)?;
    } else {
        place_store_shape(dead_store, environment)?;
    }
    // Walk forward along every path the crossed region admits: the first
    // access that can reach the dead bytes must be a write of the dead
    // place's storage whose row covers the dead range entirely; anything
    // else leaves the bytes observable or only partially overwritten on
    // every path through it. A block is only ever entered uncovered — the
    // dead bytes still current — so its scan decides identically for all
    // paths reaching it: a covering write resolves the block for all of
    // them, while any other interference, barrier, or unaccounted writer
    // leaves the bytes observable and rejects. A block scanned clear
    // crosses its terminator's roster rows and every outgoing edge's
    // transports, then defers coverage to its distinct successors.
    let mut killers: Vec<(usize, usize)> = Vec::new();
    let mut crossed: Vec<SelectedBlockId> = Vec::new();
    let mut queued = vec![false; function.blocks.len()];
    let mut interval = 0usize;
    queued[block_index] = true;
    let mut frontier = vec![(block_index, store_index + 1)];
    while let Some((cursor, cursor_start)) = frontier.pop() {
        let current = &function.blocks[cursor];
        let mut found = None;
        for (candidate_index, candidate) in
            current.instructions.iter().enumerate().skip(cursor_start)
        {
            reject_barrier(candidate)?;
            let mut has_row = false;
            let mut interfered = false;
            for access in function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == candidate.id)
            {
                has_row = true;
                interfered |= interferes(&dead, access, structural_places);
            }
            if interfered {
                covering_source(candidate, &dead, function, environment)?;
                found = Some(candidate_index);
                break;
            }
            if !has_row {
                reject_unaccounted(candidate)?;
            }
        }
        if let Some(candidate_index) = found {
            interval = interval
                .checked_add(candidate_index + 1 - cursor_start)
                .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
            killers.push((cursor, candidate_index));
            continue;
        }
        interval = interval
            .checked_add(current.instructions.len() - cursor_start)
            .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
        // The terminator instruction sits between the body and the crossed
        // edges, so its roster rows decide first; a terminator kind never
        // carries the covering store.
        let terminator = terminator_instruction(&current.terminator);
        if function.memory_accesses.iter().any(|access| {
            access.instruction == terminator.id && interferes(&dead, access, structural_places)
        }) {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
        // A terminator with no successors lets the bytes escape to the
        // boundary. Each crossed edge's transports stay quiet, then each
        // distinct successor joins the frontier exactly once — two edges
        // naming one block still arrive on separate transports.
        let edges = terminator_successors(&current.terminator);
        if edges.is_empty() {
            return Err(DeadStoreEliminationError::UnsupportedPair);
        }
        for edge in &edges {
            edge_unobserved(edge, &dead)?;
        }
        interval = interval
            .checked_add(edges.len())
            .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
        for edge in &edges {
            let next = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == edge.block)
                .ok_or(DeadStoreEliminationError::SourceMismatch)?;
            // An edge back into the dead store's own block re-executes the
            // removed store from its top — it can never be its own covering
            // write — so the looped path stays unproven.
            if next == block_index {
                return Err(DeadStoreEliminationError::UnsupportedPair);
            }
            if !queued[next] {
                queued[next] = true;
                frontier.push((next, 0));
            }
        }
        if cursor != block_index {
            crossed.push(current.id);
        }
    }
    // The walked region is closed under successors: every edge out of a
    // scanned-clear block was crossed into a block the walk also scanned.
    // A completed walk therefore admits without a coverage meet — a path
    // either reaches a covering write, or it is confined forever to clear
    // blocks where no access, transport, or terminator can observe the dead
    // bytes. Fork legs that disagree about coverage, self-loops, and
    // writerless cycles are all dead paths alike.
    // A boundary settlement positioned inside the dead interval is an event a
    // boundary could observe through; positions outside it only shift. The
    // interval covers the store's block after the removed store, every fully
    // crossed block, and each covering block through its covering store.
    let origin_cover = killers
        .iter()
        .find(|(killer, _)| *killer == block_index)
        .map(|(_, cover)| *cover);
    for settlement in function.boundary_settlements.iter() {
        let position = settlement.instruction_index as usize;
        if settlement.block == block.id {
            if store_index < position && origin_cover.is_none_or(|cover| position <= cover) {
                return Err(DeadStoreEliminationError::InterveningAccess);
            }
        } else if crossed.contains(&settlement.block)
            || killers.iter().any(|(killer, cover)| {
                *killer != block_index
                    && function.blocks[*killer].id == settlement.block
                    && position <= *cover
            })
        {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
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
        .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| DeadStoreEliminationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(DeadStoreEliminationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        store_index,
        store_access,
    })
}

/// The named store and the covering store share one operand surface: the
/// target's `[use pointer, use value]` row with nothing implicit attached.
fn place_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}

/// The removed packed store carries the target's declared `store_packed`
/// row: `[use pointer, use packed value]` plus the early-clobber scratch
/// `Def` the multi-instruction store writes through. Removing the
/// instruction removes that definition, so the scratch register must occur
/// nowhere else in the function — any other operand position, terminator
/// operand, or successor transport still naming it would observe a
/// definition the rewrite stopped making.
fn packed_store_shape(
    instruction: &SelectedInstruction,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    if environment.selected_keys().store_packed != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if row.operands.len() != 3
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[2].operand != 2
        || row.operands[2].access != RegisterOperandAccess::Def
        || !row.operands[2].early_clobber
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    if instruction.operands.len() != 3
        || instruction.operands[2].operand != 2
        || instruction.operands[2].access != RegisterOperandAccess::Def
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    if !scratch_definition_is_dead(function, instruction.operands[2].virtual_register) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}

/// The direct slot store's operand surface: the target's declared `store64`
/// row — exactly `[use value]` — and the instruction carrying just that one
/// use. Removing it removes no register definition, so no custody check like
/// the packed scratch's is needed.
fn local_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    if environment.selected_keys().store64 != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if row.operands.len() != 1
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || instruction.operands.len() != 1
        || instruction.operands[0].operand != 0
        || instruction.operands[0].access != RegisterOperandAccess::Use
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}

/// Whether `register`'s only occurrence in `function` is one `Def` operand —
/// the custody the packed store's dropped scratch requires. The operand
/// itself is that one occurrence: any other operand position, terminator
/// operand, or successor transport naming the register would leave the
/// elimination removing a definition a surviving read or second definition
/// still observes.
fn scratch_definition_is_dead(
    function: &SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    let mut occurrences = 0_usize;
    for block in &function.blocks {
        for instruction in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
        {
            occurrences += instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
                .count();
        }
        for successor in terminator_successors(&block.terminator) {
            occurrences += successor
                .structural_bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.transport,
                        SelectedStructuralTransport::WholeValue { argument, .. }
                            | SelectedStructuralTransport::Descriptor { argument, .. }
                            if argument == register
                    )
                })
                .count();
            if let Some(case) = &successor.structural_case {
                occurrences += case
                    .payloads
                    .iter()
                    .filter(|payload| match &payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter,
                        } => *parameter == register,
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => *argument == register || *parameter == register,
                    })
                    .count();
            }
            occurrences += successor
                .bindings
                .iter()
                .filter(|binding| match &binding.transport {
                    selected_instructions::SelectedValueTransport::Unused => false,
                    selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == register || *parameter == register,
                })
                .count();
        }
    }
    occurrences == 1
}

/// Whether one roster row can observe the dead bytes or leave them
/// observable. Reads must intersect the dead range. A dynamic-extent row on
/// the dead place reaches only upward from its fixed offset, so it
/// interferes exactly while that offset starts below the dead range's end;
/// a row starting at or past the end is provably disjoint and walks past.
/// Writes must target the same place root to overlap; the covering
/// write is checked by the caller after this returns true. A `WriteLocal`
/// row names an exact range on a slot: when the slot is the dead place's
/// own storage, range intersection decides and an intersecting row still has
/// to cover; when the slot only stages bytes naming the place, its bytes
/// are not the place's at any offset, so the row never interferes. A
/// materialized local address could reach the same storage by a route the
/// roster does not bound, so it interferes when its slot is the dead
/// place's storage; a staged slot's address reaches only the staged bytes.
/// Outgoing-area storage never aliases a referent place.
fn interferes(
    dead: &Dead,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            access.place == dead.place && dead.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. } => {
            access.place == dead.place && dead.reached_by(access)
        }
        SelectedMemoryAccessRole::WriteLocal { slot } => {
            local_slot_is_place_storage(slot, dead.place, structural_places)
                && dead.intersects(access)
        }
        SelectedMemoryAccessRole::AddressLocal { slot } => {
            local_slot_is_place_storage(slot, dead.place, structural_places)
        }
        SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// The found access must be a write of the dead place's storage whose single
/// roster row covers the dead range entirely and names the same bytes the
/// instruction encodes:
/// - `Store` of any exact width or packed `StorePacked` carrying `WritePlace`
///   — through a place pointer — or `WriteLocal` on the place's own
///   storage slot, through that slot's materialized address;
/// - `Store64` into `Local(slot)` carrying `WriteLocal` on that same slot —
///   directly into the place's own storage.
///
/// A write that only partially overlaps the dead range leaves the remaining
/// bytes observable. A `WriteLocal` on an operation-owned `Structural` slot
/// covers only when the place's declaration names that operation as the
/// slot's producer — otherwise the slot stages bytes that merely name the
/// place, and a staging row never reaches this check because it does not
/// interfere. A materialized local address exposes storage rather than
/// writing it.
fn covering_source(
    instruction: &SelectedInstruction,
    dead: &Dead,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    let structural_places = structural_place_declarations(function);
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(reject());
    };
    if rows.next().is_some() || row.place != dead.place {
        return Err(reject());
    }
    // The encoded byte range must equal the row's exact range, and the row's
    // role must match the route the instruction takes to the dead place's
    // storage.
    let (encoded_offset, encoded_size) = match instruction.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(reject());
            }
            place_store_shape(instruction, environment)?;
            match row.role {
                SelectedMemoryAccessRole::WritePlace => {}
                SelectedMemoryAccessRole::WriteLocal { slot }
                    if local_slot_is_place_storage(slot, dead.place, structural_places) => {}
                _ => return Err(reject()),
            }
            (byte_offset, u32::from(byte_size))
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            if environment.selected_keys().store_packed != Some(instruction.constraint) {
                return Err(DeadStoreEliminationError::ConstraintMismatch);
            }
            match row.role {
                SelectedMemoryAccessRole::WritePlace => {}
                SelectedMemoryAccessRole::WriteLocal { slot }
                    if local_slot_is_place_storage(slot, dead.place, structural_places) => {}
                _ => return Err(reject()),
            }
            (byte_offset, u32::from(width.byte_size()))
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => {
            if environment.selected_keys().store64 != Some(instruction.constraint) {
                return Err(DeadStoreEliminationError::ConstraintMismatch);
            }
            // The direct slot store covers only when the roster names the
            // same slot and that slot is the dead place's own storage.
            if row.role != (SelectedMemoryAccessRole::WriteLocal { slot })
                || !local_slot_is_place_storage(slot, dead.place, structural_places)
            {
                return Err(reject());
            }
            (byte_offset, 8)
        }
        _ => return Err(reject()),
    };
    if row.byte_offset != encoded_offset || row.byte_count != encoded_size {
        return Err(reject());
    }
    // Coverage is containment, not equality: the killer's bytes may start
    // before and extend past the dead range, but every dead byte must be
    // inside the row the killer writes.
    if row.byte_offset > dead.byte_offset
        || u64::from(row.byte_offset) + u64::from(row.byte_count)
            < u64::from(dead.byte_offset) + u64::from(dead.byte_count)
    {
        return Err(reject());
    }
    Ok(())
}

/// Calls, hosted effects, and terminator kinds are always barriers: they can
/// observe or expose reachable storage regardless of their roster rows, and a
/// terminator kind never belongs in a block body.
fn reject_barrier(instruction: &SelectedInstruction) -> Result<(), DeadStoreEliminationError> {
    use SelectedInstructionKind::*;
    match instruction.kind {
        CallUnit { .. }
        | CallScalar { .. }
        | CallAggregate { .. }
        | HostedReadByte { .. }
        | HostedWriteByteI32 { .. }
        | HostedExitProcessI32
        | ReturnScalar
        | ReturnAggregate { .. }
        | ReturnUnit
        | Jump
        | ConditionalBranchNonZero
        | ConditionalBranchU64LessThan
        | ConditionalBranchI64LessThan => Err(DeadStoreEliminationError::UnsupportedInstruction),
        _ => Ok(()),
    }
}

/// An interval instruction without a roster row must be unable to reach any
/// semantic or place-backed storage: private-slot frame accesses touch
/// compiler-owned spill/boundary slots that no referent place aliases, and
/// pure register work has no memory side at all. A referent store or any
/// other frame slot without its row is an unaccounted access.
fn reject_unaccounted(instruction: &SelectedInstruction) -> Result<(), DeadStoreEliminationError> {
    use SelectedInstructionKind::*;
    match instruction.kind {
        Store64 {
            slot:
                FrameStorageSlotId::Local(
                    LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
                ),
            ..
        }
        | Load8 { .. }
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
        | MaterializeBooleanI64LessOrEqual => Ok(()),
        Store { .. } | StorePacked { .. } | Store64 { .. } | CopyBytes => {
            Err(DeadStoreEliminationError::InterveningAccess)
        }
        _ => Err(DeadStoreEliminationError::UnsupportedInstruction),
    }
}

/// A crossed edge must not touch the dead place's storage. Register
/// transports cannot reach memory, but a structural destination, the case
/// custody slot, or a custody discard naming the dead place writes or retires
/// its bytes inside the dead interval.
fn edge_unobserved(
    successor: &SelectedSuccessor,
    dead: &Dead,
) -> Result<(), DeadStoreEliminationError> {
    for binding in &successor.structural_bindings {
        let destination = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue { destination, .. }
            | SelectedStructuralTransport::Descriptor { destination, .. } => destination,
        };
        if destination.structural_place() == Some(dead.place) {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
    }
    if let Some(case) = &successor.structural_case
        && (case.slot.structural_place() == Some(dead.place)
            || case.trivial_affine_discards.contains(&dead.place))
    {
        return Err(DeadStoreEliminationError::InterveningAccess);
    }
    Ok(())
}

/// The block's boundary settlements after removing the instruction at
/// `removed`: positions at or before it name instructions that stay put, and
/// every later position — including the after-body position — shifts one
/// ordinal earlier. Shared by proposal and replay so both compute the same
/// roster from the source, never from each other.
pub(super) fn shifted_boundary_settlements(
    function: &SelectedFunction,
    block: SelectedBlockId,
    removed: usize,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, DeadStoreEliminationError> {
    let body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(DeadStoreEliminationError::SourceMismatch)?
        .instructions
        .len();
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        if settlement.block != block {
            continue;
        }
        let position = settlement.instruction_index as usize;
        if position > body {
            return Err(DeadStoreEliminationError::SourceMismatch);
        }
        if position > removed {
            settlement.instruction_index = u32::try_from(position - 1)
                .map_err(|_| DeadStoreEliminationError::IdentityOverflow)?;
        }
    }
    Ok(shifted)
}
