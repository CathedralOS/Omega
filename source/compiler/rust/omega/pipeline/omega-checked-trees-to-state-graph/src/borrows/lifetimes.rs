use omega_state_graph::{
    StateBorrowActivation, StateBorrowEventSource, StateBorrowLoan, StateBorrowWeakening,
    StateBorrowWeakeningReason, StateGraph, StateKey,
};
use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::CheckedTrees;

pub(crate) struct StateBorrowLifetimeSummary {
    pub(crate) active_loans: HandleSpan<StateBorrowLoan>,
    pub(crate) activations: HandleSpan<StateBorrowActivation>,
    pub(crate) weakenings: HandleSpan<StateBorrowWeakening>,
}

pub(crate) fn state_borrow_lifetime_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateBorrowLifetimeSummary {
    let mut active_loan_start: Option<Handle<StateBorrowLoan>> = None;
    let mut active_loan_count = 0usize;
    let mut activations = HandleSpan::empty();
    let mut weakenings = HandleSpan::empty();
    let mut loan_map: Vec<(
        Handle<psi_checked_trees::BorrowLoanFact>,
        Handle<StateBorrowLoan>,
    )> = Vec::new();

    if let Some(flow_state) = program
        .facts
        .flow
        .control
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
            .borrow_lifetimes
            .activations
            .span_or_empty(flow_state.borrow_activations)
        {
            let loan =
                ensure_state_borrow_loan(state_graph, program, activation.loan, &mut loan_map);
            state_graph.semantics.borrow.activations.append_to_span(
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
            .borrow_lifetimes
            .weakenings
            .span_or_empty(flow_state.borrow_weakenings)
        {
            let loan =
                ensure_state_borrow_loan(state_graph, program, weakening.loan, &mut loan_map);
            state_graph.semantics.borrow.weakenings.append_to_span(
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

    StateBorrowLifetimeSummary {
        active_loans,
        activations,
        weakenings,
    }
}

fn ensure_state_borrow_loan(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    source_loan: Handle<psi_checked_trees::BorrowLoanFact>,
    loan_map: &mut Vec<(
        Handle<psi_checked_trees::BorrowLoanFact>,
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
    let mapped = state_graph.semantics.borrow.loans.append(StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: state_graph.semantics.borrow.access_segments.insert_many(
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
    source: psi_checked_trees::FlowInvalidationSource,
) -> StateBorrowEventSource {
    match source {
        psi_checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            StateBorrowEventSource::Statement { statement_index }
        }
        psi_checked_trees::FlowInvalidationSource::Call {
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
    reason: psi_checked_trees::FlowBorrowWeakeningReason,
) -> StateBorrowWeakeningReason {
    match reason {
        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired => {
            StateBorrowWeakeningReason::LastUseExpired
        }
        psi_checked_trees::FlowBorrowWeakeningReason::StateExit => {
            StateBorrowWeakeningReason::StateExit
        }
        psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
            StateBorrowWeakeningReason::LocalReassigned
        }
    }
}
