//! Program-scoped conformance carriers and call-argument destination answers.

use super::{CALL_ARGUMENT_DESTINATIONS_SLOT, CONFORMANCE_SLOT_CARRIERS_SLOT, program_key};
use std::collections::HashMap;
use typed_trees::statement::TableCall;
use typed_trees::types::TypeReferenceHandle;

/// The conformance slot-carrier index — (trait symbol, bound requirement
/// name) -> (carrier, entry symbol) rows — is pure in the program, and the
/// license builders rebuild it per judged machine. Inside a scope it is
/// built once per build; the caller keeps the clone for both license passes.
pub(crate) fn memoized_conformance_slot_carriers(
    program: &dyn crate::CheckingInput,
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
    let key = program_key(program);
    let state = CONFORMANCE_SLOT_CARRIERS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if *owner == key {
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
                *scope = Some((key, index.clone()));
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
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
    argument_count: usize,
    compute: impl FnOnce() -> Option<Vec<TypeReferenceHandle>>,
) -> Option<Vec<TypeReferenceHandle>> {
    super::scoped_keyed_memo(
        program,
        &CALL_ARGUMENT_DESTINATIONS_SLOT,
        (target, argument_count),
        compute,
    )
}

/// The single machine state `symbol` selects, when exactly one state carries
/// it.
pub(crate) fn unique_state_by_symbol<'p>(
    program: &'p dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<&'p typed_trees::state::State> {
    let matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.symbol == symbol)
        .collect::<Vec<_>>();
    let [state] = matches.as_slice() else {
        return None;
    };
    Some(state)
}

/// A call's receiver spelled as member-name strings: the statement table's
/// name-path members owned outright for the signature selectors that compare
/// them against authored path spellings.
pub(crate) fn call_receiver_member_names(
    program: &dyn crate::CheckingInput,
    call: &TableCall,
) -> Vec<String> {
    program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect()
}
