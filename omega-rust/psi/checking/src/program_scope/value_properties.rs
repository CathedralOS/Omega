//! Program-scoped ownership frontiers, drop hooks and establishment answers.
//! The value-custody owners compute each property; this owner retains its answer.

use super::{CLAIM_FRONTIER_SLOT, DROP_HOOK_SLOT, ESTABLISHMENT_SLOT};
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
    let state = CLAIM_FRONTIER_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
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
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(type_reference, claims.clone());
                    }
                    slot_none @ None => {
                        *slot_none =
                            Some((program, HashMap::from([(type_reference, claims.clone())])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    claims
}

/// Whether a machine attached to `symbol` realizes `::drop`, memoized per
/// (program, symbol): storage-content classification asks it at every data
/// node and the scan is otherwise O(machines) per node.
pub(crate) fn memoized_owns_drop_hook(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> bool {
    enum SlotState {
        NoScope,
        Hit(bool),
        Miss,
        ForeignProgram,
    }
    let state = DROP_HOOK_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&symbol)
                        .copied()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(owns_hook) = state {
        return owns_hook;
    }
    let owns_hook = program.machines().iter().any(|machine| {
        machine.attached_data_symbol == symbol && machine.name.as_str().ends_with("::drop")
    });
    if matches!(state, SlotState::Miss) {
        DROP_HOOK_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(symbol, owns_hook);
                    }
                    slot_none @ None => {
                        *slot_none = Some((program, HashMap::from([(symbol, owns_hook)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    owns_hook
}

/// Whether `definition` requires establishment, memoized per (program,
/// definition): the default-domain read and write scans ask it at every data
/// read, and each answer re-walks the definition's fields and evaluates their
/// range endpoints.
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
    let key = std::ptr::from_ref(definition).addr();
    let state = ESTABLISHMENT_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&key)
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
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(key, requires);
                    }
                    slot_none @ None => {
                        *slot_none = Some((program, HashMap::from([(key, requires)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    requires
}
