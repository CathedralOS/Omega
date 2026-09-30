//! Element views: establishing one, measuring it, reading through it,
//! storing through it, and narrowing it to a subslice.

pub(crate) mod establishment;
pub(crate) mod length;
pub(crate) mod mutable_establishment;
pub(crate) mod read;
pub(crate) mod subslice;
pub(crate) mod write;

use crate::validation::{StructuralAccess, StructuralPlaceKind, TerminalMachine};

/// The operation result a live view binding occupies, paired with the
/// access its establishment minted: a shared view lends only shared; a
/// mutable view supplies every borrow kind. Neither kind lets the place
/// masquerade as an owned local.
pub(crate) fn view_loan_result<'a>(
    caller: &'a TerminalMachine,
    place: crate::validation::PlaceId,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    StructuralAccess,
)> {
    let StructuralPlaceKind::OperationResult { producer, .. } = caller
        .structural_places
        .iter()
        .find(|row| row.id == place)?
        .kind
    else {
        return None;
    };
    let operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let access = match operation.kind {
        crate::validation::OperationKind::EstablishElementView { .. }
        | crate::validation::OperationKind::ElementViewSubslice { .. } => {
            StructuralAccess::SharedBorrow
        }
        crate::validation::OperationKind::EstablishMutableElementView { .. } => {
            StructuralAccess::MutableBorrow
        }
        _ => return None,
    };
    operation
        .result
        .structural()
        .filter(|result| result.place == place)
        .map(|result| (result, access))
}
