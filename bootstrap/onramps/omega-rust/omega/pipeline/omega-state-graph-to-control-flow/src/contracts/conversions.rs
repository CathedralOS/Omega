use omega_control_flow::{
    StateContractCall, StateContractExit, StateContractFactKind, StateContractFactRef,
};

use crate::handles::remap_contract_fact_ref_span;

pub(crate) fn remap_contract_fact_ref_owned(
    reference: omega_state_graph::StateContractFactRef,
) -> StateContractFactRef {
    StateContractFactRef {
        kind: match reference.kind {
            omega_state_graph::StateContractFactKind::Requires => StateContractFactKind::Requires,
            omega_state_graph::StateContractFactKind::Ensures => StateContractFactKind::Ensures,
        },
        fact: reference.fact,
    }
}

pub(crate) fn remap_contract_call_owned(
    call: omega_state_graph::StateContractCall,
) -> StateContractCall {
    StateContractCall {
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
        target_machine_symbol: call.target_machine_symbol,
        target_state_symbol: call.target_state_symbol,
        requires: remap_contract_fact_ref_span(call.requires),
        ensures: remap_contract_fact_ref_span(call.ensures),
    }
}

pub(crate) fn remap_contract_exit_owned(
    exit: omega_state_graph::StateContractExit,
) -> StateContractExit {
    StateContractExit {
        statement_index: exit.statement_index,
        ensures: remap_contract_fact_ref_span(exit.ensures),
    }
}
