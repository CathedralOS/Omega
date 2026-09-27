//! Program-scoped conformance carriers and call-argument destination answers.

use super::{CALL_ARGUMENT_DESTINATIONS_SLOT, CONFORMANCE_SLOT_CARRIERS_SLOT};
use std::collections::HashMap;
use typed_trees::types::TypeReferenceHandle;

/// The conformance slot-carrier index — (trait symbol, bound requirement
/// name) -> (carrier, entry symbol) rows — is pure in the program, and the
/// license builders rebuild it per judged machine. Inside a scope it is
/// built once per build; the caller keeps the clone for both license passes.
pub(crate) fn memoized_conformance_slot_carriers(
    program: &typed_trees::TypedTrees,
) -> HashMap<(symbols::SymbolHandle, String), Vec<(TypeReferenceHandle, symbols::SymbolHandle)>> {
    enum SlotState {
        NoScope,
        Hit(
            HashMap<
                (symbols::SymbolHandle, String),
                Vec<(TypeReferenceHandle, symbols::SymbolHandle)>,
            >,
        ),
        Miss,
        ForeignProgram,
    }
    let state = CONFORMANCE_SLOT_CARRIERS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    SlotState::Hit(map.clone())
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(index) = state {
        return index;
    }
    let index = crate::proof_contracts::contract_entailment::structural_judgment::conformance_slot_carriers_uncached(
        program,
    );
    if matches!(state, SlotState::Miss) {
        CONFORMANCE_SLOT_CARRIERS_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                *scope = Some((program, index.clone()));
            }
        });
    }
    index
}

/// The call-argument destination answer for a target state, memoized per
/// (target, argument count): computing it rescans every machine and every
/// state to prove the target symbol is unique, which dominated its callers
/// before verdicts were shared inside the scope.
pub(crate) fn memoized_call_argument_destinations(
    program: &typed_trees::TypedTrees,
    target: symbols::SymbolHandle,
    argument_count: usize,
    compute: impl FnOnce() -> Option<Vec<TypeReferenceHandle>>,
) -> Option<Vec<TypeReferenceHandle>> {
    enum SlotState {
        NoScope,
        Hit(Option<Vec<TypeReferenceHandle>>),
        Miss,
        ForeignProgram,
    }
    let state = CALL_ARGUMENT_DESTINATIONS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&(target, argument_count))
                        .cloned()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(destinations) = state {
        return destinations;
    }
    let destinations = compute();
    if matches!(state, SlotState::Miss) {
        CALL_ARGUMENT_DESTINATIONS_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert((target, argument_count), destinations.clone());
                    }
                    slot_none @ None => {
                        *slot_none = Some((
                            program,
                            HashMap::from([((target, argument_count), destinations.clone())]),
                        ));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    destinations
}
