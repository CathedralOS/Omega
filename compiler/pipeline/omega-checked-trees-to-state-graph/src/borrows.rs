mod calls;
mod lifetimes;
mod remap;

use omega_checked_trees::CheckedTrees;
use omega_state_graph::{
    StateBorrowRootKind, StateBorrowSummary, StateBorrowWritableRoot, StateGraph, StateKey,
};

use calls::state_borrow_calls;
use lifetimes::state_borrow_lifetime_summary;
pub(crate) use remap::remap_state_borrow_summary;

pub(crate) fn state_borrow_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateBorrowSummary {
    let Some(state_borrow) = program
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state_borrow)| {
            state_borrow.machine_symbol == key.machine && state_borrow.state_symbol == key.state
        })
        .map(|(_, state_borrow)| state_borrow)
    else {
        return StateBorrowSummary::default();
    };

    let writable_roots = state_graph.borrow_writable_roots.insert_many(
        program
            .facts
            .borrow
            .writable_roots
            .span_or_empty(state_borrow.writable_roots)
            .iter()
            .map(|root| StateBorrowWritableRoot {
                symbol: root.symbol,
                kind: match root.kind {
                    omega_checked_trees::BorrowRootKind::OwnedData => {
                        StateBorrowRootKind::OwnedData
                    }
                    omega_checked_trees::BorrowRootKind::LocalData => {
                        StateBorrowRootKind::LocalData
                    }
                    omega_checked_trees::BorrowRootKind::MutableParameter => {
                        StateBorrowRootKind::MutableParameter
                    }
                },
            }),
    );

    let calls = state_borrow_calls(state_graph, program, state_borrow.calls);
    let lifetime_summary = state_borrow_lifetime_summary(state_graph, program, key);

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: state_borrow.mutable_parameter_count,
        calls,
        active_loans: lifetime_summary.active_loans,
        activations: lifetime_summary.activations,
        weakenings: lifetime_summary.weakenings,
    }
}
