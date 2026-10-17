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
//! once rather than once per rewrite. The store row-shape checks
//! (`local_store_shape`, `place_store_row_shape`, `packed_store_row_shape`)
//! admit the store instruction's operand surface once; each rewrite
//! composes its own custody policy on the admitted row.
use selected_instructions::{
    LocalStorageSlotId, SelectedCasePayloadTransport, SelectedFunction, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedValueTransport, VirtualRegisterId, VirtualRegisterOrigin,
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
/// that name the place under its own
/// slot coordinates; a staging operation can never be its own argument's
/// producer, so the declaration check never confuses the two. A
/// `StructuralCallArgument` slot is always temporary descriptor storage;
/// its call and argument ordinal never name the backing place's own storage.
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
        LocalStorageSlotId::Spill { .. }
        | LocalStorageSlotId::Boundary { .. }
        | LocalStorageSlotId::EdgeSnapshot { .. }
        | LocalStorageSlotId::StructuralCallArgument { .. }
        | LocalStorageSlotId::BoundaryResult { .. } => false,
    }
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
/// position. A runtime-projected element row lands at `byte_offset` plus
/// every runtime element's `index · stride` — its instruction carries one
/// row per element — so it resolves only when all of that access's indices
/// do. `None` for every other role and for an unresolved index — a row then
/// reaches an unbounded-upward extent instead.
pub(super) fn resolved_sequence_position(
    access: &SelectedMemoryAccess,
    function: &SelectedFunction,
) -> Option<u64> {
    match access.role {
        SelectedMemoryAccessRole::ReadByteSequence { index, .. }
        | SelectedMemoryAccessRole::WriteByteSequence { index, .. } => {
            return constant_index(function, index)
                .and_then(|landed| u64::from(access.byte_offset).checked_add(landed));
        }
        SelectedMemoryAccessRole::ReadIndexedPrimitive { .. }
        | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
        | SelectedMemoryAccessRole::WriteIndexedStructural { .. } => {
            return function
                .memory_accesses
                .iter()
                .filter(|element| {
                    element.instruction == access.instruction
                        && element.origin == access.origin
                        && element.place == access.place
                })
                .filter_map(|element| match element.role {
                    SelectedMemoryAccessRole::ReadIndexedPrimitive { index, stride, .. }
                    | SelectedMemoryAccessRole::WriteIndexedPrimitive { index, stride, .. }
                    | SelectedMemoryAccessRole::WriteIndexedStructural { index, stride, .. } => {
                        Some((index, stride))
                    }
                    _ => None,
                })
                .try_fold(u64::from(access.byte_offset), |landed, (index, stride)| {
                    landed.checked_add(
                        constant_index(function, index)?.checked_mul(u64::from(stride))?,
                    )
                });
        }
        _ => {}
    }
    None
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
