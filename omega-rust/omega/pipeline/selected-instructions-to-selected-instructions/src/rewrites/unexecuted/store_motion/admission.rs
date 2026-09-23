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
use register_model::RegisterOperandAccess;
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
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, extent_intersects, extent_reached_by,
    local_slot_is_place_storage, local_store_shape, packed_store_row_shape, place_store_row_shape,
    slot_is_subject_storage, staging_slot, structural_place_declarations,
};
use crate::rewrites::window_hazards::{coupled, is_barrier};

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

/// The bytes the moved store writes within one place root: an exact range,
/// or a dynamic extent when the store is a byte-sequence write — its single
/// written byte sits at `byte_offset + index` for the runtime `index`, so
/// every byte it can touch lies at or after `byte_offset` with no static
/// upper bound. When that `index` itself resolves to a clean materialized
/// constant, `admit` collapses the extent to the one byte
/// `byte_offset + index` before the walk: the moved byte's position is then
/// fixed, and every interference check below decides on it. `place` is the
/// place the moved row names — the place the staged bytes name when the
/// subject is a staging slot — while `storage` settles which bytes the walk
/// actually tracks.
struct Moved {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    /// The byte-sequence store's runtime index, deciding the written byte's
    /// position: `byte_offset + index`. `None` for an exact store, and for
    /// a byte-sequence store whose index resolved — its moved byte is the
    /// collapsed `byte_offset` then.
    sequence_index: Option<semantic_vocabulary::ValueId>,
    storage: SubjectStorage,
}

impl Moved {
    /// Exact rows intersect when their half-open byte intervals share a byte.
    /// A dynamic moved extent is unbounded upward from `byte_offset`, so the
    /// exact row interferes once its own extent reaches that offset — only a
    /// row ending at or below it is provably disjoint.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        extent_intersects(
            self.byte_offset,
            self.byte_count,
            self.sequence_index.is_some(),
            access,
        )
    }

    /// Whether `access` can reach the moved extent — the shared
    /// dynamic-reach and resolved-landing decision in `place_storage`.
    fn reached_by(&self, access: &SelectedMemoryAccess, function: &SelectedFunction) -> bool {
        extent_reached_by(
            self.byte_offset,
            self.byte_count,
            self.sequence_index.is_some(),
            access,
            function,
        )
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

/// The moved store's operand surface: the target's `[use pointer, use value]`
/// row with nothing implicit attached.
fn place_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), StoreMutationMotionError> {
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(StoreMutationMotionError::ConstraintMismatch)?;
    place_store_row_shape(row, || StoreMutationMotionError::ConstraintMismatch)?;
    if !row.implicit_uses.is_empty()
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

/// The moved packed store carries the target's declared `store_packed` row:
/// `[use pointer, use packed value]` plus the early-clobber scratch `Def`
/// the multi-instruction store writes through. Unlike the removal rule, the
/// motion keeps the definition alive — it only needs the scratch's custody
/// inside the walked window, which the write-side coupling check below
/// proves. The row's implicit use and definition lists stay empty; whatever
/// the row declares as clobbers — the flag unit on a flag-publishing
/// target — moves with the instruction and is guarded the same way.
fn packed_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), StoreMutationMotionError> {
    if environment.selected_keys().store_packed != Some(instruction.constraint) {
        return Err(StoreMutationMotionError::ConstraintMismatch);
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(StoreMutationMotionError::ConstraintMismatch)?;
    packed_store_row_shape(row, || StoreMutationMotionError::ConstraintMismatch)?;
    if row.operands[0].early_clobber
        || row.operands[1].early_clobber
        || row
            .operands
            .iter()
            .any(|operand| operand.fixed_view.is_some() || operand.tied_to.is_some())
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
    {
        return Err(StoreMutationMotionError::ConstraintMismatch);
    }
    if instruction.operands.len() != 3
        || instruction.operands[0].operand != 0
        || instruction.operands[0].access != RegisterOperandAccess::Use
        || instruction.operands[1].operand != 1
        || instruction.operands[1].access != RegisterOperandAccess::Use
        || instruction.operands[2].operand != 2
        || instruction.operands[2].access != RegisterOperandAccess::Def
    {
        return Err(StoreMutationMotionError::ConstraintMismatch);
    }
    Ok(())
}

/// The register locations the move orders against the window: the registers
/// the moved store reads — the referent pointer or materialized slot address
/// plus the stored value for a place store, or just the stored value for the
/// direct `Store64` — and the registers it writes, the packed form's
/// early-clobber scratch. An edge transport naming a written register in
/// either direction would move a read or a second write across the moved
/// definition, so the crossed-edge check below needs both directions; the
/// per-instruction and terminator hazards ask `coupled`, which reads the
/// store's whole write surface — this `Def` plus any implicit definitions
/// and clobbers — directly off the instruction.
struct Carried {
    registers: Vec<VirtualRegisterId>,
    writes: Vec<VirtualRegisterId>,
}

/// Collect the moved store's register surface. Every operand must sit at the
/// position its index declares; a use joins the carried reads while the
/// packed form's scratch `Def` joins the writes. A definition on any other
/// form, or implicit writes or clobbers on a non-packed store, is a write
/// surface the route never declared and stays inadmissible.
fn carried_surface(
    instruction: &SelectedInstruction,
    packed: bool,
) -> Result<Carried, StoreMutationMotionError> {
    if !packed && (!instruction.implicit_defs.is_empty() || !instruction.clobbers.is_empty()) {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    let mut registers = Vec::with_capacity(instruction.operands.len());
    let mut writes = Vec::new();
    for (index, operand) in instruction.operands.iter().enumerate() {
        if operand.operand != index as u16 {
            return Err(StoreMutationMotionError::UnsupportedPair);
        }
        match operand.access {
            RegisterOperandAccess::Use => registers.push(operand.virtual_register),
            _ if packed => writes.push(operand.virtual_register),
            _ => return Err(StoreMutationMotionError::UnsupportedPair),
        }
    }
    Ok(Carried { registers, writes })
}

/// Whether one roster row touches the moved bytes or the place's dynamic
/// storage. Exact rows must intersect the moved range — when the moved
/// extent is itself dynamic, an exact row interferes once its extent ends
/// past the row's fixed offset, the only provable disjointness left. A
/// dynamic-extent row
/// on the moved place reaches only upward from its fixed offset, so it
/// interferes exactly while that offset starts below the moved range's end;
/// a row beginning at or past the end is provably disjoint and the store
/// slides past it like any disjoint row — as is a byte-sequence row whose
/// resolved index lands it outside the moved extent entirely, wherever its
/// payload base sits. Against a dynamic moved extent an
/// unresolved row always interferes — two unbounded-upward reaches on one
/// place can share a byte — while a resolved one interferes only at or past
/// the moved payload base. A `WriteLocal` row names an exact
/// range on a slot: when
/// the slot is the moved bytes' storage, range intersection decides; when
/// the slot is any other — a staging slot beside a place subject, or a
/// different slot beside a staging subject — its bytes are disjoint, so the
/// row never stops the walk. A materialized local address could reach the
/// same storage by a route the roster does not bound, so it stops the walk
/// when its slot is the moved bytes' storage; any other slot's address
/// reaches only that slot's bytes. For a staging subject the place-named
/// rows never interfere either: they describe the place's own extents — the
/// staged bytes are not the place's at any offset — and no place route
/// carries a slot's bytes. Outgoing-area storage never aliases a referent
/// place.
fn interferes(
    moved: &Moved,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
    function: &SelectedFunction,
) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            matches!(moved.storage, SubjectStorage::Place)
                && access.place == moved.place
                && moved.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::ReadElementView { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. }
        | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. } => {
            matches!(moved.storage, SubjectStorage::Place)
                && access.place == moved.place
                && moved.reached_by(access, function)
        }
        SelectedMemoryAccessRole::WriteLocal { slot } => {
            slot_is_subject_storage(slot, moved.storage, moved.place, structural_places)
                && moved.intersects(access)
        }
        SelectedMemoryAccessRole::AddressLocal { slot } => {
            slot_is_subject_storage(slot, moved.storage, moved.place, structural_places)
        }
        SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// Whether sliding the store past this instruction would change the observed
/// order of the write. Any of the stop conditions bound the window: a roster
/// row touching the moved bytes, a barrier, the full register and
/// condition-state coupling between the two instructions — the candidate
/// redefining a carried read, or reading or rewriting the moved store's own
/// scratch definition or declared clobbers — or an unaccounted memory reach.
fn instruction_stops(
    candidate: &SelectedInstruction,
    moved: &Moved,
    function: &SelectedFunction,
    store: &SelectedInstruction,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    let mut has_row = false;
    for access in function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == candidate.id)
    {
        has_row = true;
        if interferes(moved, access, structural_places, function) {
            return true;
        }
    }
    is_barrier(candidate) || coupled(store, candidate) || (!has_row && unaccounted_kind(candidate))
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
            | ExactMultiplyI64 { .. }
            | ExactAddI64Immediate { .. }
            | ExactSubtractI64Immediate { .. }
            | WrappingAddI64
            | WrappingSubtractI64
            | WrappingMultiplyI64
            | ExactDivideU64 { .. }
            | ExactRemainderU64 { .. }
            | WrappingRemainderI64 { .. }
            | WrappingDivideI64 { .. }
            | ExactDivideI64 { .. }
            | ExactRemainderI64 { .. }
            | SaturatingAdd { .. }
            | SaturatingSubtract { .. }
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
            | BitwiseAndI64
            | BitwiseOrI64
            | BitwiseNotI64
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
/// bytes' storage. A transport's `parameter` slot defines a register for
/// the successor while its `argument` slot reads one on the edge: a
/// parameter naming a carried read still stops the crossing, and either
/// slot naming a carried write — the packed scratch — would move a use or a
/// second definition across the moved store's own definition. Structural
/// destinations, the case custody slot, and custody discards write or
/// retire storage on the edge — the same conservative test the sibling
/// memory walks keep on edges, since an edge transport's destination role
/// is not decided here. For a staging subject the transport destination and
/// custody slot decide on the slot itself — staging bytes sit under their
/// own slot's coordinates — while a place custody discard retires the
/// place's storage, never the operation-owned slot.
fn edge_stops(successor: &SelectedSuccessor, moved: &Moved, carried: &Carried) -> bool {
    let writes = |register: &VirtualRegisterId| carried.writes.contains(register);
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers {
            argument,
            parameter,
        } = binding.transport
            && (carried.registers.contains(&parameter) || writes(&argument) || writes(&parameter))
        {
            return true;
        }
    }
    for binding in &successor.structural_bindings {
        // An address transport lends its root: moving a store to that root
        // across the edge would change what the joined pointer observes.
        let (argument, destination, lent) = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue {
                argument,
                destination,
                ..
            }
            | SelectedStructuralTransport::Descriptor {
                argument,
                destination,
            } => (Some(argument), destination, None),
            SelectedStructuralTransport::Address {
                base, destination, ..
            } => (
                match base {
                    selected_instructions::SelectedAddressBase::Register(argument) => {
                        Some(argument)
                    }
                    selected_instructions::SelectedAddressBase::Local(_) => None,
                },
                destination,
                Some(base),
            ),
        };
        let touches = match moved.storage {
            SubjectStorage::Place => {
                destination.structural_place() == Some(moved.place)
                    || (lent.is_some() && binding.semantic.argument.place == moved.place)
            }
            SubjectStorage::Staging(slot) => {
                destination == slot
                    || lent == Some(selected_instructions::SelectedAddressBase::Local(slot))
            }
        };
        if touches || argument.is_some_and(|argument| writes(&argument)) {
            return true;
        }
    }
    if let Some(case) = &successor.structural_case {
        let slot_touches = match moved.storage {
            SubjectStorage::Place => case.slot.structural_place() == Some(moved.place),
            SubjectStorage::Staging(slot) => case.slot == slot,
        };
        let discard_touches = matches!(moved.storage, SubjectStorage::Place)
            && case.trivial_affine_discards.contains(&moved.place);
        if slot_touches || discard_touches {
            return true;
        }
        for payload in &case.payloads {
            let (reads, defined) = match payload.transport {
                SelectedCasePayloadTransport::Unused => continue,
                SelectedCasePayloadTransport::Unmaterialized { parameter } => (None, parameter),
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => (Some(argument), parameter),
            };
            if carried.registers.contains(&defined)
                || writes(&defined)
                || reads.is_some_and(|argument| writes(&argument))
            {
                return true;
            }
        }
    }
    false
}

/// The join a fork's edges reconverge on when the region between keeps the
/// moved write's count at one per traversal — the diamond and the triangle
/// are its smallest shapes, a chain or a nested reconvergent arm a grown
/// one. The frontier is the set of edges still landing outside the region:
/// the fork's own edges plus every region block's edges leaving it. Each
/// pass either settles on the one block every frontier edge reaches or
/// absorbs the first frontier target every predecessor reaches only through
/// the crossed block or the region itself, so the region stays a
/// single-entry acyclic funnel — a block arrives after every block feeding
/// it — and a block with no successors never absorbs, since a path ending
/// inside the region would drop the write the move still owes it. `None`
/// when no frontier target can absorb while exits still disagree: a second
/// predecessor into the region or the join would run the write on a path
/// that never carried it, a frontier edge leaving anywhere but the one join
/// drops the write on the path that leaves, and a re-entered or
/// already-walked block never settles the order it would reorder.
fn reconvergent_join(
    function: &SelectedFunction,
    cursor: usize,
    edges: &[&SelectedSuccessor],
    visited: &[bool],
) -> Option<(usize, Vec<usize>)> {
    let mut member = vec![false; function.blocks.len()];
    let mut region: Vec<usize> = Vec::new();
    loop {
        // The frontier, in a fixed order both sides of the family walk —
        // the fork's edges first, then each region block's exits in absorb
        // order — so admission and validation grow the same region. A
        // target naming no real block leaves the frontier unnameable.
        let mut frontier: Vec<usize> = Vec::new();
        for edge in edges {
            let target = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == edge.block)?;
            if !member[target] {
                frontier.push(target);
            }
        }
        for &block in &region {
            for edge in terminator_successors(&function.blocks[block].terminator) {
                let target = function
                    .blocks
                    .iter()
                    .position(|candidate| candidate.id == edge.block)?;
                if !member[target] {
                    frontier.push(target);
                }
            }
        }
        // Every frontier edge landing on one block names the join — the
        // first block every traversal of the crossed block reaches. The
        // frontier cannot empty while exits disagree: absorbed blocks carry
        // successors and the region stays acyclic, so an edge always
        // leaves it.
        let join = *frontier.first()?;
        if frontier.iter().all(|&target| target == join) {
            // The join's predecessors are exactly the crossed block and the
            // region: any other incoming edge hands the join's readers a
            // write that path never carried, and a path back into the
            // walked chain would close a cycle whose unverified interval
            // could reorder an access across the moved write.
            if visited[join] {
                return None;
            }
            for (index, candidate) in function.blocks.iter().enumerate() {
                if index != cursor
                    && !member[index]
                    && terminator_successors(&candidate.terminator)
                        .iter()
                        .any(|edge| edge.block == function.blocks[join].id)
                {
                    return None;
                }
            }
            if reaches_visited(function, join, visited) {
                return None;
            }
            return Some((join, region));
        }
        // Exits disagree: absorb the first frontier target — in frontier
        // order — whose predecessors all lead from the crossed block or the
        // region. A target fed from anywhere else stays on the frontier:
        // absorbing it would let a path reach the join without the write.
        // A walked block, or a block carrying no successors, stays too —
        // absorbing it would close a cycle or end a path inside the
        // region and drop the write entirely.
        let mut absorbed = false;
        for &target in &frontier {
            if visited[target]
                || terminator_successors(&function.blocks[target].terminator).is_empty()
            {
                continue;
            }
            let fed_inside = function
                .blocks
                .iter()
                .enumerate()
                .all(|(index, candidate)| {
                    index == cursor
                        || member[index]
                        || terminator_successors(&candidate.terminator)
                            .iter()
                            .all(|edge| edge.block != function.blocks[target].id)
                });
            if fed_inside {
                member[target] = true;
                region.push(target);
                absorbed = true;
                break;
            }
        }
        if !absorbed {
            return None;
        }
    }
}

/// Whether any position inside a crossed region block must stay ordered
/// after the store — the producer's per-block audit. The store cannot land
/// inside the region without dropping the write on the paths its other
/// blocks carry, so the whole body must walk past: no stopping instruction,
/// no settlement at any position — the write ran before the region where
/// the settlement would now observe after it — no interfering row or
/// coupling on the block's terminator, and no stopping transport on the
/// block's edges, whether they stay inside the region or leave for the
/// join.
fn arm_stops(
    function: &SelectedFunction,
    arm: &selected_instructions::SelectedBlock,
    moved: &Moved,
    carried: &Carried,
    moved_store: &SelectedInstruction,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    arm.instructions
        .iter()
        .enumerate()
        .any(|(index, candidate)| {
            instruction_stops(candidate, moved, function, moved_store, structural_places)
                || settlement_before(function, arm.id, index)
        })
        || settlement_before(function, arm.id, arm.instructions.len())
        || {
            let terminator = terminator_instruction(&arm.terminator);
            function.memory_accesses.iter().any(|access| {
                access.instruction == terminator.id
                    && interferes(moved, access, structural_places, function)
            }) || coupled(moved_store, terminator)
        }
        || terminator_successors(&arm.terminator)
            .iter()
            .any(|edge| edge_stops(edge, moved, carried))
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
