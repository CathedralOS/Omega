//! Shared admission for dead-store elimination: locate the named `Store`,
//! `StorePacked`, own-storage `Store64`, byte-sequence `Store { 0, 1 }`, or
//! `CopyBytes` destination span, prove its write row, then walk forward to
//! the first access on the dead place and require it to be a write of the
//! dead place's storage whose own row covers the dead range entirely.
//!
//! Interference is decided from the validated access roster. A row naming the
//! dead place interferes when it can observe the stored bytes or leave them
//! observable: any overlapping read, any write that is not
//! the exact covering write, or a materialized local address. A
//! dynamic-extent row interferes the same way while its fixed offset starts
//! below the dead range's end — its reach is unbounded upward, so only a row
//! beginning at or past that end is provably disjoint. A sequence row whose
//! `index` resolves to a clean `MaterializeI64` touches exactly the byte
//! `byte_offset + index` instead, so it interferes only by landing inside
//! the dead extent — a landing anywhere off it walks past. When the dead
//! extent is itself dynamic the directions mirror the store-motion walk: an
//! exact or local row still reaches the dead byte once its own extent ends
//! past the row's fixed offset, and a dynamic-extent row on the dead place
//! always meets it — unless the dead store's own extent decider resolved
//! the same way, collapsing the extent to the fixed bytes the constant
//! names before the walk. A `CopyBytes` dead store carries that second
//! dynamic shape: its destination `WriteByteSpan` claims `length` bytes at
//! its fixed `byte_offset`, so the may-written set is again unbounded
//! upward from `byte_offset`; a resolved `length` collapses it to the exact
//! range `[byte_offset, byte_offset + length)` and every covering route
//! applies unchanged, while an unresolved one is covered only by another
//! `CopyBytes` whose destination span spells the same extent — the same
//! `byte_offset` and the same `length` value — since nothing else can bound
//! an unbounded reach. Rows for other
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
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedCasePayloadTransport,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedMemoryAccess, SelectedMemoryAccessRole, SelectedStructuralTransport, SelectedSuccessor,
    SelectedValueTransport, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use super::DeadStoreEliminationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::condition_state::materialized_bits;
use crate::rewrites::place_storage::{local_slot_is_place_storage, structural_place_declarations};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub store_index: usize,
    /// Indices of the dead store's roster rows in `memory_accesses`, in
    /// roster order — the one write row for every route but the `CopyBytes`,
    /// whose source read row drops with the instruction beside its
    /// destination span row. Replay requires the proposed roster to drop
    /// exactly these rows.
    pub store_accesses: Vec<usize>,
}

/// The bytes the dead store wrote within one place root: an exact range, or
/// a dynamic extent whose runtime decider places it anywhere at or after
/// `byte_offset` with no static upper bound — a byte-sequence store's
/// single written byte at `byte_offset + index`, or a `CopyBytes`
/// destination span's `length` bytes at `byte_offset`. When the decider
/// itself resolves to a clean materialized constant, `admit` collapses the
/// extent to the exact bytes the constant names before the walk: the dead
/// extent's position is then fixed, and every interference and coverage
/// check below decides on it.
struct Dead {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    extent: DeadExtent,
}

/// How a dynamic dead extent's reach is decided at runtime: the
/// byte-sequence store's `index` places its one byte at
/// `byte_offset + index`, and the `CopyBytes` destination span's `length`
/// is the number of bytes it writes at `byte_offset`. `Exact` carries no
/// decider — the dead range is `byte_count` bytes at `byte_offset`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DeadExtent {
    Exact,
    SequenceByte(semantic_vocabulary::ValueId),
    ByteSpan(semantic_vocabulary::ValueId),
}

impl DeadExtent {
    /// Whether the dead store's written bytes sit anywhere at or after
    /// `byte_offset` with no static upper bound — the interference shape
    /// both dynamic extents share.
    fn dynamic(&self) -> bool {
        !matches!(self, DeadExtent::Exact)
    }
}

impl Dead {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap. A dynamic dead extent is
    /// unbounded upward from `byte_offset`, so the exact row still reaches
    /// the written bytes once its own extent ends past that offset — ending
    /// at or below it is the only provable disjointness.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        if self.extent.dynamic() {
            return u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count);
        }
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
    /// extends. A sequence row whose `index` resolves to a clean
    /// `MaterializeI64` — the same carrier audit the covering routes run —
    /// touches exactly that one byte wherever its payload base sits, so it
    /// reaches this range only by landing inside it. When the dead extent
    /// is itself dynamic — its own extent decider unresolved — an
    /// unresolved row always meets it, and a resolved landing byte meets it
    /// only at or past the fixed offset the dead extent starts at.
    fn reached_by(&self, access: &SelectedMemoryAccess, function: &SelectedFunction) -> bool {
        if let SelectedMemoryAccessRole::ReadByteSequence { index, .. }
        | SelectedMemoryAccessRole::WriteByteSequence { index, .. } = access.role
            && let Ok(landed) = constant_index(function, index)
            && let Some(position) = u64::from(access.byte_offset).checked_add(landed)
        {
            let start = u64::from(self.byte_offset);
            return if self.extent.dynamic() {
                position >= start
            } else {
                position >= start && position < start + u64::from(self.byte_count)
            };
        }
        if self.extent.dynamic() {
            return true;
        }
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
    // would lose its definition to the removal. The `CopyBytes` dead store
    // writes a dynamic extent instead: `length` bytes at its destination
    // span's fixed `byte_offset`, carrying the copy's source `ReadByteSpan`
    // beside it on the same instruction.
    let (encoded_offset, encoded_size, packed, direct_slot, byte_span) = match dead_store.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(DeadStoreEliminationError::UnsupportedInstruction);
            }
            (byte_offset, u32::from(byte_size), false, None, false)
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            (byte_offset, u32::from(width.byte_size()), true, None, false)
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => (byte_offset, 8, false, Some(slot), false),
        SelectedInstructionKind::CopyBytes => (0, 0, false, None, true),
        _ => return Err(DeadStoreEliminationError::UnsupportedInstruction),
    };
    // The write's semantic identity: the place root and the same bytes the
    // instruction encodes. Every route but the `CopyBytes` carries exactly
    // one roster row, whose role must match the route the instruction takes
    // to the place's storage: `WritePlace`
    // for the referent-pointer place stores, or `WriteLocal` on the place's
    // own storage slot for the local-storage routes — the direct `Store64`'s
    // row naming the same slot the instruction encodes. An operation-owned
    // `Structural` slot is that storage only when the place's declaration
    // names the operation as its producer; a slot that merely stages bytes
    // naming the place never moves the place's bytes and cannot be the dead
    // store of them. The byte-sequence route takes the dynamic extent: a
    // `Store { 0, 1 }` writes one byte through a fully computed view
    // address, and its `WriteByteSequence` row carries the payload base as
    // `byte_offset` plus the runtime `index` — so the row's offset is a
    // lower bound the index extends, not the encoded range, and the written
    // byte's position is decided at runtime. Its bounds-obligation payload
    // drops with the row: the write it guarded is gone, and the address
    // computation's own provenance still names the obligation. The
    // `CopyBytes` row is its destination `WriteByteSpan`: `length` bytes at
    // the row's `byte_offset`, the same lower bound the runtime `length`
    // extends — and the instruction's other rows are the copy's reads, which
    // the removal drops beside it.
    let structural_places = structural_place_declarations(function);
    let mut write = None;
    let mut store_accesses = Vec::new();
    for (index, access) in function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| access.instruction == store)
    {
        if byte_span {
            match access.role {
                SelectedMemoryAccessRole::WriteByteSpan { .. } if write.is_none() => {
                    write = Some((index, access));
                }
                // The copy's source read — and any read role — drops with
                // the instruction; a second write row would be an effect
                // the removal contract does not name.
                SelectedMemoryAccessRole::ReadPlace
                | SelectedMemoryAccessRole::ReadByteSpan { .. }
                | SelectedMemoryAccessRole::ReadByteSequence { .. } => {}
                _ => return Err(DeadStoreEliminationError::UnsupportedPair),
            }
        } else {
            if write.is_some() {
                return Err(DeadStoreEliminationError::UnsupportedPair);
            }
            write = Some((index, access));
        }
        store_accesses.push(index);
    }
    let Some((_, write)) = write else {
        return Err(DeadStoreEliminationError::UnsupportedInstruction);
    };
    let extent = match write.role {
        SelectedMemoryAccessRole::WriteByteSequence { index, .. } => {
            DeadExtent::SequenceByte(index)
        }
        SelectedMemoryAccessRole::WriteByteSpan { length, .. } => DeadExtent::ByteSpan(length),
        _ => DeadExtent::Exact,
    };
    let storage_route = match (write.role, direct_slot) {
        (SelectedMemoryAccessRole::WritePlace, None) => true,
        (SelectedMemoryAccessRole::WriteLocal { slot }, None) => {
            local_slot_is_place_storage(slot, write.place, structural_places)
        }
        (SelectedMemoryAccessRole::WriteLocal { slot }, Some(encoded)) => {
            slot == encoded && local_slot_is_place_storage(slot, write.place, structural_places)
        }
        (SelectedMemoryAccessRole::WriteByteSequence { .. }, None) => {
            encoded_offset == 0 && encoded_size == 1 && write.byte_count == 1
        }
        // A `WriteByteSpan` names no encoded range — its `byte_count` is
        // contractual zero and the `length` is the authoritative extent —
        // so the row is the dead copy's write row only on a `CopyBytes`.
        (SelectedMemoryAccessRole::WriteByteSpan { .. }, None) => {
            byte_span && write.byte_count == 0
        }
        _ => false,
    };
    if !storage_route
        || (!extent.dynamic()
            && (write.byte_offset != encoded_offset || write.byte_count != encoded_size))
    {
        return Err(DeadStoreEliminationError::UnsupportedPair);
    }
    let mut dead = Dead {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
        extent,
    };
    // A dynamic dead extent whose own decider resolves through the
    // same carrier audit the covering routes run — sole `InstructionResult`
    // carrier, clean `MaterializeI64` definition, no edge-transport or
    // case-payload redefinition — collapses to fixed bytes before the
    // walk: the byte-sequence store's one byte at `byte_offset + index`,
    // or the copy's `length` bytes at `byte_offset`. Every check below
    // then decides on a fixed position — a row that
    // cannot contain or land on the dead bytes walks past, and a write that
    // does covers — while an unresolved decider, or an extent no u32 names,
    // leaves the reach unbounded upward from `byte_offset`.
    match dead.extent {
        DeadExtent::SequenceByte(index) => {
            if let Ok(landed) = constant_index(function, index)
                && let Some(position) = u64::from(dead.byte_offset).checked_add(landed)
                && let Ok(position) = u32::try_from(position)
            {
                dead.byte_offset = position;
                dead.byte_count = 1;
                dead.extent = DeadExtent::Exact;
            }
        }
        DeadExtent::ByteSpan(length) => {
            if let Ok(written) = constant_index(function, length)
                && let Ok(count) = u32::try_from(written)
            {
                dead.byte_count = count;
                dead.extent = DeadExtent::Exact;
            }
        }
        DeadExtent::Exact => {}
    }
    // The removed instruction must keep the operand surface its kind
    // declares: the plain two-use place store, the packed store's
    // two-use-plus-dead-scratch row, the direct slot store's single-use
    // row, or the copy's three-use-plus-two-dead-scratch row. An exotic
    // operand surface would make the removal contract unclear.
    if packed {
        packed_store_shape(dead_store, function, environment)?;
    } else if direct_slot.is_some() {
        local_store_shape(dead_store, environment)?;
    } else if byte_span {
        byte_span_store_shape(dead_store, write, function, environment)?;
    } else {
        place_store_shape(dead_store, environment)?;
    }
    // The copy's source read rides on the removed instruction beside its
    // destination span, so no row of the dead copy may reach the dead
    // extent: a source span overlapping the destination it feeds would
    // leave the removal contract guessing which bytes the copy observed.
    if byte_span
        && function.memory_accesses.iter().any(|access| {
            access.instruction == store
                && !matches!(access.role, SelectedMemoryAccessRole::WriteByteSpan { .. })
                && interferes(&dead, access, structural_places, function)
        })
    {
        return Err(DeadStoreEliminationError::UnsupportedPair);
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
                interfered |= interferes(&dead, access, structural_places, function);
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
            access.instruction == terminator.id
                && interferes(&dead, access, structural_places, function)
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
        store_accesses,
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

/// The removed `CopyBytes` carries the target's declared `copy_bytes` row:
/// `[use source, use destination, use count]` plus the two early-clobber
/// scratch defs the copy loop writes through. Removing the instruction
/// removes both scratch definitions, so each register must occur nowhere
/// else in the function — the custody the packed store's scratch needs —
/// and operand 2's count register must carry the destination span row's
/// `length` value: the row-instruction agreement that makes a resolved
/// constant, or a same-extent covering span naming that same value, the
/// extent the instruction really writes.
fn byte_span_store_shape(
    instruction: &SelectedInstruction,
    write: &SelectedMemoryAccess,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    if environment.selected_keys().copy_bytes != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if row.operands.len() != 5
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[2].operand != 2
        || row.operands[2].access != RegisterOperandAccess::Use
        || row.operands[3].operand != 3
        || row.operands[3].access != RegisterOperandAccess::Def
        || !row.operands[3].early_clobber
        || row.operands[4].operand != 4
        || row.operands[4].access != RegisterOperandAccess::Def
        || !row.operands[4].early_clobber
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    if instruction.operands.len() != 5
        || instruction.operands[3].operand != 3
        || instruction.operands[3].access != RegisterOperandAccess::Def
        || instruction.operands[4].operand != 4
        || instruction.operands[4].access != RegisterOperandAccess::Def
        || !scratch_definition_is_dead(function, instruction.operands[3].virtual_register)
        || !scratch_definition_is_dead(function, instruction.operands[4].virtual_register)
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let SelectedMemoryAccessRole::WriteByteSpan { length, .. } = write.role else {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    };
    let count = instruction
        .operands
        .iter()
        .find(|operand| operand.operand == 2 && operand.access == RegisterOperandAccess::Use)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?
        .virtual_register;
    let register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == count)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if !origin_carries(register.origin, length) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}

/// Whether `origin` names `value` as the source scalar the register carries:
/// an `EntryParameter`, `BlockParameter`, or `InstructionResult` all carry
/// their value's identity, so a runtime `length` — an entry or block
/// parameter with no materializing producer — agrees with the span row as
/// surely as a constant's instruction result does. Compiler-owned origins
/// carry no source value and never agree.
fn origin_carries(origin: VirtualRegisterOrigin, value: semantic_vocabulary::ValueId) -> bool {
    match origin {
        VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::BlockParameter { source_value, .. }
        | VirtualRegisterOrigin::InstructionResult { source_value, .. } => source_value == value,
        _ => false,
    }
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
/// a row starting at or past the end is provably disjoint and walks past —
/// as is a byte-sequence row whose resolved index lands it outside the
/// range entirely, wherever its payload base sits.
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
    function: &SelectedFunction,
) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            access.place == dead.place && dead.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. } => {
            access.place == dead.place && dead.reached_by(access, function)
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
///   directly into the place's own storage;
/// - `CopyBytes` carrying the destination `WriteByteSpan` — a dynamic-extent
///   row that can cover an exact dead range, and only when the span's reach
///   is itself exact: the count register's sole definition must be a clean
///   `MaterializeI64`, so the span writes a compile-time `count` bytes at
///   its fixed `byte_offset`. Containment then decides on constants —
///   `byte_offset` at or before the dead start and `byte_offset + count`
///   reaching the dead end. A `CopyBytes` carries a source read span beside
///   the destination write, so its roster holds several rows: the span is
///   the single row that may reach the dead range, and every other row must
///   stay quiet on it — a read reaching the dead bytes observes them before
///   the write rewrites them;
/// - `Store { 0, 1 }` carrying `WriteByteSequence` — the other
///   dynamic-extent row that can cover an exact dead range, and only when
///   the index resolves exact the same way: the register carrying the row's
///   `index` value must hold the function's sole `InstructionResult`
///   definition of it in a clean `MaterializeI64` with no edge-transport or
///   case-payload redefinition, so the write lands on the one fixed byte
///   `byte_offset + index`. A single-byte write covers a single-byte dead
///   range exactly when that is the dead byte; a runtime index lands the
///   write anywhere at or past the payload base and can never provably
///   rewrite one fixed byte;
/// - for a byte-sequence dead store whose `index` stays runtime, another
///   `Store { 0, 1 }` carrying `WriteByteSequence` — the only write that
///   can provably land on the dead byte: same payload base and same
///   runtime `index` spell the same position, and distinct index values
///   still do when both resolve to constants whose `byte_offset + index`
///   sums agree. Any exact or local row would have to contain a byte
///   placed at runtime, and a sequence write whose index stays runtime or
///   lands elsewhere may land on a different byte entirely. A dead index
///   that resolves never reaches this arm: the extent collapsed to that
///   one byte, so coverage is the exact-range decision above — a sequence
///   write landing on it covers like any other, and an exact or local
///   write containing it covers too.
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
    // A `CopyBytes` writes `count` bytes into the dead place through the
    // destination span its `WriteByteSpan` row records, and carries the
    // copy's source read beside it — the only covering write with several
    // roster rows, and the only dynamic-extent row that can cover an exact
    // dead range at all.
    if matches!(instruction.kind, SelectedInstructionKind::CopyBytes) {
        return byte_span_covering(instruction, dead, function, environment, structural_places);
    }
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
    // An unresolved `CopyBytes` dead store wrote `length` bytes at its
    // span's `byte_offset` — an extent unbounded upward that only another
    // `CopyBytes` spelling the same extent could rewrite; that route
    // returned above, so no other write kind can cover it.
    if matches!(dead.extent, DeadExtent::ByteSpan(_)) {
        return Err(reject());
    }
    // A byte-sequence dead store wrote exactly one byte at
    // `byte_offset + index`; the covering write must spell that same byte.
    // Equal row offset and equal index identity place it exactly — no other
    // write route can, since an exact range cannot contain a byte whose
    // position is only known at runtime. Distinct index values still spell
    // one byte when each resolves to a compile-time constant — the same
    // carrier audit the exact-dead-range route runs — because the two
    // `byte_offset + index` sums then name one fixed position apiece, and
    // equal sums are the same byte whatever the payload bases were.
    if let DeadExtent::SequenceByte(index) = dead.extent {
        if !matches!(
            instruction.kind,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 1
            }
        ) {
            return Err(reject());
        }
        place_store_shape(instruction, environment)?;
        let SelectedMemoryAccessRole::WriteByteSequence {
            index: covering, ..
        } = row.role
        else {
            return Err(reject());
        };
        if row.byte_count != 1 {
            return Err(reject());
        }
        if covering == index {
            return if row.byte_offset == dead.byte_offset {
                Ok(())
            } else {
                Err(reject())
            };
        }
        let dead_byte = u64::from(dead.byte_offset)
            .checked_add(constant_index(function, index)?)
            .ok_or_else(reject)?;
        let written = u64::from(row.byte_offset)
            .checked_add(constant_index(function, covering)?)
            .ok_or_else(reject)?;
        return if written == dead_byte {
            Ok(())
        } else {
            Err(reject())
        };
    }
    // An exact dead byte's other covering route: a sequence write whose
    // index resolves to a materialized constant lands on one fixed byte.
    // The exact-range arms below cannot place a sequence row — its
    // `byte_offset` is a payload base the index extends, not the written
    // offset — so the constant index decides here.
    if let SelectedMemoryAccessRole::WriteByteSequence { index, .. } = row.role {
        return byte_sequence_covering(instruction, dead, row, index, function, environment);
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

/// The `CopyBytes` covering route: a `WriteByteSpan` row claims `length`
/// bytes at its fixed `byte_offset`, but the instruction really writes the
/// `count` its third operand carries — so the span covers an exact dead
/// range only when `count` is itself a compile-time constant. The count
/// register must carry the row's own `length` value, hold its function's
/// sole definition in a clean `MaterializeI64`, and never be redefined by
/// an edge transport or case payload the instruction audit cannot see —
/// only then does `byte_offset + count` bound the written span exactly.
/// Coverage is containment on the resolved constants: the span starts at or
/// before the dead range and its constant extent reaches the dead end.
/// A byte-sequence dead store still cannot be covered here — its dead byte
/// sits at `byte_offset + index`, a position no fixed span can contain.
///
/// An unresolved `CopyBytes` dead extent is the other shape this route
/// covers: it reaches unboundedly upward from `byte_offset`, so no bounded
/// write can contain it — only another `CopyBytes` spelling the same
/// extent, the same `byte_offset` and the same `length` value, rewrites
/// every byte the dead copy could have written whatever the runtime count.
///
/// The roster's other rows on the copy — its source read span among them —
/// must stay quiet on the dead range: exactly one row may reach it, the
/// covering span itself. A second reaching row either reads the dead bytes
/// before the write rewrites them or writes a second span that the single
/// covering claim cannot describe.
fn byte_span_covering(
    instruction: &SelectedInstruction,
    dead: &Dead,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
    structural_places: &[StructuralPlaceDeclaration],
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    if matches!(dead.extent, DeadExtent::SequenceByte(_)) {
        return Err(reject());
    }
    let mut covering = None;
    for access in function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id)
    {
        if !interferes(dead, access, structural_places, function) {
            continue;
        }
        if covering.is_some() {
            return Err(reject());
        }
        covering = Some(access);
    }
    let Some(row) = covering else {
        return Err(reject());
    };
    // The span row names the dynamic extent the copy writes; its byte count
    // is contractual zero — the runtime `length` is the authoritative reach.
    let SelectedMemoryAccessRole::WriteByteSpan { length, .. } = row.role else {
        return Err(reject());
    };
    if row.byte_count != 0 {
        return Err(reject());
    }
    // The target's declared `copy_bytes` row pins operand 2 as the count
    // use, so the register named there is the extent the instruction writes.
    if environment.selected_keys().copy_bytes != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let count = instruction
        .operands
        .iter()
        .find(|operand| operand.operand == 2 && operand.access == RegisterOperandAccess::Use)
        .ok_or_else(reject)?
        .virtual_register;
    // Row-instruction agreement for a dynamic extent: the count register
    // must carry the `length` value the row claims, so a resolved constant
    // — or a same-extent dead span naming the value — really is the reach
    // the instruction writes.
    let register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == count)
        .ok_or_else(reject)?;
    if !origin_carries(register.origin, length) {
        return Err(reject());
    }
    // The same-extent route: the dead copy's `length` never resolved, so
    // its reach is unbounded upward and only a span writing the identical
    // extent — the same `byte_offset` and the same `length` value — can
    // provably rewrite all of it.
    if let DeadExtent::ByteSpan(dead_length) = dead.extent {
        return if row.byte_offset == dead.byte_offset && length == dead_length {
            Ok(())
        } else {
            Err(reject())
        };
    }
    let written = materialized_bits(function, count).map_err(|_| reject())?;
    if transport_defines(function, count) {
        return Err(reject());
    }
    if row.byte_offset > dead.byte_offset {
        return Err(reject());
    }
    let needed =
        u64::from(dead.byte_offset) + u64::from(dead.byte_count) - u64::from(row.byte_offset);
    if written < needed {
        return Err(reject());
    }
    Ok(())
}

/// The `WriteByteSequence` covering route for an exact dead range: the row
/// claims the one byte at `byte_offset + index`, so it covers only when the
/// index is itself a compile-time constant and the dead range is that one
/// byte. The index's constant resolution mirrors the span's count: the
/// `index` value's register must be the function's sole `InstructionResult`
/// carrier of it — more or fewer carriers leave the value's producer
/// unproven — hold its sole definition in a clean `MaterializeI64`, and
/// never be redefined by an edge transport or case payload. Only then is
/// `byte_offset + index` a fixed position, and the dead range is covered
/// exactly when it is that one byte: a one-byte dead store at
/// `byte_offset + index`. A runtime index lands the write anywhere at or
/// past the payload base, so it can never provably rewrite one fixed byte;
/// a resolved index landing anywhere but the dead byte leaves it
/// observable.
fn byte_sequence_covering(
    instruction: &SelectedInstruction,
    dead: &Dead,
    row: &SelectedMemoryAccess,
    index: semantic_vocabulary::ValueId,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    // The route's encoded shape: the one-byte store through a fully
    // computed view address on the plain `[use pointer, use value]` row,
    // and the sequence row's contractual one-byte count.
    if !matches!(
        instruction.kind,
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 1
        }
    ) || row.byte_count != 1
    {
        return Err(reject());
    }
    place_store_shape(instruction, environment)?;
    let landed = constant_index(function, index)?;
    // The write lands on one byte; the dead range is covered exactly when
    // it is that byte.
    if dead.byte_count != 1
        || u64::from(row.byte_offset)
            .checked_add(landed)
            .ok_or_else(reject)?
            != u64::from(dead.byte_offset)
    {
        return Err(reject());
    }
    Ok(())
}

/// The compile-time constant a byte-sequence row's `index` resolves to, when
/// it does. The register carrying the `index` value is its sole
/// `InstructionResult` carrier — the way the copy's count operand names the
/// span's extent — so an `index` no instruction result carries (an entry or
/// block parameter) has no producer to resolve, and two instruction results
/// claiming one value make the constant ambiguous; both stay unproven. The
/// carrier must then hold the function's one clean `MaterializeI64`
/// definition and never be redefined by an edge transport or case payload
/// the instruction audit cannot see — only then does `byte_offset + index`
/// name a fixed position rather than a runtime-placed byte.
fn constant_index(
    function: &SelectedFunction,
    index: semantic_vocabulary::ValueId,
) -> Result<u64, DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    let mut carriers = function.virtual_registers.iter().filter(|register| {
        matches!(
            register.origin,
            VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == index
        )
    });
    let carrier = carriers.next().ok_or_else(reject)?;
    if carriers.next().is_some() {
        return Err(reject());
    }
    let landed = materialized_bits(function, carrier.id).map_err(|_| reject())?;
    if transport_defines(function, carrier.id) {
        return Err(reject());
    }
    Ok(landed)
}

/// Whether an edge transport or case payload defines `register` — a
/// definition the instruction-operand audit in `materialized_bits` cannot
/// see, which would falsify the constant it reports for the count.
fn transport_defines(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
    for block in &function.blocks {
        for successor in terminator_successors(&block.terminator) {
            if successor.bindings.iter().any(|binding| {
                matches!(
                    binding.transport,
                    SelectedValueTransport::Registers { parameter, .. } if parameter == register
                )
            }) {
                return true;
            }
            if let Some(case) = &successor.structural_case
                && case.payloads.iter().any(|payload| {
                    matches!(
                        payload.transport,
                        SelectedCasePayloadTransport::Unmaterialized { parameter }
                            | SelectedCasePayloadTransport::Registers { parameter, .. }
                            if parameter == register
                    )
                })
            {
                return true;
            }
        }
    }
    false
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
