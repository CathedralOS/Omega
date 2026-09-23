//! Independent validation of dead-store elimination.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! store shape against the bound constraint row, the write row's place and
//! byte extent (or the fixed bytes a resolved `index`/`length` collapses
//! the dynamic extent to), the forward walk that must find one covering
//! write on every reachable path or confine the path to clear blocks
//! forever, the crossed-edge and boundary-settlement audits, and the
//! dropped-row contract — then rebuilds the function the contract demands
//! and requires the proposal to equal it. Restoring the store and its
//! roster rows must reproduce the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedCasePayloadTransport,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedStructuralTransport, SelectedSuccessor, SelectedValueTransport, VirtualRegisterOrigin,
};
use semantic_vocabulary::PlaceId;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::StructuralPlaceDeclaration;

use super::{
    DeadStoreEliminationError, DeadStoreEliminationReceipt, ValidatedDeadStoreElimination,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::unexecuted::condition_state::materialized_bits;
use crate::rewrites::unexecuted::place_storage::{
    SubjectStorage, constant_index, extent_intersects, extent_reached_by,
    local_slot_is_place_storage, slot_is_subject_storage, staging_slot,
    structural_place_declarations, transport_defines,
};

/// The validator's own reconstruction of the removal the contract permits:
/// the admitted store's coordinates, the roster rows the removal drops,
/// and the walked interval the measured-step contract charges. It shares
/// no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    block: SelectedBlockId,
    store_index: usize,
    /// Indices of the dead store's roster rows in `memory_accesses`, in
    /// roster order — the one write row for every route but the `CopyBytes`,
    /// whose source read row drops with the instruction beside its
    /// destination span row. Replay requires the proposed roster to drop
    /// exactly these rows.
    store_accesses: Vec<usize>,
    /// The scanned positions and crossed edges the walk charged: every
    /// cleared span's instructions, each covering block's span through its
    /// covering store, and one step per crossed successor edge.
    interval: usize,
}

/// The bytes the removed store wrote within one place root, re-decoded by
/// the validator from the store's roster row alone: an exact range, or a
/// dynamic extent whose runtime decider places it anywhere at or after
/// `byte_offset` — a byte-sequence store's one byte at
/// `byte_offset + index`, or a `CopyBytes` destination span's `length`
/// bytes at `byte_offset`. When the decider resolves to a clean
/// materialized constant the audit collapses the extent to the fixed bytes
/// the constant names before the walk, and every interference and coverage
/// check below decides on a fixed position. `place` is the place the dead
/// row names — the place the staged bytes name when the subject is a
/// staging slot — while `storage` settles which bytes the walk tracks.
struct Subject {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    reach: SubjectReach,
    storage: SubjectStorage,
}

/// How a dynamic dead extent's reach is decided at runtime: the
/// byte-sequence store's `index` places its one byte at
/// `byte_offset + index`, and the `CopyBytes` destination span's `length`
/// is the number of bytes it writes at `byte_offset`. `Fixed` carries no
/// decider — the dead range is `byte_count` bytes at `byte_offset`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SubjectReach {
    Fixed,
    SequenceByte(semantic_vocabulary::ValueId),
    ByteSpan(semantic_vocabulary::ValueId),
}

impl SubjectReach {
    /// Whether the dead store's written bytes sit anywhere at or after
    /// `byte_offset` with no static upper bound — the interference shape
    /// both dynamic reaches share.
    fn dynamic(&self) -> bool {
        !matches!(self, SubjectReach::Fixed)
    }
}

impl Subject {
    /// Exact rows intersect when their half-open byte intervals share a
    /// byte. A dynamic dead reach is unbounded upward from `byte_offset`,
    /// so the exact row still reaches the written bytes once its own
    /// extent ends past that offset — ending at or below it is the only
    /// provable disjointness.
    fn overlaps(&self, access: &SelectedMemoryAccess) -> bool {
        extent_intersects(
            self.byte_offset,
            self.byte_count,
            self.reach.dynamic(),
            access,
        )
    }

    /// Whether `access` can reach the dead extent — the shared dynamic-reach
    /// and resolved-landing decision in `place_storage`.
    fn reached_by(&self, access: &SelectedMemoryAccess, function: &SelectedFunction) -> bool {
        extent_reached_by(
            self.byte_offset,
            self.byte_count,
            self.reach.dynamic(),
            access,
            function,
        )
    }
}

/// Whether one roster row can observe the dead bytes or leave them
/// observable — the validator's own interference decision over the
/// validated access roster. Reads must intersect the dead range; writes
/// must target the same place root to overlap, and the covering check then
/// decides whether the write qualifies. A dynamic-extent row on the dead
/// place reaches only upward from its fixed offset, so it interferes
/// exactly while that offset starts below the dead range's end; a row
/// starting at or past the end is provably disjoint and walks past — as is
/// a byte-sequence row whose resolved index lands it outside the range
/// entirely. When the dead extent is itself dynamic the directions
/// mirror: an exact or local row still reaches the dead byte once its own
/// extent ends past the row's fixed offset, and a dynamic-extent row on
/// the dead place always meets it.
/// A `WriteLocal` row names an exact range on a slot: when the slot is the
/// dead bytes' storage, range intersection decides and an intersecting row
/// still has to cover; when the slot is any other — a staging slot beside
/// a place subject, or a different slot beside a staging subject — its
/// bytes are disjoint, so the row never interferes. A materialized local
/// address could reach the same storage by a route the roster does not
/// bound, so it interferes when its slot is the dead bytes' storage; any
/// other slot's address reaches only that slot's bytes. For a staging
/// subject the place-named rows never interfere either: they describe the
/// place's own extents — the staged bytes are not the place's at any
/// offset — and no place route carries a slot's bytes. Outgoing-area
/// storage never aliases a referent place.
fn disturbs(
    subject: &Subject,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
    function: &SelectedFunction,
) -> bool {
    use SelectedMemoryAccessRole::*;
    match access.role {
        ReadPlace | WritePlace => {
            matches!(subject.storage, SubjectStorage::Place)
                && access.place == subject.place
                && subject.overlaps(access)
        }
        ReadByteSpan { .. }
        | ReadByteSequence { .. }
        | ReadElementView { .. }
        | WriteByteSpan { .. }
        | WriteByteSequence { .. }
        | WriteIndexedPrimitive { .. } => {
            matches!(subject.storage, SubjectStorage::Place)
                && access.place == subject.place
                && subject.reached_by(access, function)
        }
        WriteLocal { slot } => {
            slot_is_subject_storage(slot, subject.storage, subject.place, structural_places)
                && subject.overlaps(access)
        }
        AddressLocal { slot } => {
            slot_is_subject_storage(slot, subject.storage, subject.place, structural_places)
        }
        WriteOutgoing { .. } | AddressOutgoing { .. } => false,
    }
}

/// The named store and the covering store share one operand surface: the
/// target's `[use pointer, use value]` row with nothing implicit attached.
fn store_row(
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
fn packed_store_row(
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
    if !scratch_dies(function, instruction.operands[2].virtual_register) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}

/// The direct slot store's operand surface: the target's declared `store64`
/// row — exactly `[use value]` — and the instruction carrying just that one
/// use. Removing it removes no register definition, so no custody check like
/// the packed scratch's is needed.
fn local_store_row(
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
fn byte_span_row(
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
        || !scratch_dies(function, instruction.operands[3].virtual_register)
        || !scratch_dies(function, instruction.operands[4].virtual_register)
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
/// the custody a dropped early-clobber scratch requires. The operand itself
/// is that one occurrence: any other operand position, terminator operand,
/// or successor transport naming the register would leave the elimination
/// removing a definition a surviving read or second definition still
/// observes.
fn scratch_dies(
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
                        SelectedCasePayloadTransport::Unused => false,
                        SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                            *parameter == register
                        }
                        SelectedCasePayloadTransport::Registers {
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
                    SelectedValueTransport::Unused => false,
                    SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == register || *parameter == register,
                })
                .count();
        }
    }
    occurrences == 1
}

/// The found access must be a write of the dead bytes' storage whose single
/// roster row covers the dead range entirely and names the same bytes the
/// instruction encodes — the validator's own re-decision of the covering
/// routes:
/// - `Store` of any exact width or packed `StorePacked` carrying
///   `WritePlace` — through a place pointer — or `WriteLocal` on the dead
///   bytes' own slot, through that slot's materialized address: the
///   place's storage slot for a place subject, the staging slot itself for
///   a staging one;
/// - `Store64` into `Local(slot)` carrying `WriteLocal` on that same slot —
///   directly into the dead bytes' storage;
/// - `CopyBytes` carrying the destination `WriteByteSpan` — a
///   dynamic-extent row that can cover an exact dead range, and only when
///   the span's reach is itself exact: the count register's sole
///   definition must be a clean `MaterializeI64`, so the span writes a
///   compile-time `count` bytes at its fixed `byte_offset`. The copy's
///   other rows must stay quiet on the dead range, since a read reaching
///   it would observe the bytes before the write rewrites them;
/// - `Store { 0, 1 }` carrying `WriteByteSequence` — the other
///   dynamic-extent row that can cover an exact dead range, and only when
///   the index resolves exact the same way: the write lands on the one
///   fixed byte `byte_offset + index`, which must be the single dead byte;
/// - for a byte-sequence dead store whose `index` stays runtime, another
///   `Store { 0, 1 }` carrying `WriteByteSequence` — the only write that
///   can provably land on the dead byte: same payload base and same
///   runtime `index` spell the same position, and distinct index values
///   still do when both resolve to constants whose `byte_offset + index`
///   sums agree. Anything else may land on a different byte entirely.
///
/// A write that only partially overlaps the dead range leaves the
/// remaining bytes observable. For a place subject a `WriteLocal` on an
/// operation-owned `Structural` slot covers only when the place's
/// declaration names that operation as the slot's producer; for a staging
/// subject the covering slot is the staging slot itself. A `WritePlace`
/// never reaches a staging slot's bytes — it writes through the place's
/// referent pointer — so the staging subject's only covering route is a
/// `WriteLocal` naming that slot.
fn covers_dead(
    instruction: &SelectedInstruction,
    subject: &Subject,
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
        return span_covers(
            instruction,
            subject,
            function,
            environment,
            structural_places,
        );
    }
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(reject());
    };
    if rows.next().is_some() || row.place != subject.place {
        return Err(reject());
    }
    // An unresolved `CopyBytes` dead store wrote `length` bytes at its
    // span's `byte_offset` — an extent unbounded upward that only another
    // `CopyBytes` spelling the same extent could rewrite; that route
    // returned above, so no other write kind can cover it.
    if matches!(subject.reach, SubjectReach::ByteSpan(_)) {
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
    if let SubjectReach::SequenceByte(index) = subject.reach {
        if !matches!(
            instruction.kind,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 1
            }
        ) {
            return Err(reject());
        }
        store_row(instruction, environment)?;
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
            return if row.byte_offset == subject.byte_offset {
                Ok(())
            } else {
                Err(reject())
            };
        }
        let dead_byte = u64::from(subject.byte_offset)
            .checked_add(constant_index(function, index).ok_or_else(reject)?)
            .ok_or_else(reject)?;
        let written = u64::from(row.byte_offset)
            .checked_add(constant_index(function, covering).ok_or_else(reject)?)
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
        return sequence_covers(instruction, subject, row, index, function, environment);
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
            store_row(instruction, environment)?;
            match row.role {
                SelectedMemoryAccessRole::WritePlace
                    if matches!(subject.storage, SubjectStorage::Place) => {}
                SelectedMemoryAccessRole::WriteLocal { slot }
                    if slot_is_subject_storage(
                        slot,
                        subject.storage,
                        subject.place,
                        structural_places,
                    ) => {}
                _ => return Err(reject()),
            }
            (byte_offset, u32::from(byte_size))
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            if environment.selected_keys().store_packed != Some(instruction.constraint) {
                return Err(DeadStoreEliminationError::ConstraintMismatch);
            }
            match row.role {
                SelectedMemoryAccessRole::WritePlace
                    if matches!(subject.storage, SubjectStorage::Place) => {}
                SelectedMemoryAccessRole::WriteLocal { slot }
                    if slot_is_subject_storage(
                        slot,
                        subject.storage,
                        subject.place,
                        structural_places,
                    ) => {}
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
            // same slot and that slot is the dead bytes' own storage.
            if row.role != (SelectedMemoryAccessRole::WriteLocal { slot })
                || !slot_is_subject_storage(slot, subject.storage, subject.place, structural_places)
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
    if row.byte_offset > subject.byte_offset
        || u64::from(row.byte_offset) + u64::from(row.byte_count)
            < u64::from(subject.byte_offset) + u64::from(subject.byte_count)
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
/// Coverage is containment on the resolved constants: the span starts at
/// or before the dead range and its constant extent reaches the dead end.
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
fn span_covers(
    instruction: &SelectedInstruction,
    subject: &Subject,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
    structural_places: &[StructuralPlaceDeclaration],
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    if matches!(subject.reach, SubjectReach::SequenceByte(_)) {
        return Err(reject());
    }
    let mut covering = None;
    for access in function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id)
    {
        if !disturbs(subject, access, structural_places, function) {
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
    // is contractual zero — the runtime `length` is the authoritative
    // reach.
    let SelectedMemoryAccessRole::WriteByteSpan { length, .. } = row.role else {
        return Err(reject());
    };
    if row.byte_count != 0 {
        return Err(reject());
    }
    // The target's declared `copy_bytes` row pins operand 2 as the count
    // use, so the register named there is the extent the instruction
    // writes.
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
    if let SubjectReach::ByteSpan(dead_length) = subject.reach {
        return if row.byte_offset == subject.byte_offset && length == dead_length {
            Ok(())
        } else {
            Err(reject())
        };
    }
    let written = materialized_bits(function, count).map_err(|_| reject())?;
    if transport_defines(function, count) {
        return Err(reject());
    }
    if row.byte_offset > subject.byte_offset {
        return Err(reject());
    }
    let needed =
        u64::from(subject.byte_offset) + u64::from(subject.byte_count) - u64::from(row.byte_offset);
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
/// exactly when it is that one byte. A runtime index lands the write
/// anywhere at or past the payload base, so it can never provably rewrite
/// one fixed byte; a resolved index landing anywhere but the dead byte
/// leaves it observable.
fn sequence_covers(
    instruction: &SelectedInstruction,
    subject: &Subject,
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
    store_row(instruction, environment)?;
    let landed = constant_index(function, index).ok_or_else(reject)?;
    // The write lands on one byte; the dead range is covered exactly when
    // it is that byte.
    if subject.byte_count != 1
        || u64::from(row.byte_offset)
            .checked_add(landed)
            .ok_or_else(reject)?
            != u64::from(subject.byte_offset)
    {
        return Err(reject());
    }
    Ok(())
}

/// Calls, hosted effects, and terminator kinds are always barriers: they can
/// observe or expose reachable storage regardless of their roster rows, and a
/// terminator kind never belongs in a block body.
fn barrier(instruction: &SelectedInstruction) -> Result<(), DeadStoreEliminationError> {
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

/// A walked instruction without a roster row must be unable to reach any
/// semantic or place-backed storage: loads only observe, address forms only
/// compute, private-slot frame stores touch compiler-owned spill/boundary
/// slots, and pure register work has no memory side at all. A referent
/// store or any other frame slot without its row is an unaccounted access.
fn unaccounted(instruction: &SelectedInstruction) -> Result<(), DeadStoreEliminationError> {
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
        | MaterializeBooleanI64LessOrEqual => Ok(()),
        Store { .. } | StorePacked { .. } | Store64 { .. } | CopyBytes => {
            Err(DeadStoreEliminationError::InterveningAccess)
        }
        _ => Err(DeadStoreEliminationError::UnsupportedInstruction),
    }
}

/// A crossed edge must not touch the dead bytes' storage — the validator's
/// own edge audit. Register transports cannot reach memory, but a
/// structural destination, the case custody slot, or a custody discard
/// naming the dead place writes or retires its bytes inside the dead
/// interval. For a staging subject the transport destination and custody
/// slot decide on the slot itself — staging bytes sit under their own
/// slot's coordinates — while a place custody discard retires the place's
/// storage, never the operation-owned slot.
fn edge_keeps(
    successor: &SelectedSuccessor,
    subject: &Subject,
) -> Result<(), DeadStoreEliminationError> {
    for binding in &successor.structural_bindings {
        // An address transport also lends its root: the referent stays
        // observable through the joined pointer, so the root counts as read.
        let (destination, lent) = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue { destination, .. }
            | SelectedStructuralTransport::Descriptor { destination, .. } => (destination, None),
            SelectedStructuralTransport::Address {
                base, destination, ..
            } => (destination, Some(base)),
        };
        let touches = match subject.storage {
            SubjectStorage::Place => {
                destination.structural_place() == Some(subject.place)
                    || (lent.is_some() && binding.semantic.argument.place == subject.place)
            }
            SubjectStorage::Staging(slot) => {
                destination == slot
                    || lent == Some(selected_instructions::SelectedAddressBase::Local(slot))
            }
        };
        if touches {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
    }
    if let Some(case) = &successor.structural_case {
        let slot_touches = match subject.storage {
            SubjectStorage::Place => case.slot.structural_place() == Some(subject.place),
            SubjectStorage::Staging(slot) => case.slot == slot,
        };
        let discard_touches = matches!(subject.storage, SubjectStorage::Place)
            && case.trivial_affine_discards.contains(&subject.place);
        if slot_touches || discard_touches {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
    }
    Ok(())
}

/// Re-derive the elimination's legality from the source records, without
/// the producer's `admission` routine: locate the named store, decode the
/// written bytes from its roster row, audit the operand surface the
/// removal drops, then walk forward along every path — the first access
/// that can reach the dead bytes must be a covering write, and a block
/// scanned clear defers coverage to its distinct successors. A legality
/// error surfaces here even when the proposal matches the edit the
/// producer emitted.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, DeadStoreEliminationError> {
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
    // slot directly; or the byte-sequence `Store { 0, 1 }` through a fully
    // computed view address. The `CopyBytes` dead store writes a dynamic
    // extent instead: `length` bytes at its destination span's fixed
    // `byte_offset`, carrying the copy's source `ReadByteSpan` beside it on
    // the same instruction.
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
    // to the place's storage. The `CopyBytes` row is its destination
    // `WriteByteSpan` and the instruction's other rows are the copy's
    // reads, which the removal drops beside it.
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
                | SelectedMemoryAccessRole::ReadByteSequence { .. }
                | SelectedMemoryAccessRole::ReadElementView { .. } => {}
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
    let reach = match write.role {
        SelectedMemoryAccessRole::WriteByteSequence { index, .. } => {
            SubjectReach::SequenceByte(index)
        }
        SelectedMemoryAccessRole::WriteByteSpan { length, .. } => SubjectReach::ByteSpan(length),
        _ => SubjectReach::Fixed,
    };
    // The `WriteLocal` routes can also name a staging slot: when the slot
    // is not the row place's own storage it stages bytes that merely name
    // the place, and the write is the dead store of the slot's own bytes —
    // the staging subject `Subject::storage` records below. `staging_slot`
    // still requires the row's place to be the slot's staged place: a
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
        // A `WriteByteSequence` names a payload base, not the written
        // position: the row is the dead store's write only on the
        // one-byte `Store { 0, 1 }` whose byte lands at `byte_offset +
        // index`.
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
        || (!reach.dynamic()
            && (write.byte_offset != encoded_offset || write.byte_count != encoded_size))
    {
        return Err(DeadStoreEliminationError::UnsupportedPair);
    }
    let mut subject = Subject {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
        reach,
        storage: staging.map_or(SubjectStorage::Place, SubjectStorage::Staging),
    };
    // A dynamic dead extent whose own decider resolves through the same
    // carrier audit the covering routes run — sole `InstructionResult`
    // carrier, clean `MaterializeI64` definition, no edge-transport or
    // case-payload redefinition — collapses to fixed bytes before the
    // walk: the byte-sequence store's one byte at `byte_offset + index`,
    // or the copy's `length` bytes at `byte_offset`. Every check below
    // then decides on a fixed position, while an unresolved decider, or
    // an extent no u32 names, leaves the reach unbounded upward from
    // `byte_offset`.
    match subject.reach {
        SubjectReach::SequenceByte(index) => {
            if let Some(landed) = constant_index(function, index)
                && let Some(position) = u64::from(subject.byte_offset).checked_add(landed)
                && let Ok(position) = u32::try_from(position)
            {
                subject.byte_offset = position;
                subject.byte_count = 1;
                subject.reach = SubjectReach::Fixed;
            }
        }
        SubjectReach::ByteSpan(length) => {
            if let Some(written) = constant_index(function, length)
                && let Ok(count) = u32::try_from(written)
            {
                subject.byte_count = count;
                subject.reach = SubjectReach::Fixed;
            }
        }
        SubjectReach::Fixed => {}
    }
    // The removed instruction must keep the operand surface its kind
    // declares: the plain two-use place store, the packed store's
    // two-use-plus-dead-scratch row, the direct slot store's single-use
    // row, or the copy's three-use-plus-two-dead-scratch row. An exotic
    // operand surface would make the removal contract unclear.
    if packed {
        packed_store_row(dead_store, function, environment)?;
    } else if direct_slot.is_some() {
        local_store_row(dead_store, environment)?;
    } else if byte_span {
        byte_span_row(dead_store, write, function, environment)?;
    } else {
        store_row(dead_store, environment)?;
    }
    // The copy's source read rides on the removed instruction beside its
    // destination span, so no row of the dead copy may reach the dead
    // extent: a source span overlapping the destination it feeds would
    // leave the removal contract guessing which bytes the copy observed.
    if byte_span
        && function.memory_accesses.iter().any(|access| {
            access.instruction == store
                && !matches!(access.role, SelectedMemoryAccessRole::WriteByteSpan { .. })
                && disturbs(&subject, access, structural_places, function)
        })
    {
        return Err(DeadStoreEliminationError::UnsupportedPair);
    }
    // The validator's own forward walk: scan each reached block to the
    // first access that can reach the dead bytes — a covering write
    // resolves the block, anything else rejects — and a block whose walked
    // span stays clear defers to every successor edge. A block is only
    // ever entered uncovered, so its first interfering access decides all
    // paths through it at once; a completed walk needs no all-paths
    // coverage proof because a path that never reaches a covering write is
    // confined to clear blocks forever and can never observe the dead
    // bytes.
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
            barrier(candidate)?;
            let mut has_row = false;
            let mut interfered = false;
            for access in function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == candidate.id)
            {
                has_row = true;
                interfered |= disturbs(&subject, access, structural_places, function);
            }
            if interfered {
                covers_dead(candidate, &subject, function, environment)?;
                found = Some(candidate_index);
                break;
            }
            if !has_row {
                unaccounted(candidate)?;
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
                && disturbs(&subject, access, structural_places, function)
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
            edge_keeps(edge, &subject)?;
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
            // removed store from its top — it can never be its own
            // covering write — so the looped path stays unproven.
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
    // A boundary settlement positioned inside the dead interval is an
    // event a boundary could observe the still-current bytes through;
    // positions outside it only shift. The interval covers the store's
    // block after the removed store, every fully crossed block, and each
    // covering block through its covering store.
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
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        block: block.id,
        store_index,
        store_accesses,
        interval,
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction across
/// the plan, the walked interval's scanned positions and crossed edges, and
/// one step per roster row.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    interval: usize,
) -> Result<u64, DeadStoreEliminationError> {
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
    u64::try_from(steps).map_err(|_| DeadStoreEliminationError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own record:
/// the named store leaves its block, boundary settlements shift over the
/// removed ordinal, and the roster drops exactly the dead store's rows.
/// The producer's `rewrite` is not consulted; both sides derive the same
/// function from the source alone.
fn expect(
    reconstructed: &Reconstructed<'_>,
) -> Result<SelectedFunction, DeadStoreEliminationError> {
    let mut expected = reconstructed.function.clone();
    let body = {
        let block = expected
            .blocks
            .get_mut(reconstructed.block_index)
            .ok_or(DeadStoreEliminationError::SourceMismatch)?;
        if block.id != reconstructed.block {
            return Err(DeadStoreEliminationError::SourceMismatch);
        }
        block.instructions.len()
    };
    // Positions at or before the removed ordinal name instructions that
    // stay put; every later position — including the after-body position —
    // shifts one ordinal earlier.
    expected.boundary_settlements = expected
        .boundary_settlements
        .into_iter()
        .map(|mut settlement| {
            if settlement.block == reconstructed.block {
                let position = settlement.instruction_index as usize;
                if position > body {
                    return Err(DeadStoreEliminationError::SourceMismatch);
                }
                if position > reconstructed.store_index {
                    settlement.instruction_index = u32::try_from(position - 1)
                        .map_err(|_| DeadStoreEliminationError::IdentityOverflow)?;
                }
            }
            Ok(settlement)
        })
        .collect::<Result<Vec<_>, _>>()?;
    expected.blocks[reconstructed.block_index]
        .instructions
        .remove(reconstructed.store_index);
    // The dead store's rows drop together — a `CopyBytes` carries its
    // source read beside the destination span. The indices are roster
    // order, so removing highest-first keeps the lower indices valid.
    for &row in reconstructed.store_accesses.iter().rev() {
        expected.memory_accesses.remove(row);
    }
    Ok(expected)
}

/// Reinsert the store and its roster rows and restore the source
/// settlements: undoing the validator's expected edit must restore the
/// complete source by content — every other instruction, register, call,
/// and function included. Every removed row reinserts at its own source
/// index, lowest first: the rows below a restored position are already
/// back in place, so each source ordinal is where its row belongs again.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, DeadStoreEliminationError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(DeadStoreEliminationError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(DeadStoreEliminationError::ReplayMismatch)?;
    if block.id != reconstructed.block || block.instructions.len() < reconstructed.store_index {
        return Err(DeadStoreEliminationError::ReplayMismatch);
    }
    block.instructions.insert(
        reconstructed.store_index,
        reconstructed.function.blocks[reconstructed.block_index].instructions
            [reconstructed.store_index]
            .clone(),
    );
    for &row in &reconstructed.store_accesses {
        if row > function.memory_accesses.len() {
            return Err(DeadStoreEliminationError::ReplayMismatch);
        }
        function
            .memory_accesses
            .insert(row, reconstructed.function.memory_accesses[row].clone());
    }
    // The proposed settlements already matched the shifted source, so
    // restoring the source rows restores the source function.
    function.boundary_settlements = reconstructed.function.boundary_settlements.clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the removal's preconditions from the source, requires the proposal to
/// equal the function its own record produces, and restores the complete
/// source by content — every other instruction, register, call, and
/// settlement included. The producer's admission routine is never
/// consulted, so a wrong legality decision fails here even when the
/// proposal matches the edit the producer emitted.
pub fn validate_dead_store_elimination(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedDeadStoreElimination, DeadStoreEliminationError> {
    let reconstructed = reconstruct(source, function_index, store, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        reconstructed.interval,
    )? > budget.validation_steps()
    {
        return Err(DeadStoreEliminationError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)?) {
        return Err(DeadStoreEliminationError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(DeadStoreEliminationError::ReplayMismatch);
    }
    Ok(ValidatedDeadStoreElimination {
        receipt: DeadStoreEliminationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
