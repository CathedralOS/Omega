//! A recursive guarantee is available only after independent strict descent
//! at its exact call edge. Complete body admission, actual argument capture,
//! premise checking and post-return preservation remain separate obligations.

use super::Invocation;
use checked_trees::{CheckFacts, FlowStateFact};

pub(super) fn strict_recursive_call(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    invocation: &Invocation<'_>,
) -> bool {
    if invocation.callable.owner_symbol() != caller.machine_symbol
        || invocation.caller_state != caller.state_symbol
        || invocation
            .site
            .is_internal_state_transfer(program, caller.machine_symbol)
    {
        return false;
    }
    let Some((machine, state)) =
        program.state_flow_site(caller.machine_symbol, caller.state_symbol)
    else {
        return false;
    };
    let plan = &facts.termination.rank_arrivals;
    let Some(judgment) = plan.judgment_for_body(machine.symbol, machine.states) else {
        return false;
    };
    let needs_range = machine
        .termination_plan
        .implementation_witness
        .as_ref()
        .is_some_and(|witness| witness.rank_range.is_some());
    if !judgment.local_terminates
        || !judgment.diagnostic.is_empty()
        || (needs_range && !judgment.entry_rank_formed)
    {
        return false;
    }
    let Some(correspondence) = plan.for_body(machine.symbol, machine.states) else {
        return false;
    };
    let Some(target) = validation::transition_call_target(
        program,
        machine,
        state,
        invocation.statement,
        invocation.ordinal,
    ) else {
        return false;
    };
    target.is_valid()
        && plan
            .arrival_edges
            .span_or_empty(correspondence.edges)
            .iter()
            .any(|edge| {
                edge.source == state.symbol
                    && edge.destination == correspondence.entry_state
                    && usize::try_from(edge.statement_ordinal).ok() == Some(invocation.statement)
                    && edge.target == target
                    && edge.strictly_decreases
            })
}
