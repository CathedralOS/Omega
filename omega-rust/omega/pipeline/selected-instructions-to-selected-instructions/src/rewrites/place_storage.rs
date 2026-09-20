//! Which local storage slot is a structural place's own storage, shared by
//! the rewrites that reason about the bytes a place owns.
//!
//! This module is the single owner for the byte-extent reasoning the
//! rewrites once copied per rule: a dead store's written bytes, a
//! forwarded load's read bytes, and a moved store's written span all pose
//! the same questions — does a roster row's access intersect the subject's
//! extent (`extent_intersects`), reach it (`extent_reached_by`), or name
//! the subject's own storage (`extent_interferes`). `SubjectExtent` carries
//! the subject's span — `dynamic` when an unresolved runtime index or span
//! length leaves its reach unbounded upward from `byte_offset` — and
//! `SubjectStorage` carries which storage the subject tracks. The store
//! row-shape checks (`local_store_shape`, `place_store_row_shape`,
//! `packed_store_row_shape`) admit the store instruction's operand surface
//! once; each rewrite composes its own custody policy on the admitted row.
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    LocalStorageSlotId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
    SelectedMemoryAccess, SelectedMemoryAccessRole, SelectedValueTransport, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, ValueId};
use terminal_psi::StructuralPlaceDeclaration;

use crate::rewrites::block_edges::terminator_successors;
use crate::rewrites::condition_state::materialized_bits;

/// The function's declared structural places — the producer evidence the
/// `Structural` slot check needs. A function without a structural contract
/// declares none, leaving every `Structural` slot a staging slot.
pub(super) fn structural_place_declarations(
    function: &SelectedFunction,
) -> &[StructuralPlaceDeclaration] {
    function
        .structural
        .as_ref()
        .map_or(&[], |contract| contract.structural_places.as_slice())
}

/// Whether `slot` is `place`'s own storage, so a write into it moves the
/// place's bytes in the place's byte coordinates: a parameter home, a block
/// parameter, or the producing operation's `Structural` home. For
/// `Structural { operation, place }` the place's declaration settles which:
/// the slot is the result's storage exactly when the place is declared as
/// that operation's result — record, case, array, scalar-local,
/// subslice-descriptor, and call-result homes all publish the slot's
/// materialized address as the place's storage pointer, so slot and place
/// share byte coordinates. Any other `Structural` slot only stages bytes
/// that name the place — a call's staged view descriptor — under its own
/// slot coordinates; a staging operation can never be its own argument's
/// producer, so the declaration check never confuses the two.
pub(super) fn local_slot_is_place_storage(
    slot: LocalStorageSlotId,
    place: PlaceId,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    match slot {
        LocalStorageSlotId::StructuralParameter { place: slot_place }
        | LocalStorageSlotId::StructuralBlockParameter {
            place: slot_place, ..
        } => slot_place == place,
        LocalStorageSlotId::Structural {
            operation,
            place: slot_place,
        } => {
            slot_place == place
                && structural_places.iter().any(|declaration| {
                    declaration.id == place
                        && matches!(
                            declaration.kind,
                            StructuralPlaceKind::OperationResult { producer, .. }
                                if producer == operation
                        )
                })
        }
        LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. } => false,
    }
}

/// A subject's written or read span in a place's byte coordinates: the
/// `byte_count` bytes at `byte_offset`, or — when `dynamic` — an extent
/// whose runtime decider places its bytes anywhere at or after
/// `byte_offset` with no static upper bound. The decider is the
/// byte-sequence row's `index` or a byte-span's `length`; when it resolves
/// to a clean materialized constant the caller collapses the subject to the
/// exact bytes first, so `dynamic` here means genuinely unresolved.
#[derive(Clone, Copy)]
pub(super) struct SubjectExtent {
    pub byte_offset: u32,
    pub byte_count: u32,
    /// Whether the extent's reach is unbounded upward from `byte_offset`.
    pub dynamic: bool,
}

/// Which storage holds the subject's bytes. `Place` is the place's own
/// storage — the referent bytes every place-named roster route decides.
/// `Staging` is one staging slot's own bytes: a `Structural` slot the
/// place's declaration does not charge to the slot's operation stages bytes
/// that name the place under slot coordinates no place-named row can reach,
/// so only the rows naming that very slot decide the subject.
#[derive(Clone, Copy)]
pub(super) enum SubjectStorage {
    Place(PlaceId),
    Staging(LocalStorageSlotId),
}

/// Whether `slot` is the subject's own storage: the place's own slot for a
/// `Place` subject, the staging slot itself for a `Staging` one.
pub(super) fn slot_is_subject_storage(
    slot: LocalStorageSlotId,
    storage: SubjectStorage,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    match storage {
        SubjectStorage::Place(place) => local_slot_is_place_storage(slot, place, structural_places),
        SubjectStorage::Staging(staging) => slot == staging,
    }
}

/// Exact rows intersect when their half-open byte intervals share a byte;
/// widened to u64 so edge offsets cannot wrap. A dynamic extent is
/// unbounded upward from `byte_offset`, so the exact row still reaches the
/// subject once its own extent ends past that offset — ending at or below
/// it is the only provable disjointness.
pub(super) fn extent_intersects(subject: SubjectExtent, access: &SelectedMemoryAccess) -> bool {
    if subject.dynamic {
        return u64::from(subject.byte_offset)
            < u64::from(access.byte_offset) + u64::from(access.byte_count);
    }
    u64::from(access.byte_offset) < u64::from(subject.byte_offset) + u64::from(subject.byte_count)
        && u64::from(subject.byte_offset)
            < u64::from(access.byte_offset) + u64::from(access.byte_count)
}

/// A byte-sequence row's runtime `index` value, when the row is one — the
/// decider whose `byte_offset + index` position `extent_reached_by` lands.
/// Callers that treat only one sequence direction apply their own match.
pub(super) fn byte_sequence_index(access: &SelectedMemoryAccess) -> Option<ValueId> {
    match access.role {
        SelectedMemoryAccessRole::ReadByteSequence { index, .. }
        | SelectedMemoryAccessRole::WriteByteSequence { index, .. } => Some(index),
        _ => None,
    }
}

/// Whether a roster row's reach intersects the subject's extent. A
/// dynamic-extent row's reach is unbounded only upward: a span row covers
/// `length` bytes starting at `byte_offset` and a sequence row touches the
/// single byte `byte_offset + index`, so every byte the row can touch lies
/// at or after `byte_offset` — it reaches the subject exactly while its
/// fixed offset starts below the subject's end. A sequence row whose
/// `index` resolves to a clean `MaterializeI64` touches exactly that one
/// byte wherever its payload base sits, so it reaches the subject only by
/// landing inside it; `sequence_index` carries that row's index when the
/// caller treats the row kind as a sequence access. When the subject is
/// itself dynamic, an unresolved row can always meet it, and a resolved
/// landing byte meets it only at or past the subject's base.
pub(super) fn extent_reached_by(
    subject: SubjectExtent,
    sequence_index: Option<ValueId>,
    access: &SelectedMemoryAccess,
    function: &SelectedFunction,
) -> bool {
    if let Some(index) = sequence_index
        && let Some(landed) = constant_index(function, index)
        && let Some(position) = u64::from(access.byte_offset).checked_add(landed)
    {
        let start = u64::from(subject.byte_offset);
        return if subject.dynamic {
            position >= start
        } else {
            position >= start && position < start + u64::from(subject.byte_count)
        };
    }
    if subject.dynamic {
        return true;
    }
    u64::from(access.byte_offset) < u64::from(subject.byte_offset) + u64::from(subject.byte_count)
}

/// Whether a roster row's access interferes with the subject's extent.
/// Rows on the subject's storage could place their bytes over the subject's
/// and are decided on it — place-named rows must name the subject's place
/// first, then intersect or reach the subject's extent; local-slot rows
/// decide by naming the subject's own slot. Outgoing slots the subject
/// cannot own never interfere.
pub(super) fn extent_interferes(
    subject: SubjectExtent,
    storage: SubjectStorage,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
    function: &SelectedFunction,
) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            matches!(storage, SubjectStorage::Place(place) if access.place == place)
                && extent_intersects(subject, access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. } => {
            matches!(storage, SubjectStorage::Place(place) if access.place == place)
                && extent_reached_by(subject, byte_sequence_index(access), access, function)
        }
        SelectedMemoryAccessRole::WriteLocal { slot } => {
            slot_is_subject_storage(slot, storage, structural_places)
                && extent_intersects(subject, access)
        }
        SelectedMemoryAccessRole::AddressLocal { slot } => {
            slot_is_subject_storage(slot, storage, structural_places)
        }
        SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// The compile-time constant a byte-sequence row's `index` or span row's
/// `length` resolves to when it does: the register carrying the value must
/// be its sole `InstructionResult` carrier, holding the function's one
/// clean `MaterializeI64` definition and never redefined by an edge
/// transport or case payload. `None` leaves the extent dynamic — a carrier
/// ambiguity or any intervening definition is not provable here.
pub(super) fn constant_index(function: &SelectedFunction, index: ValueId) -> Option<u64> {
    let mut carriers = function.virtual_registers.iter().filter(|register| {
        matches!(
            register.origin,
            VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == index
        )
    });
    let carrier = carriers.next()?;
    if carriers.next().is_some() {
        return None;
    }
    let landed = materialized_bits(function, carrier.id).ok()?;
    if transport_defines(function, carrier.id) {
        return None;
    }
    Some(landed)
}

/// Whether an edge transport or case payload defines `register` — a
/// definition the instruction-operand audit in `materialized_bits` cannot
/// see, which would falsify the constant it reports.
pub(super) fn transport_defines(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
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

/// The named store's operand surface as a constraint row: exactly
/// `[use pointer, use value]`. Each rewrite adds its own custody policy —
/// operand plainness for a store that must move, scratch custody for one
/// that is removed.
pub(super) fn place_store_row_shape<E>(
    row: &RegisterInstructionConstraint,
    reject: impl Fn() -> E,
) -> Result<(), E> {
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
    {
        return Err(reject());
    }
    Ok(())
}

/// The packed store's operand surface as a constraint row:
/// `[use pointer, use packed value]` plus the early-clobber scratch `Def`
/// the multi-instruction store writes through. Each rewrite adds its own
/// custody policy on the scratch the row declares.
pub(super) fn packed_store_row_shape<E>(
    row: &RegisterInstructionConstraint,
    reject: impl Fn() -> E,
) -> Result<(), E> {
    if row.operands.len() != 3
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[2].operand != 2
        || row.operands[2].access != RegisterOperandAccess::Def
        || !row.operands[2].early_clobber
    {
        return Err(reject());
    }
    Ok(())
}

/// The direct slot store's operand surface: the target's declared `store64`
/// row — exactly `[use value]` — and the instruction carrying just that one
/// use. The instruction defines nothing, so no custody check is needed.
pub(super) fn local_store_shape<E>(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
    reject: impl Fn() -> E,
) -> Result<(), E> {
    if environment.selected_keys().store64 != Some(instruction.constraint) {
        return Err(reject());
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or_else(&reject)?;
    if row.operands.len() != 1
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || instruction.operands.len() != 1
        || instruction.operands[0].operand != 0
        || instruction.operands[0].access != RegisterOperandAccess::Use
    {
        return Err(reject());
    }
    Ok(())
}
