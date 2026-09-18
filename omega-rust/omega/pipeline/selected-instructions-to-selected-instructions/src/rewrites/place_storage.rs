//! Which local storage slot is a structural place's own storage, shared by
//! the rewrites that reason about the bytes a place owns.
use selected_instructions::{LocalStorageSlotId, SelectedFunction};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::StructuralPlaceDeclaration;

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
