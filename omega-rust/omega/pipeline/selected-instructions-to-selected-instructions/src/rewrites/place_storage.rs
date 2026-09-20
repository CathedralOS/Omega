//! The one owner mapping a memory-roster row to the place, storage route and
//! byte extent it reaches, and deciding the relations the memory rewrites
//! share. A row's route is the storage it can reach — its place's referent
//! bytes, one local slot, or one outgoing staging slot — and its extent is
//! the byte shape that reach occupies: an exact range, a lower-bounded
//! dynamic reach (a byte-sequence index or a span length carried as a
//! runtime value), or the base-plus-index landing a resolved constant makes
//! fixed. `load_forwarding`, `dead_store` and `store_motion` walk the roster
//! through these relations, and the commutation audit decides row ordering
//! by the same route-and-extent reading, so the overlap reasoning lives here
//! once rather than once per rewrite.
use selected_instructions::{
    LocalStorageSlotId, OutgoingArgumentSlotId, SelectedCasePayloadTransport, SelectedFunction,
    SelectedMemoryAccess, SelectedMemoryAccessRole, SelectedValueTransport, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
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

/// The storage one roster row reaches. A place-role row reaches its place's
/// bytes; a local-slot row reaches its slot, which is the row's place's own
/// storage only when the place declaration says so; an outgoing-slot row
/// reaches that staging slot, which no place's storage shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StorageRoute {
    Place(PlaceId),
    LocalSlot(LocalStorageSlotId),
    OutgoingSlot(OutgoingArgumentSlotId),
}

/// The route `access` takes to storage: the first step of every extent or
/// overlap decision, shared so the rewrites and the commutation audit read
/// one row the same way.
pub(super) fn row_storage(
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
) -> StorageRoute {
    match access.role {
        SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } => {
            if local_slot_is_place_storage(slot, access.place, structural_places) {
                StorageRoute::Place(access.place)
            } else {
                // A staging or private slot is its own storage: the staged
                // bytes only name the place under the slot's own
                // coordinates and no place row reaches them.
                StorageRoute::LocalSlot(slot)
            }
        }
        SelectedMemoryAccessRole::WriteOutgoing { slot }
        | SelectedMemoryAccessRole::AddressOutgoing { slot } => StorageRoute::OutgoingSlot(slot),
        _ => StorageRoute::Place(access.place),
    }
}

/// Which storage a tracked subject's bytes live in. `Place` is the place's
/// own storage — the referent bytes every place-named roster route decides.
/// `Staging` is one staging slot's own bytes: a `Structural` slot the
/// place's declaration does not charge to the slot's operation stages bytes
/// that name the place under slot coordinates no place-named row can reach,
/// so only the rows naming that very slot — a `WriteLocal` rewriting them
/// or an `AddressLocal` exposing them — decide the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SubjectStorage {
    Place,
    Staging(LocalStorageSlotId),
}

/// Whether a roster row's local slot is the subject's storage: the place's
/// own storage for a place subject, or the staging slot itself for a
/// staging subject — an access into any other slot touches bytes the
/// subject never occupies.
pub(super) fn slot_is_subject_storage(
    slot: LocalStorageSlotId,
    subject: SubjectStorage,
    place: PlaceId,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    match subject {
        SubjectStorage::Place => local_slot_is_place_storage(slot, place, structural_places),
        SubjectStorage::Staging(staging) => slot == staging,
    }
}

/// The staging slot a `WriteLocal` row names when the slot is not the row
/// place's own storage: a `Structural` slot staging bytes that name
/// `place`. The row's place must be the place the slot stages — a
/// `WriteLocal` claiming a different place than the slot's staged name is
/// no coherent staging row — and the caller's `local_slot_is_place_storage`
/// check has already ruled out the producer-home reading, so the slot's
/// bytes are staging coordinates only.
pub(super) fn staging_slot(slot: LocalStorageSlotId, place: PlaceId) -> Option<LocalStorageSlotId> {
    if matches!(slot, LocalStorageSlotId::Structural { .. })
        && slot.structural_place() == Some(place)
    {
        Some(slot)
    } else {
        None
    }
}

/// Whether the row's reach is the fixed byte extent `byte_offset ..
/// byte_offset + byte_count`. The span and sequence roles carry a dynamic
/// length or index — their byte_count is zero by contract but their reach
/// is not — so only the fixed roles may settle a shared-storage question by
/// extent.
pub(super) fn fixed_extent(role: SelectedMemoryAccessRole) -> bool {
    use SelectedMemoryAccessRole::*;
    matches!(
        role,
        ReadPlace
            | WritePlace
            | WriteLocal { .. }
            | WriteOutgoing { .. }
            | AddressLocal { .. }
            | AddressOutgoing { .. }
    )
}

/// Whether the two rows' recorded byte extents leave a gap between them.
/// Both sides must be fixed extents — the commutation audit applies this
/// only after `fixed_extent`, while a subject walk applies the directed
/// `extent_intersects`/`extent_reached_by` relations instead.
pub(super) fn ranges_disjoint(
    earlier: &SelectedMemoryAccess,
    later: &SelectedMemoryAccess,
) -> bool {
    let earlier_end = u64::from(earlier.byte_offset) + u64::from(earlier.byte_count);
    let later_end = u64::from(later.byte_offset) + u64::from(later.byte_count);
    earlier_end <= u64::from(later.byte_offset) || later_end <= u64::from(earlier.byte_offset)
}

/// Whether an exact-extent row's half-open byte interval shares a byte with
/// the subject extent `byte_offset .. byte_offset + byte_count`, widened to
/// u64 so edge offsets cannot wrap. When the subject's reach is dynamic —
/// unbounded upward from `byte_offset` — the row still interferes once its
/// own extent ends past that offset; ending at or below it is the only
/// provable disjointness.
pub(super) fn extent_intersects(
    byte_offset: u32,
    byte_count: u32,
    dynamic_reach: bool,
    access: &SelectedMemoryAccess,
) -> bool {
    if dynamic_reach {
        return u64::from(byte_offset)
            < u64::from(access.byte_offset) + u64::from(access.byte_count);
    }
    u64::from(access.byte_offset) < u64::from(byte_offset) + u64::from(byte_count)
        && u64::from(byte_offset) < u64::from(access.byte_offset) + u64::from(access.byte_count)
}

/// The fixed position a byte-sequence row lands on when its `index` value
/// resolves: `byte_offset + index`, with the overflow carrying no named
/// position. `None` for every other role and for an unresolved index — a
/// row then reaches an unbounded-upward extent instead.
pub(super) fn resolved_sequence_position(
    access: &SelectedMemoryAccess,
    function: &SelectedFunction,
) -> Option<u64> {
    let index = match access.role {
        SelectedMemoryAccessRole::ReadByteSequence { index, .. }
        | SelectedMemoryAccessRole::WriteByteSequence { index, .. } => index,
        _ => return None,
    };
    constant_index(function, index)
        .and_then(|landed| u64::from(access.byte_offset).checked_add(landed))
}

/// Whether a row can reach the subject extent at `byte_offset ..`, counting
/// the row's own reach. A dynamic-extent row's reach is unbounded only
/// upward: a span row covers `length` bytes starting at `byte_offset` and a
/// sequence row touches the single byte `byte_offset + index`, so every
/// byte the row can touch lies at or after `byte_offset`. It still reaches
/// an exact subject range while its fixed offset starts below the range's
/// end; an offset at or past the end is provably disjoint however far the
/// reach extends. A sequence row whose `index` resolves to a clean
/// `MaterializeI64` touches exactly that one byte wherever its payload base
/// sits, so it reaches the subject only by landing inside it. When the
/// subject extent is itself dynamic — its own decider unresolved — an
/// unresolved row always meets it, and a resolved landing byte meets it
/// only at or past the payload base the subject starts at.
pub(super) fn extent_reached_by(
    byte_offset: u32,
    byte_count: u32,
    dynamic_reach: bool,
    access: &SelectedMemoryAccess,
    function: &SelectedFunction,
) -> bool {
    if let Some(position) = resolved_sequence_position(access, function) {
        let start = u64::from(byte_offset);
        return if dynamic_reach {
            position >= start
        } else {
            position >= start && position < start + u64::from(byte_count)
        };
    }
    if dynamic_reach {
        return true;
    }
    u64::from(access.byte_offset) < u64::from(byte_offset) + u64::from(byte_count)
}

/// The compile-time constant a byte-sequence row's `index` (or a span's
/// `length`) resolves to, when it does. The register carrying the value is
/// its sole `InstructionResult` carrier — a value no instruction result
/// carries (an entry or block parameter) has no producer to resolve, and
/// two instruction results claiming one value make the constant ambiguous;
/// both stay unproven. The carrier must then hold the function's one clean
/// `MaterializeI64` definition and never be redefined by an edge transport
/// or case payload the instruction audit cannot see — only then does
/// `byte_offset + index` name a fixed position rather than a runtime-placed
/// byte. Callers map the `None` refusal onto their own diagnostic.
pub(super) fn constant_index(
    function: &SelectedFunction,
    index: semantic_vocabulary::ValueId,
) -> Option<u64> {
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
/// see, which would falsify the constant it reports for the index.
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
