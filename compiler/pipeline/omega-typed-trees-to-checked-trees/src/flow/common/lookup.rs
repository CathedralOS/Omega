use super::*;

pub(crate) fn borrow_state_fact(
    borrow: &BorrowFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&StateBorrowFact> {
    borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
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

pub(crate) fn effects_machine(
    effects: &omega_effects::EffectPlan,
    machine_symbol: SymbolHandle,
) -> Option<&omega_effects::MachineEffects> {
    effects
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
}

pub(crate) fn effects_state<'effects>(
    effects: &'effects omega_effects::EffectPlan,
    machine_effects: Option<&'effects omega_effects::MachineEffects>,
    state_symbol: SymbolHandle,
) -> Option<&'effects omega_effects::StateEffects> {
    machine_effects.and_then(|machine_effects| {
        effects
            .states
            .span_or_empty(machine_effects.states)
            .iter()
            .find(|state| state.symbol == state_symbol)
    })
}

pub(crate) fn effects_call<'effects>(
    effects: &'effects omega_effects::EffectPlan,
    state_effects: Option<&'effects omega_effects::StateEffects>,
    borrow_call: &BorrowCallFact,
) -> Option<&'effects omega_effects::CallEffects> {
    state_effects.and_then(|state_effects| {
        effects
            .calls
            .span_or_empty(state_effects.calls)
            .iter()
            .find(|call| {
                call.statement_index == borrow_call.statement_index
                    && call.call_ordinal == borrow_call.call_ordinal
                    && call.target_state_symbol == borrow_call.target_symbol
            })
    })
}
