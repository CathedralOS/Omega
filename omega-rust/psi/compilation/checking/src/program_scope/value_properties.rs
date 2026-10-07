//! Program-scoped ownership frontiers and establishment answers. The
//! value-custody owners compute each property; this owner retains its
//! answer under the program's identity and structural fingerprint.
//! Declaration answers (drop hooks, definition positions) delegate to the
//! `TypedTrees`-owned indexes instead of a slot.

use super::{CLAIM_FRONTIER_SLOT, ESTABLISHMENT_SLOT};
use crate::value_custody::claim_frontier::ClaimFrontierClaim;
use typed_trees::types::TypeReferenceHandle;

/// The exact linear claim frontier of a type is pure in the program, and
/// each caller's walk rebuilds the declaration/parameter indexes before
/// recursing; inside a scope, each queried type reference walks once.
pub(crate) fn memoized_claim_frontier(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
    super::scoped_keyed_memo(program, &CLAIM_FRONTIER_SLOT, type_reference, || {
        crate::value_custody::claim_frontier::linear_claim_frontier_uncached(
            program,
            type_reference,
        )
    })
}

/// Whether a machine attached to `symbol` realizes `::drop` — storage
/// content classification asks it at every data node. The answer lives in
/// the program's owned attached-drop index, rebuilt on any machine-table
/// mutation; no scope slot is needed.
pub(crate) fn memoized_owns_drop_hook(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> bool {
    program.attached_drop_machine_exists(symbol)
}

/// Whether `definition` requires establishment, memoized per (program,
/// definition symbol): the default-domain read and write scans ask it at
/// every data read, and each answer re-walks the definition's fields and
/// evaluates their range endpoints. The definition's symbol is the same
/// key the walk's inner establishment index already memoizes on.
pub(crate) fn memoized_data_requires_establishment(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    compute: impl FnOnce() -> bool,
) -> bool {
    super::scoped_keyed_memo(program, &ESTABLISHMENT_SLOT, definition.symbol, compute)
}
