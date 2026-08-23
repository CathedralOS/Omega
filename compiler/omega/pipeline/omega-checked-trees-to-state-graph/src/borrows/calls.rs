use omega_state_graph::{
    StateBorrowAccessKind, StateBorrowArgumentAccess, StateBorrowCall, StateGraph,
};
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;

pub(crate) fn state_borrow_calls(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    calls: HandleSpan<psi_checked_trees::BorrowCallFact>,
) -> HandleSpan<StateBorrowCall> {
    let mut state_calls = HandleSpan::empty();
    for call in program.facts.borrow.calls.span_or_empty(calls) {
        let accesses = state_graph.semantics.borrow.argument_accesses.insert_many(
            program
                .facts
                .borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .map(|access| StateBorrowArgumentAccess {
                    root_symbol: access.root_symbol,
                    segments: state_graph.semantics.borrow.access_segments.insert_many(
                        program
                            .facts
                            .borrow
                            .access_segments
                            .span_or_empty(access.segments)
                            .iter()
                            .copied(),
                    ),
                    kind: match access.kind {
                        psi_checked_trees::BorrowAccessKind::Read => StateBorrowAccessKind::Read,
                        psi_checked_trees::BorrowAccessKind::Mutable => {
                            StateBorrowAccessKind::Mutable
                        }
                        psi_checked_trees::BorrowAccessKind::WriteOnly => {
                            StateBorrowAccessKind::WriteOnly
                        }
                    },
                }),
        );

        state_graph.semantics.borrow.calls.append_to_span(
            &mut state_calls,
            StateBorrowCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                has_receiver: call.has_receiver,
                accesses,
            },
        );
    }

    state_calls
}
