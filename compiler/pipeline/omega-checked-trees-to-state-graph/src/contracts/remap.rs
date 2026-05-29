use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::{
    StateContractCall, StateContractExit, StateContractFactRef, StateContractSummary, StateGraph,
};

pub(crate) fn remap_state_contract_summary(
    target: &mut StateGraph,
    source_fact_refs: &Arena<StateContractFactRef>,
    source_calls: &Arena<StateContractCall>,
    source_exits: &Arena<StateContractExit>,
    contracts: &StateContractSummary,
) -> StateContractSummary {
    let calls =
        append_remapped_contract_calls(target, source_fact_refs, source_calls, contracts.calls);
    let exits =
        append_remapped_contract_exits(target, source_fact_refs, source_exits, contracts.exits);

    StateContractSummary { calls, exits }
}

fn append_remapped_contract_calls(
    target: &mut StateGraph,
    source_fact_refs: &Arena<StateContractFactRef>,
    source_calls: &Arena<StateContractCall>,
    calls: HandleSpan<StateContractCall>,
) -> HandleSpan<StateContractCall> {
    let mut remapped_calls = HandleSpan::empty();

    for call in source_calls.span_or_empty(calls) {
        let requires = target.contract_fact_refs.insert_many(
            source_fact_refs
                .span_or_empty(call.requires)
                .iter()
                .cloned(),
        );
        let ensures = target
            .contract_fact_refs
            .insert_many(source_fact_refs.span_or_empty(call.ensures).iter().cloned());

        target.contract_calls.append_to_span(
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
    source_fact_refs: &Arena<StateContractFactRef>,
    source_exits: &Arena<StateContractExit>,
    exits: HandleSpan<StateContractExit>,
) -> HandleSpan<StateContractExit> {
    let mut remapped_exits = HandleSpan::empty();

    for exit in source_exits.span_or_empty(exits) {
        let ensures = target
            .contract_fact_refs
            .insert_many(source_fact_refs.span_or_empty(exit.ensures).iter().cloned());

        target.contract_exits.append_to_span(
            &mut remapped_exits,
            StateContractExit {
                statement_index: exit.statement_index,
                ensures,
            },
        );
    }

    remapped_exits
}
