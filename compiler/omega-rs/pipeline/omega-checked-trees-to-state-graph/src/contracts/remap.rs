use omega_state_graph::{
    StateContractCall, StateContractExit, StateContractFactRef, StateContractSummary, StateGraph,
};
use psi_arena::{Arena, HandleSpan};

pub(crate) struct SourceContractArenas<'a> {
    pub(crate) fact_refs: &'a Arena<StateContractFactRef>,
    pub(crate) calls: &'a Arena<StateContractCall>,
    pub(crate) exits: &'a Arena<StateContractExit>,
}

pub(crate) fn remap_state_contract_summary(
    target: &mut StateGraph,
    source: &SourceContractArenas<'_>,
    contracts: &StateContractSummary,
) -> StateContractSummary {
    let calls = append_remapped_contract_calls(target, source, contracts.calls);
    let exits = append_remapped_contract_exits(target, source, contracts.exits);

    StateContractSummary { calls, exits }
}

fn append_remapped_contract_calls(
    target: &mut StateGraph,
    source: &SourceContractArenas<'_>,
    calls: HandleSpan<StateContractCall>,
) -> HandleSpan<StateContractCall> {
    let mut remapped_calls = HandleSpan::empty();

    for call in source.calls.span_or_empty(calls) {
        let requires = target.semantics.contracts.fact_refs.insert_many(
            source
                .fact_refs
                .span_or_empty(call.requires)
                .iter()
                .cloned(),
        );
        let ensures = target
            .semantics
            .contracts
            .fact_refs
            .insert_many(source.fact_refs.span_or_empty(call.ensures).iter().cloned());

        target.semantics.contracts.calls.append_to_span(
            &mut remapped_calls,
            StateContractCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine_symbol: call.target_machine_symbol,
                target_state_symbol: call.target_state_symbol,
                requires,
                ensures,
            },
        );
    }

    remapped_calls
}

fn append_remapped_contract_exits(
    target: &mut StateGraph,
    source: &SourceContractArenas<'_>,
    exits: HandleSpan<StateContractExit>,
) -> HandleSpan<StateContractExit> {
    let mut remapped_exits = HandleSpan::empty();

    for exit in source.exits.span_or_empty(exits) {
        let ensures = target
            .semantics
            .contracts
            .fact_refs
            .insert_many(source.fact_refs.span_or_empty(exit.ensures).iter().cloned());

        target.semantics.contracts.exits.append_to_span(
            &mut remapped_exits,
            StateContractExit {
                statement_index: exit.statement_index,
                ensures,
            },
        );
    }

    remapped_exits
}

#[cfg(test)]
mod tests;
