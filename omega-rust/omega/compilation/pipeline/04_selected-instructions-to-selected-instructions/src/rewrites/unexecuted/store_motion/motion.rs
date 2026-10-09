//! The motion decision surface both sides of the family walk.
//!
//! Admission proposes where a store may move; validation re-derives the
//! same window independently. Both read the same bytes — the moved
//! extent, the carried register surface, the row shapes, the per-
//! instruction and crossed-edge stops, the walk and reconvergence — so
//! the audit vocabulary lives once, here. Neither side calls the other:
//! the proposal and the proof share only these definitions.

use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
    SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedStructuralTransport, SelectedSuccessor, SelectedValueTransport, VirtualRegisterId,
    terminator_instruction,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use super::StoreMutationMotionError;
use crate::rewrites::block_edges::terminator_successors;
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, extent_intersects, extent_reached_by, packed_store_row_shape,
    place_store_row_shape, slot_is_subject_storage,
};
use crate::rewrites::window_hazards::{coupled, is_barrier, unaccounted_kind};

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
pub(super) struct Moved {
    pub(super) place: PlaceId,
    pub(super) byte_offset: u32,
    pub(super) byte_count: u32,
    /// The byte-sequence store's runtime index, deciding the written byte's
    /// position: `byte_offset + index`. `None` for an exact store, and for
    /// a byte-sequence store whose index resolved — its moved byte is the
    /// collapsed `byte_offset` then.
    pub(super) sequence_index: Option<semantic_vocabulary::ValueId>,
    pub(super) storage: SubjectStorage,
}

impl Moved {
    /// Exact rows intersect when their half-open byte intervals share a byte.
    /// A dynamic moved extent is unbounded upward from `byte_offset`, so the
    /// exact row interferes once its own extent reaches that offset — only a
    /// row ending at or below it is provably disjoint.
    pub(super) fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        extent_intersects(
            self.byte_offset,
            self.byte_count,
            self.sequence_index.is_some(),
            access,
        )
    }

    /// Whether `access` can reach the moved extent — the shared
    /// dynamic-reach and resolved-landing decision in `place_storage`.
    pub(super) fn reached_by(
        &self,
        access: &SelectedMemoryAccess,
        function: &SelectedFunction,
    ) -> bool {
        extent_reached_by(
            self.byte_offset,
            self.byte_count,
            self.sequence_index.is_some(),
            access,
            function,
        )
    }
}
/// The moved store's operand surface: the target's `[use pointer, use value]`
/// row with nothing implicit attached.
pub(super) fn place_store_shape(
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
pub(super) fn packed_store_shape(
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
pub(super) struct Carried {
    pub(super) registers: Vec<VirtualRegisterId>,
    pub(super) writes: Vec<VirtualRegisterId>,
}
/// Collect the moved store's register surface. Every operand must sit at the
/// position its index declares; a use joins the carried reads while the
/// packed or frame store's scratch `Def` joins the writes. A definition on any other
/// form, or implicit writes or clobbers on a non-packed store, is a write
/// surface the route never declared and stays inadmissible.
pub(super) fn carried_surface(
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
            RegisterOperandAccess::Def
                if packed
                    || matches!(instruction.kind, SelectedInstructionKind::Store64 { .. }) =>
            {
                writes.push(operand.virtual_register)
            }
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
pub(super) fn interferes(
    moved: &Moved,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
    function: &SelectedFunction,
) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadReturnedView { .. } => true,
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            matches!(moved.storage, SubjectStorage::Place)
                && access.place == moved.place
                && moved.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::ReadMeasuredSpan { .. }
        | SelectedMemoryAccessRole::ReadElementView { .. }
        | SelectedMemoryAccessRole::ReadIndexedPrimitive { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. }
        | SelectedMemoryAccessRole::WriteElementView { .. }
        | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
        | SelectedMemoryAccessRole::WriteIndexedStructural { .. } => {
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
pub(super) fn instruction_stops(
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
/// A boundary settlement at a position in the crossed interval would observe
/// the place before the moved write instead of after it.
pub(super) fn settlement_before(
    function: &SelectedFunction,
    block: SelectedBlockId,
    position: usize,
) -> bool {
    function.boundary_settlements.iter().any(|settlement| {
        settlement.block == block && settlement.instruction_index as usize == position
    })
}
/// Whether `block`'s only predecessor block is `expected`: every other
/// block's terminator is scanned for an edge naming it, so a second incoming
/// edge or a self-loop keeps the join visible.
pub(super) fn sole_predecessor(function: &SelectedFunction, block: usize, expected: usize) -> bool {
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
pub(super) fn reaches_visited(function: &SelectedFunction, start: usize, visited: &[bool]) -> bool {
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
            if let Some(target) = function.block_index(edge.block) {
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
pub(super) fn edge_stops(successor: &SelectedSuccessor, moved: &Moved, carried: &Carried) -> bool {
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
            } => (Some(argument), destination, None),
            SelectedStructuralTransport::ScalarReference {
                base, destination, ..
            }
            | SelectedStructuralTransport::Address {
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
            SubjectStorage::Place => case.source.place() == Some(moved.place),
            SubjectStorage::Staging(slot) => case.source.local_slot() == Some(slot),
        };
        let discard_touches = matches!(moved.storage, SubjectStorage::Place)
            && (case
                .cleanup_actions
                .iter()
                .any(|action| action.place() == moved.place));
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
pub(super) fn reconvergent_join(
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
            let target = function.block_index(edge.block)?;
            if !member[target] {
                frontier.push(target);
            }
        }
        for &block in &region {
            for edge in terminator_successors(&function.blocks[block].terminator) {
                let target = function.block_index(edge.block)?;
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
pub(super) fn arm_stops(
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
