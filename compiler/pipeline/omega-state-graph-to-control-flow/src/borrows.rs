use omega_control_flow::{
    StateBorrowAccessKind, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowEventSource, StateBorrowLoan, StateBorrowRootKind, StateBorrowSummary,
    StateBorrowWeakening, StateBorrowWeakeningReason, StateBorrowWritableRoot,
};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

use crate::handles::{
    remap_borrow_activation_span, remap_borrow_argument_access_span, remap_borrow_call_span,
    remap_borrow_loan_handle, remap_borrow_loan_span, remap_borrow_weakening_span,
    remap_borrow_writable_root_span,
};

pub(crate) fn remap_borrow_writable_roots(
    state_graph: &StateGraph,
) -> Arena<StateBorrowWritableRoot> {
    let mut writable_roots = Arena::with_capacity(state_graph.borrow_writable_roots.len());

    for (_, root) in state_graph.borrow_writable_roots.iter() {
        writable_roots.append(StateBorrowWritableRoot {
            symbol: root.symbol,
            kind: match root.kind {
                omega_state_graph::StateBorrowRootKind::OwnedData => StateBorrowRootKind::OwnedData,
                omega_state_graph::StateBorrowRootKind::LocalData => StateBorrowRootKind::LocalData,
                omega_state_graph::StateBorrowRootKind::MutableParameter => {
                    StateBorrowRootKind::MutableParameter
                }
            },
        });
    }

    writable_roots
}

pub(crate) fn remap_borrow_writable_root_owned(
    root: omega_state_graph::StateBorrowWritableRoot,
) -> StateBorrowWritableRoot {
    StateBorrowWritableRoot {
        symbol: root.symbol,
        kind: match root.kind {
            omega_state_graph::StateBorrowRootKind::OwnedData => StateBorrowRootKind::OwnedData,
            omega_state_graph::StateBorrowRootKind::LocalData => StateBorrowRootKind::LocalData,
            omega_state_graph::StateBorrowRootKind::MutableParameter => {
                StateBorrowRootKind::MutableParameter
            }
        },
    }
}

pub(crate) fn remap_borrow_argument_accesses(
    state_graph: &StateGraph,
) -> Arena<StateBorrowArgumentAccess> {
    let mut accesses = Arena::with_capacity(state_graph.borrow_argument_accesses.len());

    for (_, access) in state_graph.borrow_argument_accesses.iter() {
        accesses.append(StateBorrowArgumentAccess {
            root_symbol: access.root_symbol,
            segments: access.segments,
            kind: match access.kind {
                omega_state_graph::StateBorrowAccessKind::Read => StateBorrowAccessKind::Read,
                omega_state_graph::StateBorrowAccessKind::Mutable => StateBorrowAccessKind::Mutable,
            },
        });
    }

    accesses
}

pub(crate) fn remap_borrow_argument_access_owned(
    access: omega_state_graph::StateBorrowArgumentAccess,
) -> StateBorrowArgumentAccess {
    StateBorrowArgumentAccess {
        root_symbol: access.root_symbol,
        segments: access.segments,
        kind: match access.kind {
            omega_state_graph::StateBorrowAccessKind::Read => StateBorrowAccessKind::Read,
            omega_state_graph::StateBorrowAccessKind::Mutable => StateBorrowAccessKind::Mutable,
        },
    }
}

pub(crate) fn remap_borrow_calls(state_graph: &StateGraph) -> Arena<StateBorrowCall> {
    let mut calls = Arena::with_capacity(state_graph.borrow_calls.len());

    for (_, call) in state_graph.borrow_calls.iter() {
        calls.append(StateBorrowCall {
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            receiver_symbol: call.receiver_symbol,
            target_symbol: call.target_symbol,
            has_receiver: call.has_receiver,
            accesses: remap_borrow_argument_access_span(call.accesses),
        });
    }

    calls
}

pub(crate) fn remap_borrow_loans(state_graph: &StateGraph) -> Arena<StateBorrowLoan> {
    let mut loans = Arena::with_capacity(state_graph.borrow_loans.len());

    for (_, loan) in state_graph.borrow_loans.iter() {
        loans.append(remap_borrow_loan(loan));
    }

    loans
}

pub(crate) fn remap_borrow_activations(state_graph: &StateGraph) -> Arena<StateBorrowActivation> {
    let mut activations = Arena::with_capacity(state_graph.borrow_activations.len());

    for (_, activation) in state_graph.borrow_activations.iter() {
        activations.append(remap_borrow_activation(activation));
    }

    activations
}

pub(crate) fn remap_borrow_weakenings(state_graph: &StateGraph) -> Arena<StateBorrowWeakening> {
    let mut weakenings = Arena::with_capacity(state_graph.borrow_weakenings.len());

    for (_, weakening) in state_graph.borrow_weakenings.iter() {
        weakenings.append(remap_borrow_weakening(weakening));
    }

    weakenings
}

pub(crate) fn remap_borrow_call_owned(call: omega_state_graph::StateBorrowCall) -> StateBorrowCall {
    StateBorrowCall {
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
        receiver_symbol: call.receiver_symbol,
        target_symbol: call.target_symbol,
        has_receiver: call.has_receiver,
        accesses: remap_borrow_argument_access_span(call.accesses),
    }
}

fn remap_borrow_loan(loan: &omega_state_graph::StateBorrowLoan) -> StateBorrowLoan {
    StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: loan.segments,
    }
}

pub(crate) fn remap_borrow_loan_owned(loan: omega_state_graph::StateBorrowLoan) -> StateBorrowLoan {
    StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: loan.segments,
    }
}

fn remap_borrow_event_source(
    source: omega_state_graph::StateBorrowEventSource,
) -> StateBorrowEventSource {
    match source {
        omega_state_graph::StateBorrowEventSource::Statement { statement_index } => {
            StateBorrowEventSource::Statement { statement_index }
        }
        omega_state_graph::StateBorrowEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => StateBorrowEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
    }
}

fn remap_borrow_activation(
    activation: &omega_state_graph::StateBorrowActivation,
) -> StateBorrowActivation {
    StateBorrowActivation {
        source: remap_borrow_event_source(activation.source),
        loan: remap_borrow_loan_handle(activation.loan),
    }
}

pub(crate) fn remap_borrow_activation_owned(
    activation: omega_state_graph::StateBorrowActivation,
) -> StateBorrowActivation {
    StateBorrowActivation {
        source: remap_borrow_event_source(activation.source),
        loan: remap_borrow_loan_handle(activation.loan),
    }
}

fn remap_borrow_weakening_reason(
    reason: omega_state_graph::StateBorrowWeakeningReason,
) -> StateBorrowWeakeningReason {
    match reason {
        omega_state_graph::StateBorrowWeakeningReason::LastUseExpired => {
            StateBorrowWeakeningReason::LastUseExpired
        }
        omega_state_graph::StateBorrowWeakeningReason::StateExit => {
            StateBorrowWeakeningReason::StateExit
        }
        omega_state_graph::StateBorrowWeakeningReason::LocalReassigned => {
            StateBorrowWeakeningReason::LocalReassigned
        }
    }
}

fn remap_borrow_weakening(
    weakening: &omega_state_graph::StateBorrowWeakening,
) -> StateBorrowWeakening {
    StateBorrowWeakening {
        source: remap_borrow_event_source(weakening.source),
        loan: remap_borrow_loan_handle(weakening.loan),
        reason: remap_borrow_weakening_reason(weakening.reason),
    }
}

pub(crate) fn remap_borrow_weakening_owned(
    weakening: omega_state_graph::StateBorrowWeakening,
) -> StateBorrowWeakening {
    StateBorrowWeakening {
        source: remap_borrow_event_source(weakening.source),
        loan: remap_borrow_loan_handle(weakening.loan),
        reason: remap_borrow_weakening_reason(weakening.reason),
    }
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
