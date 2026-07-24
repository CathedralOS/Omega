use super::*;

pub(crate) fn borrow_state_fact(
    borrow: &BorrowFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<(Handle<StateBorrowFact>, &StateBorrowFact)> {
    borrow.states.iter().find_map(|(handle, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some((handle, state))
    })
}

pub(crate) fn proof_contract_call(
    proof: &ProofFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&ContractCallFact> {
    proof.contract_calls.iter().find_map(|(_, call)| {
        (call.caller_machine_symbol == machine_symbol
            && call.caller_state_symbol == state_symbol
            && call.statement_index == statement_index
            && call.call_ordinal == call_ordinal)
            .then_some(call)
    })
}
