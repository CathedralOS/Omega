mod conversions;

use omega_control_flow::{
    StateContractCall, StateContractExit, StateContractFactRef, StateContractSummary,
};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

pub(crate) use conversions::{
    remap_contract_call_owned, remap_contract_exit_owned, remap_contract_fact_ref_owned,
};

use crate::handles::{remap_contract_call_span, remap_contract_exit_span};

pub(crate) fn remap_contract_fact_refs(state_graph: &StateGraph) -> Arena<StateContractFactRef> {
    let mut refs = Arena::with_capacity(state_graph.semantics.contract_fact_refs.len());

    for (_, reference) in state_graph.semantics.contract_fact_refs.iter() {
        refs.append(remap_contract_fact_ref_owned(reference.clone()));
    }

    refs
}

pub(crate) fn remap_contract_calls(state_graph: &StateGraph) -> Arena<StateContractCall> {
    let mut calls = Arena::with_capacity(state_graph.semantics.contract_calls.len());

    for (_, call) in state_graph.semantics.contract_calls.iter() {
        calls.append(remap_contract_call_owned(call.clone()));
    }

    calls
}

pub(crate) fn remap_contract_exits(state_graph: &StateGraph) -> Arena<StateContractExit> {
    let mut exits = Arena::with_capacity(state_graph.semantics.contract_exits.len());

    for (_, exit) in state_graph.semantics.contract_exits.iter() {
        exits.append(remap_contract_exit_owned(exit.clone()));
    }

    exits
}

pub(crate) fn remap_contract_summary(
    contracts: &omega_state_graph::StateContractSummary,
) -> StateContractSummary {
    StateContractSummary {
        calls: remap_contract_call_span(contracts.calls),
        exits: remap_contract_exit_span(contracts.exits),
    }
}
