use omega_checked_trees::CheckedTrees;
use omega_core::arena::HandleSpan;
use omega_state_graph::{
    StateBorrowAccessKind, StateBorrowArgumentAccess, StateBorrowCall, StateGraph,
};

pub(crate) fn state_borrow_calls(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    calls: HandleSpan<omega_checked_trees::BorrowCallFact>,
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
                        omega_checked_trees::BorrowAccessKind::Read => StateBorrowAccessKind::Read,
                        omega_checked_trees::BorrowAccessKind::Mutable => {
                            StateBorrowAccessKind::Mutable
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
