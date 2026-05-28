mod remap;

use omega_checked_trees::CheckedTrees;
use omega_core::arena::{Handle, HandleSpan};
use omega_state_graph::{
    StateBorrowAccessKind, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowEventSource, StateBorrowLoan, StateBorrowRootKind, StateBorrowSummary,
    StateBorrowWeakening, StateBorrowWeakeningReason, StateBorrowWritableRoot, StateGraph,
    StateKey,
};

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

    let mut calls = HandleSpan::empty();
    for call in program.facts.borrow.calls.span_or_empty(state_borrow.calls) {
        let accesses = state_graph.borrow_argument_accesses.insert_many(
            program
                .facts
                .borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .map(|access| StateBorrowArgumentAccess {
                    root_symbol: access.root_symbol,
                    segments: state_graph.borrow_access_segments.insert_many(
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

        state_graph.borrow_calls.append_to_span(
            &mut calls,
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

    let mut active_loan_start: Option<Handle<StateBorrowLoan>> = None;
    let mut active_loan_count = 0usize;
    let mut activations = HandleSpan::empty();
    let mut weakenings = HandleSpan::empty();
    let mut loan_map: Vec<(
        Handle<omega_checked_trees::BorrowLoanFact>,
        Handle<StateBorrowLoan>,
    )> = Vec::new();

    if let Some(flow_state) = program
        .facts
        .flow
        .states
        .iter()
        .find(|(_, state)| state.machine_symbol == key.machine && state.state_symbol == key.state)
        .map(|(_, state)| state)
    {
        for source_loan in program
            .facts
            .flow
            .borrow_loan_constraints(flow_state.entry_constraints)
        {
            let loan = ensure_state_borrow_loan(state_graph, program, source_loan, &mut loan_map);
            active_loan_start.get_or_insert(loan);
            active_loan_count += 1;
        }

        for activation in program
            .facts
            .flow
            .borrow_activations
            .span_or_empty(flow_state.borrow_activations)
        {
            let loan =
                ensure_state_borrow_loan(state_graph, program, activation.loan, &mut loan_map);
            state_graph.borrow_activations.append_to_span(
                &mut activations,
                StateBorrowActivation {
                    source: remap_flow_borrow_event_source(activation.source),
                    loan,
                },
            );
        }

        for weakening in program
            .facts
            .flow
            .borrow_weakenings
            .span_or_empty(flow_state.borrow_weakenings)
        {
            let loan =
                ensure_state_borrow_loan(state_graph, program, weakening.loan, &mut loan_map);
            state_graph.borrow_weakenings.append_to_span(
                &mut weakenings,
                StateBorrowWeakening {
                    source: remap_flow_borrow_event_source(weakening.source),
                    loan,
                    reason: remap_flow_borrow_weakening_reason(weakening.reason),
                },
            );
        }
    }

    let active_loans = active_loan_start
        .map(|start| {
            HandleSpan::from_parts(
                start,
                active_loan_count
                    .try_into()
                    .expect("active loan count should fit in u32"),
            )
        })
        .unwrap_or_else(HandleSpan::empty);

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: state_borrow.mutable_parameter_count,
        calls,
        active_loans,
        activations,
        weakenings,
    }
}

fn ensure_state_borrow_loan(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    source_loan: Handle<omega_checked_trees::BorrowLoanFact>,
    loan_map: &mut Vec<(
        Handle<omega_checked_trees::BorrowLoanFact>,
        Handle<StateBorrowLoan>,
    )>,
) -> Handle<StateBorrowLoan> {
    if let Some((_, mapped)) = loan_map
        .iter()
        .find(|(candidate, _)| *candidate == source_loan)
    {
        return *mapped;
    }

    let loan = program.facts.borrow.loans.get(source_loan);
    let mapped = state_graph.borrow_loans.append(StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: state_graph.borrow_access_segments.insert_many(
            program
                .facts
                .borrow
                .access_segments
                .span_or_empty(loan.segments)
                .iter()
                .copied(),
        ),
    });
    loan_map.push((source_loan, mapped));
    mapped
}

fn remap_flow_borrow_event_source(
    source: omega_checked_trees::FlowInvalidationSource,
) -> StateBorrowEventSource {
    match source {
        omega_checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            StateBorrowEventSource::Statement { statement_index }
        }
        omega_checked_trees::FlowInvalidationSource::Call {
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

fn remap_flow_borrow_weakening_reason(
    reason: omega_checked_trees::FlowBorrowWeakeningReason,
) -> StateBorrowWeakeningReason {
    match reason {
        omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired => {
            StateBorrowWeakeningReason::LastUseExpired
        }
        omega_checked_trees::FlowBorrowWeakeningReason::StateExit => {
            StateBorrowWeakeningReason::StateExit
        }
        omega_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
            StateBorrowWeakeningReason::LocalReassigned
        }
    }
}
