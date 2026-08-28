use omega_control_flow::{
    StateBorrowAccessKind, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowEventSource, StateBorrowLoan, StateBorrowRootKind, StateBorrowWeakening,
    StateBorrowWeakeningReason, StateBorrowWritableRoot,
};

use crate::handles::{remap_borrow_argument_access_span, remap_borrow_loan_handle};

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

pub(crate) fn remap_borrow_argument_access_owned(
    access: omega_state_graph::StateBorrowArgumentAccess,
) -> StateBorrowArgumentAccess {
    StateBorrowArgumentAccess {
        root_symbol: access.root_symbol,
        segments: access.segments,
        kind: match access.kind {
            omega_state_graph::StateBorrowAccessKind::Read => StateBorrowAccessKind::Read,
            omega_state_graph::StateBorrowAccessKind::Mutable => StateBorrowAccessKind::Mutable,
            omega_state_graph::StateBorrowAccessKind::WriteOnly => StateBorrowAccessKind::WriteOnly,
        },
    }
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

pub(crate) fn remap_borrow_loan_owned(loan: omega_state_graph::StateBorrowLoan) -> StateBorrowLoan {
    StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: loan.segments,
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

pub(crate) fn remap_borrow_weakening_owned(
    weakening: omega_state_graph::StateBorrowWeakening,
) -> StateBorrowWeakening {
    StateBorrowWeakening {
        source: remap_borrow_event_source(weakening.source),
        loan: remap_borrow_loan_handle(weakening.loan),
        reason: remap_borrow_weakening_reason(weakening.reason),
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
