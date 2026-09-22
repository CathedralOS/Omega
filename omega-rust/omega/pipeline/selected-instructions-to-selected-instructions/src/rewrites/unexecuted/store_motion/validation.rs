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
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedBoundarySettlement,
    SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedStructuralTransport, SelectedSuccessor,
    SelectedValueTransport, VirtualRegisterId,
};
use semantic_vocabulary::PlaceId;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::StructuralPlaceDeclaration;

use super::{StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, extent_intersects, extent_reached_by,
    local_slot_is_place_storage, slot_is_subject_storage, staging_slot,
    structural_place_declarations,
};

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

/// The bytes the moved store writes within one place root, re-decoded by
/// the validator from the store's own roster row: an exact range, or a
/// dynamic extent when the store is a byte-sequence write — its single
/// written byte sits at `byte_offset + index` for the runtime `index`, so
/// every byte it can touch lies at or after `byte_offset` with no static
/// upper bound. When that `index` itself resolves to a clean materialized
/// constant, the audit collapses the extent to the one byte
/// `byte_offset + index` before the walk: the moved byte's position is
/// then fixed, and every interference check below decides on it. `place`
/// is the place the moved row names — the place the staged bytes name
/// when the subject is a staging slot — while `storage` settles which
/// bytes the walk actually tracks.
struct Moved {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    /// The byte-sequence store's runtime index, deciding the written
    /// byte's position: `byte_offset + index`. `None` for an exact store,
    /// and for a byte-sequence store whose index resolved — its moved
    /// byte is the collapsed `byte_offset` then.
    sequence_index: Option<semantic_vocabulary::ValueId>,
    storage: SubjectStorage,
}

impl Moved {
    /// Exact rows intersect when their half-open byte intervals share a
    /// byte. A dynamic moved extent is unbounded upward from `byte_offset`,
    /// so the exact row interferes once its own extent ends past that
    /// offset — ending at or below it is the only provable disjointness.
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

/// Whether one roster row touches the moved bytes or the subject's
/// storage — the validator's own interference decision over the validated
/// access roster. Exact rows must intersect the moved range — when the
/// moved extent is itself dynamic, an exact row interferes once its
/// extent ends past the row's fixed offset, the only provable
/// disjointness left. A dynamic-extent row on the moved place reaches
/// only upward from its fixed offset, so it interferes exactly while
/// that offset starts below the moved range's end; a byte-sequence row
/// whose resolved index lands outside the moved extent walks past
/// wherever its payload base sits, while against a dynamic moved extent
/// an unresolved row always interferes and a resolved one interferes only
/// at or past the moved payload base. A `WriteLocal` row names an exact
/// range on a slot: when the slot is the moved bytes' storage, range
/// intersection decides; when the slot is any other — a staging slot
/// beside a place subject, or a different slot beside a staging subject —
/// its bytes are disjoint, so the row never stops the walk. A
/// materialized local address could reach the same storage by a route
/// the roster does not bound, so it stops the walk when its slot is the
/// moved bytes' storage; any other slot's address reaches only that
/// slot's bytes. For a staging subject the place-named rows never
/// interfere either: they describe the place's own extents — the staged
/// bytes are not the place's at any offset — and no place route carries
/// a slot's bytes. Outgoing-area storage never aliases a referent place.
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

/// Register locations an operand reads or writes. `UseDef` participates
/// in both directions, matching the access marks the constraint rows
/// publish.
fn reads(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

fn writes(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}

fn register_reads(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| reads(operand.access))
        .map(|operand| operand.virtual_register)
}

fn register_writes(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| writes(operand.access))
        .map(|operand| operand.virtual_register)
}

/// Whether `writer`'s writes meet `reader`'s reads on a register or a
/// condition-state unit — the validator's own read of the two operand
/// surfaces. Implicit uses read units; implicit definitions and clobbers
/// write them, so flag publishers and flag consumers couple the same way
/// explicit operands do.
fn writes_meet_reads(writer: &SelectedInstruction, reader: &SelectedInstruction) -> bool {
    register_writes(writer)
        .any(|register| register_reads(reader).any(|candidate| candidate == register))
        || writer
            .implicit_defs
            .iter()
            .chain(writer.clobbers.iter())
            .any(|unit| reader.implicit_uses.contains(unit))
}

/// Whether sliding the store past `later` reorders a register or
/// condition-state location between them — the validator's own coupling
/// audit. The three hazards cover every pair a move passes: the moved
/// store defining a location the candidate reads would starve it or hand
/// it a different value, the store reading a location the candidate
/// writes would hand it the new value, and a shared written location —
/// register or condition-state unit — would change which definition
/// later positions observe.
fn coupled(earlier: &SelectedInstruction, later: &SelectedInstruction) -> bool {
    writes_meet_reads(earlier, later)
        || writes_meet_reads(later, earlier)
        || register_writes(earlier)
            .any(|register| register_writes(later).any(|candidate| candidate == register))
        || earlier
            .implicit_defs
            .iter()
            .chain(earlier.clobbers.iter())
            .any(|unit| later.implicit_defs.contains(unit) || later.clobbers.contains(unit))
}

/// Calls, hosted effects, floating-control saves and restores, and
/// terminator kinds always bound the slide: they can observe or expose
/// reachable state regardless of their roster rows, and a terminator
/// kind never belongs in a block body — the validator's own barrier
/// list, not the producer's.
fn barrier_kind(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    matches!(
        instruction.kind,
        CallUnit { .. }
            | CallScalar { .. }
            | CallAggregate { .. }
            | HostedReadByte { .. }
            | HostedWriteByteI32 { .. }
            | SaveFloatingControl { .. }
            | RestoreFloatingControl { .. }
            | HostedExitProcessI32
            | ReturnScalar
            | ReturnAggregate { .. }
            | ReturnUnit
            | Jump
            | ConditionalBranchNonZero
            | ConditionalBranchU64LessThan
            | ConditionalBranchI64LessThan
    )
}

/// An interval instruction without a roster row must be unable to reach
/// any semantic or place-backed storage — the validator's own accounting
/// decision. Private-slot frame accesses touch compiler-owned
/// spill/boundary slots that no referent place aliases, and pure
/// register work has no memory side at all. Any other memory-capable
/// kind without a row is an unaccounted access whose reach the slide
/// cannot prove.
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

/// Whether sliding the store past this instruction would change the
/// observed order of the write — the validator's own stop decision. Any
/// of the stop conditions bound the window: a roster row touching the
/// moved bytes, a barrier kind, the full register and condition-state
/// coupling between the two instructions — the candidate redefining a
/// carried read, or reading or rewriting the moved store's own scratch
/// definition or declared clobbers — or an unaccounted memory reach.
fn stops_slide(
    candidate: &SelectedInstruction,
    moved: &Moved,
    function: &SelectedFunction,
    moved_store: &SelectedInstruction,
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
    barrier_kind(candidate)
        || coupled(moved_store, candidate)
        || (!has_row && unaccounted_kind(candidate))
}

/// A boundary settlement at a position in the crossed interval would
/// observe the place before the moved write instead of after it.
fn settlement_before(function: &SelectedFunction, block: SelectedBlockId, position: usize) -> bool {
    function.boundary_settlements.iter().any(|settlement| {
        settlement.block == block && settlement.instruction_index as usize == position
    })
}

/// Whether `block`'s only predecessor block is `expected` — the
/// validator's own predecessor audit. Every other block's terminator is
/// scanned for an edge naming it, so a second incoming edge or a
/// self-loop keeps the join visible.
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

/// Whether `start` can reach any block the walk already crossed — the
/// validator's own cycle audit. A return path closes a cycle containing
/// both the store's old and new positions; the unverified half of that
/// cycle could reorder a same-place access across the moved write.
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

/// The register locations the move orders against the window — the
/// validator's own record of the moved store's surface: the registers
/// the moved store reads — the referent pointer or materialized slot
/// address plus the stored value for a place store, or just the stored
/// value for the direct `Store64` — and the registers it writes, the
/// packed form's early-clobber scratch. An edge transport naming a
/// written register in either direction would move a read or a second
/// write across the moved definition, so the crossed-edge check below
/// needs both directions; the per-instruction and terminator hazards ask
/// `coupled`, which reads the store's whole write surface — this `Def`
/// plus any implicit definitions and clobbers — directly off the
/// instruction.
struct Carried {
    registers: Vec<VirtualRegisterId>,
    writes: Vec<VirtualRegisterId>,
}

/// Collect the moved store's register surface — the validator's own
/// operand-position audit. Every operand must sit at the position its
/// index declares; a use joins the carried reads while the packed form's
/// scratch `Def` joins the writes. A definition on any other form, or
/// implicit writes or clobbers on a non-packed store, is a write surface
/// the route never declared and stays inadmissible.
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

/// The moved store's operand surface — the validator's own surface
/// audit: the target's `[use pointer, use value]` row with nothing
/// implicit attached.
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

/// The direct slot store's operand surface — the validator's own
/// surface audit: the target's declared `store64` row, exactly
/// `[use value]`, and the instruction carrying just that one use. The
/// move defines nothing, so no custody check is needed.
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

/// The moved packed store's operand surface — the validator's own
/// surface audit: the target's declared `store_packed` row,
/// `[use pointer, use packed value]` plus the early-clobber scratch
/// `Def` the multi-instruction store writes through, and the
/// instruction carrying exactly that row. The move keeps the definition
/// alive — it only needs the scratch's custody inside the walked
/// window, which the write-side coupling check proves. The row's
/// implicit use and definition lists stay empty; whatever the row
/// declares as clobbers — the flag unit on a flag-publishing target —
/// moves with the instruction and is guarded the same way.
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
    if row.operands.len() != 3
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[2].operand != 2
        || row.operands[2].access != RegisterOperandAccess::Def
        || !row.operands[2].early_clobber
        || row.operands[0].early_clobber
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

/// A crossed edge must not move the carried registers or touch the
/// moved bytes' storage — the validator's own edge audit. A transport's
/// `parameter` slot defines a register for the successor while its
/// `argument` slot reads one on the edge: a parameter naming a carried
/// read still stops the crossing, and either slot naming a carried
/// write — the packed scratch — would move a use or a second definition
/// across the moved store's own definition. Structural destinations,
/// the case custody slot, and custody discards write or retire storage
/// on the edge — the same conservative test the sibling memory walks
/// keep on edges, since an edge transport's destination role is not
/// decided here. For a staging subject the transport destination and
/// custody slot decide on the slot itself — staging bytes sit under
/// their own slot's coordinates — while a place custody discard retires
/// the place's storage, never the operation-owned slot.
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
        let (argument, destination) = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue {
                argument,
                destination,
                ..
            }
            | SelectedStructuralTransport::Descriptor {
                argument,
                destination,
            } => (argument, destination),
        };
        let touches = match moved.storage {
            SubjectStorage::Place => destination.structural_place() == Some(moved.place),
            SubjectStorage::Staging(slot) => destination == slot,
        };
        if touches || writes(&argument) {
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
        local_store_shape(moved_store, environment)?;
    } else {
        place_store_shape(moved_store, environment)?;
    }
    let carried = carried_surface(moved_store, packed)?;
    // The validator's own forward walk to the latest provable position.
    // Scanning a block stops before the first instruction or settlement
    // position that must stay ordered after the store; reaching a block's
    // end cleanly crosses only a unique-successor, unique-predecessor,
    // acyclic edge. Every failed crossing lands the store at the crossed
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
            if stops_slide(candidate, &moved, function, moved_store, structural_places)
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
        // Every path forward must reach one block: edges to distinct
        // blocks admit a path the moved write never runs on.
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
