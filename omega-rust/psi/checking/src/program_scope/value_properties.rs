//! Program-scoped ownership frontiers and establishment answers. The
//! value-custody owners compute each property; this owner retains its
//! answer under the program's identity and structural fingerprint.
//! Declaration answers (drop hooks, definition positions) delegate to the
//! `TypedTrees`-owned indexes instead of a slot.

use super::{CLAIM_FRONTIER_SLOT, ESTABLISHMENT_SLOT, program_key};
use crate::value_custody::claim_frontier::ClaimFrontierClaim;
use std::collections::HashMap;
use typed_trees::types::TypeReferenceHandle;

/// The exact linear claim frontier of a type is pure in the program, and
/// each caller's walk rebuilds the declaration/parameter indexes before
/// recursing; inside a scope, each queried type reference walks once.
pub(crate) fn memoized_claim_frontier(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
    enum SlotState {
        NoScope,
        Hit(Vec<ClaimFrontierClaim>),
        Miss,
        ForeignProgram,
    }
    let key = program_key(program);
    let state = CLAIM_FRONTIER_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if *owner == key {
                    map.get(&type_reference)
                        .cloned()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(claims) = state {
        return claims;
    }
    let claims = crate::value_custody::claim_frontier::linear_claim_frontier_uncached(
        program,
        type_reference,
    );
    if matches!(state, SlotState::Miss) {
        CLAIM_FRONTIER_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if *owner == key => {
                        map.insert(type_reference, claims.clone());
                    }
                    slot_none @ None => {
                        *slot_none = Some((key, HashMap::from([(type_reference, claims.clone())])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    claims
}

/// Whether a machine attached to `symbol` realizes `::drop` — storage
/// content classification asks it at every data node. The answer lives in
/// the program's owned attached-drop index, rebuilt on any machine-table
/// mutation; no scope slot is needed.
pub(crate) fn memoized_owns_drop_hook(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    compute: impl FnOnce() -> bool,
) -> bool {
    enum SlotState {
        NoScope,
        Hit(bool),
        Miss,
        ForeignProgram,
    }
    let key = program_key(program);
    let definition_symbol = definition.symbol;
    let state = ESTABLISHMENT_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if *owner == key {
                    map.get(&definition_symbol)
                        .copied()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(requires) = state {
        return requires;
    }
    let requires = compute();
    if matches!(state, SlotState::Miss) {
        ESTABLISHMENT_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if *owner == key => {
                        map.insert(definition_symbol, requires);
                    }
                    slot_none @ None => {
                        *slot_none = Some((key, HashMap::from([(definition_symbol, requires)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    requires
}
