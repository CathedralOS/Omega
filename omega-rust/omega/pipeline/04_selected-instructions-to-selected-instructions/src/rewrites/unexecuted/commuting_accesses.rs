//! Memory-roster commutation for the scheduling rewrites under proven
//! access independence: which two recorded accesses may trade order
//! without either observing the other's effect, and the row permutation
//! that keeps the roster in execution order when they do. The commuting
//! interchange families — `commuting_interchange` at pair granularity,
//! `commuting_member_run_interchange` at member-against-run granularity,
//! and `commuting_run_interchange` at run granularity — share this one
//! audit; the families that keep recorded accesses in place never consult
//! it.
//!
//! The overlap reading is the roster's own, owned by `place_storage`: each
//! row's storage route comes from `row_storage`, fixed extents compare
//! through `ranges_disjoint`, and a local-slot row reaches a place's bytes
//! exactly when the slot is that place's own storage. This audit applies
//! the same reading symmetrically between two rows.
use selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedMemoryAccess, SelectedMemoryAccessRole,
};
use terminal_psi::StructuralPlaceDeclaration;

use crate::rewrites::unexecuted::place_storage::{
    StorageRoute, fixed_extent, local_slot_is_place_storage, ranges_disjoint, row_storage,
};

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
        row_storage(earlier, structural_places),
        row_storage(later, structural_places),
    ) {
        (StorageRoute::Place(earlier_place), StorageRoute::Place(later_place)) => {
            earlier_place == later_place
        }
        (StorageRoute::LocalSlot(slot), StorageRoute::Place(place))
        | (StorageRoute::Place(place), StorageRoute::LocalSlot(slot)) => {
            local_slot_is_place_storage(slot, place, structural_places)
        }
        (StorageRoute::LocalSlot(earlier_slot), StorageRoute::LocalSlot(later_slot)) => {
            earlier_slot == later_slot
        }
        (StorageRoute::OutgoingSlot(earlier_slot), StorageRoute::OutgoingSlot(later_slot)) => {
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
            && ranges_disjoint(earlier, later))
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
