mod conversions;

use omega_control_flow::{
    StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall, StateBorrowLoan,
    StateBorrowSummary, StateBorrowWeakening, StateBorrowWritableRoot,
};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

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
    let mut writable_roots =
        Arena::with_capacity(state_graph.semantics.borrow.writable_roots.len());

    for (_, root) in state_graph.semantics.borrow.writable_roots.iter() {
        writable_roots.append(remap_borrow_writable_root_owned(root.clone()));
    }

    writable_roots
}

pub(crate) fn remap_borrow_argument_accesses(
    state_graph: &StateGraph,
) -> Arena<StateBorrowArgumentAccess> {
    let mut accesses = Arena::with_capacity(state_graph.semantics.borrow.argument_accesses.len());

    for (_, access) in state_graph.semantics.borrow.argument_accesses.iter() {
        accesses.append(remap_borrow_argument_access_owned(access.clone()));
    }

    accesses
}

pub(crate) fn remap_borrow_calls(state_graph: &StateGraph) -> Arena<StateBorrowCall> {
    let mut calls = Arena::with_capacity(state_graph.semantics.borrow.calls.len());

    for (_, call) in state_graph.semantics.borrow.calls.iter() {
        calls.append(remap_borrow_call_owned(call.clone()));
    }

    calls
}

pub(crate) fn remap_borrow_loans(state_graph: &StateGraph) -> Arena<StateBorrowLoan> {
    let mut loans = Arena::with_capacity(state_graph.semantics.borrow.loans.len());

    for (_, loan) in state_graph.semantics.borrow.loans.iter() {
        loans.append(remap_borrow_loan_owned(loan.clone()));
    }

    loans
}

pub(crate) fn remap_borrow_activations(state_graph: &StateGraph) -> Arena<StateBorrowActivation> {
    let mut activations = Arena::with_capacity(state_graph.semantics.borrow.activations.len());

    for (_, activation) in state_graph.semantics.borrow.activations.iter() {
        activations.append(remap_borrow_activation_owned(activation.clone()));
    }

    activations
}

pub(crate) fn remap_borrow_weakenings(state_graph: &StateGraph) -> Arena<StateBorrowWeakening> {
    let mut weakenings = Arena::with_capacity(state_graph.semantics.borrow.weakenings.len());

    for (_, weakening) in state_graph.semantics.borrow.weakenings.iter() {
        weakenings.append(remap_borrow_weakening_owned(weakening.clone()));
    }

    weakenings
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
