mod conversions;

use omega_control_flow::{
    StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall, StateBorrowLoan,
    StateBorrowSummary, StateBorrowWeakening, StateBorrowWritableRoot,
};
use omega_state_graph::StateGraph;
use psi_arena::Arena;

use crate::arena_remap::remap_arena;

pub(crate) use conversions::{
    remap_borrow_activation_owned, remap_borrow_argument_access_owned, remap_borrow_call_owned,
    remap_borrow_loan_owned, remap_borrow_weakening_owned, remap_borrow_writable_root_owned,
};

use crate::handles::{
    remap_borrow_activation_span, remap_borrow_call_span, remap_borrow_loan_span,
    remap_borrow_weakening_span, remap_borrow_writable_root_span,
};

pub(crate) fn remap_borrow_writable_roots(
    state_graph: &StateGraph,
) -> Arena<StateBorrowWritableRoot> {
    remap_arena(
        &state_graph.semantics.borrow.writable_roots,
        remap_borrow_writable_root_owned,
    )
}

pub(crate) fn remap_borrow_argument_accesses(
    state_graph: &StateGraph,
) -> Arena<StateBorrowArgumentAccess> {
    remap_arena(
        &state_graph.semantics.borrow.argument_accesses,
        remap_borrow_argument_access_owned,
    )
}

pub(crate) fn remap_borrow_calls(state_graph: &StateGraph) -> Arena<StateBorrowCall> {
    remap_arena(&state_graph.semantics.borrow.calls, remap_borrow_call_owned)
}

pub(crate) fn remap_borrow_loans(state_graph: &StateGraph) -> Arena<StateBorrowLoan> {
    remap_arena(&state_graph.semantics.borrow.loans, remap_borrow_loan_owned)
}

pub(crate) fn remap_borrow_activations(state_graph: &StateGraph) -> Arena<StateBorrowActivation> {
    remap_arena(
        &state_graph.semantics.borrow.activations,
        remap_borrow_activation_owned,
    )
}

pub(crate) fn remap_borrow_weakenings(state_graph: &StateGraph) -> Arena<StateBorrowWeakening> {
    remap_arena(
        &state_graph.semantics.borrow.weakenings,
        remap_borrow_weakening_owned,
    )
}

pub(crate) fn remap_borrow_summary(
    summary: &omega_state_graph::StateBorrowSummary,
) -> StateBorrowSummary {
    StateBorrowSummary {
        writable_roots: remap_borrow_writable_root_span(summary.writable_roots),
        mutable_parameter_count: summary.mutable_parameter_count,
        calls: remap_borrow_call_span(summary.calls),
        active_loans: remap_borrow_loan_span(summary.active_loans),
        activations: remap_borrow_activation_span(summary.activations),
        weakenings: remap_borrow_weakening_span(summary.weakenings),
    }
}

#[cfg(test)]
mod tests;
