use omega_control_flow::{StateContractCall, StateContractExit, StateContractFactRef};
use psi_arena::HandleSpan;

use super::remap_span;

pub(crate) fn remap_contract_fact_ref_span(
    refs: HandleSpan<omega_state_graph::StateContractFactRef>,
) -> HandleSpan<StateContractFactRef> {
    remap_span(refs)
}

pub(crate) fn remap_contract_call_span(
    calls: HandleSpan<omega_state_graph::StateContractCall>,
) -> HandleSpan<StateContractCall> {
    remap_span(calls)
}

pub(crate) fn remap_contract_exit_span(
    exits: HandleSpan<omega_state_graph::StateContractExit>,
) -> HandleSpan<StateContractExit> {
    remap_span(exits)
}
