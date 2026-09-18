//! Memory-roster commutation for the scheduling rewrites under proven
//! access independence: which two recorded accesses may trade order
//! without either observing the other's effect, and the row permutation
//! that keeps the roster in execution order when they do. The commuting
//! interchange families — `commuting_interchange` at pair granularity and
//! `commuting_run_interchange` at run granularity — share this one audit;
//! the families that keep recorded accesses in place never consult it.
//!
//! The overlap reading is the roster's own: the dead-store and store-motion
//! walks ask `local_slot_is_place_storage` whether a local-slot row reaches a
//! place's bytes and compare byte extents only inside one place or slot.
//! This audit applies the same reading symmetrically between two rows.
use selected_instructions::{
    LocalStorageSlotId, OutgoingArgumentSlotId, SelectedFunction, SelectedInstructionId,
    SelectedMemoryAccess, SelectedMemoryAccessRole,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use crate::rewrites::place_storage::local_slot_is_place_storage;

/// The storage one roster row reaches. A place-role row reaches its place's
/// bytes; a local-slot row reaches its slot, which is the row's place's own
/// storage only when the place declaration says so; an outgoing-slot row
/// reaches that staging slot, which no place's storage shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Storage {
    Place(PlaceId),
    LocalSlot(LocalStorageSlotId),
    OutgoingSlot(OutgoingArgumentSlotId),
}

fn storage(
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
) -> Storage {
    match access.role {
        SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } => {
            if local_slot_is_place_storage(slot, access.place, structural_places) {
                Storage::Place(access.place)
            } else {
                // A staging or private slot is its own storage: the staged
                // bytes only name the place under the slot's own
                // coordinates and no place row reaches them.
                Storage::LocalSlot(slot)
            }
        }
        SelectedMemoryAccessRole::WriteOutgoing { slot }
        | SelectedMemoryAccessRole::AddressOutgoing { slot } => Storage::OutgoingSlot(slot),
        _ => Storage::Place(access.place),
    }
}

/// Whether the row's role writes the bytes it reaches. Reads and address
/// exposures observe or name storage without changing it, so two
/// non-writing accesses always commute.
fn writes(role: SelectedMemoryAccessRole) -> bool {
    use SelectedMemoryAccessRole::*;
    matches!(
        role,
        WritePlace
            | WriteByteSpan { .. }
            | WriteByteSequence { .. }
            | WriteLocal { .. }
            | WriteOutgoing { .. }
    )
}

/// Whether the row's reach is the fixed byte extent `byte_offset ..
/// byte_offset + byte_count`. The span and sequence roles carry a dynamic
/// length or index — their byte_count is zero by contract but their reach
/// is not — so only the fixed roles may settle a shared-storage question by
/// extent.
fn fixed_extent(role: SelectedMemoryAccessRole) -> bool {
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
fn disjoint_extents(earlier: &SelectedMemoryAccess, later: &SelectedMemoryAccess) -> bool {
    let earlier_end = u64::from(earlier.byte_offset) + u64::from(earlier.byte_count);
    let later_end = u64::from(later.byte_offset) + u64::from(later.byte_count);
    earlier_end <= u64::from(later.byte_offset) || later_end <= u64::from(earlier.byte_offset)
}

/// Whether the two rows may reach overlapping bytes. Distinct places name
/// distinct storage under the roster's own discipline, distinct slot
/// identities are distinct storage, and a place's bytes are never an
/// outgoing staging slot's — but a slot row reaches a place's bytes exactly
/// when the slot is that place's own storage, the same question the
/// dead-store and store-motion walks ask.
fn overlapping(
    earlier: &SelectedMemoryAccess,
    later: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    match (
        storage(earlier, structural_places),
        storage(later, structural_places),
    ) {
        (Storage::Place(earlier_place), Storage::Place(later_place)) => {
            earlier_place == later_place
        }
        (Storage::LocalSlot(slot), Storage::Place(place))
        | (Storage::Place(place), Storage::LocalSlot(slot)) => {
            local_slot_is_place_storage(slot, place, structural_places)
        }
        (Storage::LocalSlot(earlier_slot), Storage::LocalSlot(later_slot)) => {
            earlier_slot == later_slot
        }
        (Storage::OutgoingSlot(earlier_slot), Storage::OutgoingSlot(later_slot)) => {
            earlier_slot == later_slot
        }
        _ => false,
    }
}

/// Whether the two recorded accesses may trade order. Two non-writing rows
/// commute unconditionally — neither changes what the other observes. When
/// at least one writes, the rows must reach provably disjoint bytes:
/// different storage identities never overlap, and shared storage admits
/// only disjoint fixed extents — a dynamic-extent row on the same storage
/// cannot be bounded away from the other access and refuses.
pub(super) fn commutes(
    earlier: &SelectedMemoryAccess,
    later: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
) -> bool {
    if !writes(earlier.role) && !writes(later.role) {
        return true;
    }
    !overlapping(earlier, later, structural_places)
        || (fixed_extent(earlier.role)
            && fixed_extent(later.role)
            && disjoint_extents(earlier, later))
}

/// The roster rows naming each instruction of `window`, grouped by
/// instruction in the window's own order, each group's rows in roster
/// order.
pub(super) fn window_rows<'function>(
    function: &'function SelectedFunction,
    window: &[selected_instructions::SelectedInstruction],
) -> Vec<Vec<&'function SelectedMemoryAccess>> {
    window
        .iter()
        .map(|instruction| {
            function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == instruction.id)
                .collect()
        })
        .collect()
}

/// The roster positions of every row naming a window instruction, in
/// roster order — the subsequence the interchange permutes. Rows naming
/// instructions outside the window keep their positions even if a
/// malformed roster interleaved them among the window's rows.
pub(super) fn window_row_positions(
    function: &SelectedFunction,
    window: &[SelectedInstructionId],
) -> Vec<usize> {
    function
        .memory_accesses
        .iter()
        .enumerate()
        .filter_map(|(position, access)| window.contains(&access.instruction).then_some(position))
        .collect()
}

/// The window's rows in the window's new execution order: for each
/// instruction in `new_order`, its rows in their own roster order.
pub(super) fn rows_in_order(
    function: &SelectedFunction,
    new_order: &[SelectedInstructionId],
) -> Vec<SelectedMemoryAccess> {
    new_order
        .iter()
        .flat_map(|instruction| {
            function
                .memory_accesses
                .iter()
                .filter(move |access| access.instruction == *instruction)
                .cloned()
        })
        .collect()
}
