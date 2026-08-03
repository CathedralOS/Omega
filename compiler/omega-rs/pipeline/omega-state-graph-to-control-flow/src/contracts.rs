mod conversions;

use omega_control_flow::{
    StateContractCall, StateContractExit, StateContractFactRef, StateContractSummary,
};
use omega_state_graph::StateGraph;
use psi_arena::Arena;

use crate::arena_remap::remap_arena;

pub(crate) use conversions::{
    remap_contract_call_owned, remap_contract_exit_owned, remap_contract_fact_ref_owned,
};

use crate::handles::{remap_contract_call_span, remap_contract_exit_span};

pub(crate) fn remap_contract_fact_refs(state_graph: &StateGraph) -> Arena<StateContractFactRef> {
    remap_arena(
        &state_graph.semantics.contracts.fact_refs,
        remap_contract_fact_ref_owned,
    )
}

pub(crate) fn remap_contract_calls(state_graph: &StateGraph) -> Arena<StateContractCall> {
    remap_arena(
        &state_graph.semantics.contracts.calls,
        remap_contract_call_owned,
    )
}

pub(crate) fn remap_contract_exits(state_graph: &StateGraph) -> Arena<StateContractExit> {
    remap_arena(
        &state_graph.semantics.contracts.exits,
        remap_contract_exit_owned,
    )
}

pub(crate) fn remap_contract_summary(
    contracts: &omega_state_graph::StateContractSummary,
) -> StateContractSummary {
    StateContractSummary {
        calls: remap_contract_call_span(contracts.calls),
        exits: remap_contract_exit_span(contracts.exits),
    }
}

#[cfg(test)]
mod tests;
