use omega_control_flow::{
    StateContractCall, StateContractExit, StateContractFactKind, StateContractFactRef,
    StateContractSummary,
};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

use crate::handles::{
    remap_contract_call_span, remap_contract_exit_span, remap_contract_fact_ref_span,
};

pub(crate) fn remap_contract_fact_refs(state_graph: &StateGraph) -> Arena<StateContractFactRef> {
    let mut refs = Arena::with_capacity(state_graph.contract_fact_refs.len());

    for (_, reference) in state_graph.contract_fact_refs.iter() {
        refs.append(remap_contract_fact_ref(reference));
    }

    refs
}

fn remap_contract_fact_ref(
    reference: &omega_state_graph::StateContractFactRef,
) -> StateContractFactRef {
    StateContractFactRef {
        kind: match reference.kind {
            omega_state_graph::StateContractFactKind::Requires => StateContractFactKind::Requires,
            omega_state_graph::StateContractFactKind::Ensures => StateContractFactKind::Ensures,
            omega_state_graph::StateContractFactKind::Boundary => StateContractFactKind::Boundary,
        },
        fact: reference.fact,
    }
}

pub(crate) fn remap_contract_fact_ref_owned(
    reference: omega_state_graph::StateContractFactRef,
) -> StateContractFactRef {
    remap_contract_fact_ref(&reference)
}

pub(crate) fn remap_contract_calls(state_graph: &StateGraph) -> Arena<StateContractCall> {
    let mut calls = Arena::with_capacity(state_graph.contract_calls.len());

    for (_, call) in state_graph.contract_calls.iter() {
        calls.append(remap_contract_call(call));
    }

    calls
}

fn remap_contract_call(call: &omega_state_graph::StateContractCall) -> StateContractCall {
    StateContractCall {
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
        target_machine_symbol: call.target_machine_symbol,
        target_state_symbol: call.target_state_symbol,
        requires: remap_contract_fact_ref_span(call.requires),
        ensures: remap_contract_fact_ref_span(call.ensures),
    }
}

pub(crate) fn remap_contract_call_owned(
    call: omega_state_graph::StateContractCall,
) -> StateContractCall {
    remap_contract_call(&call)
}

pub(crate) fn remap_contract_exits(state_graph: &StateGraph) -> Arena<StateContractExit> {
    let mut exits = Arena::with_capacity(state_graph.contract_exits.len());

    for (_, exit) in state_graph.contract_exits.iter() {
        exits.append(remap_contract_exit(exit));
    }

    exits
}

fn remap_contract_exit(exit: &omega_state_graph::StateContractExit) -> StateContractExit {
    StateContractExit {
        statement_index: exit.statement_index,
        ensures: remap_contract_fact_ref_span(exit.ensures),
    }
}

pub(crate) fn remap_contract_exit_owned(
    exit: omega_state_graph::StateContractExit,
) -> StateContractExit {
    remap_contract_exit(&exit)
}

pub(crate) fn remap_contract_summary(
    contracts: &omega_state_graph::StateContractSummary,
) -> StateContractSummary {
    StateContractSummary {
        calls: remap_contract_call_span(contracts.calls),
        exits: remap_contract_exit_span(contracts.exits),
    }
}
