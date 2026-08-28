use omega_state_graph::{
    StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall, StateBorrowLoan,
    StateBorrowSummary, StateBorrowWeakening, StateBorrowWritableRoot, StateGraph,
};
use psi_arena::{Arena, Handle, HandleSpan};

pub(crate) struct SourceBorrowArenas<'a> {
    pub(crate) writable_roots: &'a Arena<StateBorrowWritableRoot>,
    pub(crate) access_segments: &'a Arena<psi_facts::PlaceSegment>,
    pub(crate) argument_accesses: &'a Arena<StateBorrowArgumentAccess>,
    pub(crate) calls: &'a Arena<StateBorrowCall>,
    pub(crate) loans: &'a Arena<StateBorrowLoan>,
    pub(crate) activations: &'a Arena<StateBorrowActivation>,
    pub(crate) weakenings: &'a Arena<StateBorrowWeakening>,
}

pub(crate) fn remap_state_borrow_summary(
    target: &mut StateGraph,
    source: &SourceBorrowArenas<'_>,
    borrow: &StateBorrowSummary,
) -> StateBorrowSummary {
    let writable_roots = target.semantics.borrow.writable_roots.insert_many(
        source
            .writable_roots
            .span_or_empty(borrow.writable_roots)
            .iter()
            .cloned(),
    );

    let calls = append_remapped_borrow_calls(target, source, borrow.calls);
    let active_loans = append_remapped_borrow_loans(target, source, borrow.active_loans);
    let activations = append_remapped_borrow_activations(target, source, borrow.activations);
    let weakenings = append_remapped_borrow_weakenings(target, source, borrow.weakenings);

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: borrow.mutable_parameter_count,
        calls,
        active_loans,
        activations,
        weakenings,
    }
}

fn append_remapped_borrow_calls(
    target: &mut StateGraph,
    source: &SourceBorrowArenas<'_>,
    calls: HandleSpan<StateBorrowCall>,
) -> HandleSpan<StateBorrowCall> {
    let mut remapped_calls = HandleSpan::empty();

    for call in source.calls.span_or_empty(calls) {
        let accesses = target.semantics.borrow.argument_accesses.insert_many(
            source
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .map(|access| StateBorrowArgumentAccess {
                    root_symbol: access.root_symbol,
                    segments: target.semantics.borrow.access_segments.insert_many(
                        source
                            .access_segments
                            .span_or_empty(access.segments)
                            .iter()
                            .copied(),
                    ),
                    kind: access.kind.clone(),
                }),
        );

        target.semantics.borrow.calls.append_to_span(
            &mut remapped_calls,
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

    remapped_calls
}

fn append_remapped_borrow_loans(
    target: &mut StateGraph,
    source: &SourceBorrowArenas<'_>,
    loans: HandleSpan<StateBorrowLoan>,
) -> HandleSpan<StateBorrowLoan> {
    let mut remapped_loans = HandleSpan::empty();

    for loan in source.loans.span_or_empty(loans) {
        target.semantics.borrow.loans.append_to_span(
            &mut remapped_loans,
            StateBorrowLoan {
                statement_index: loan.statement_index,
                last_use_statement_index: loan.last_use_statement_index,
                owner_symbol: loan.owner_symbol,
                root_symbol: loan.root_symbol,
                segments: target.semantics.borrow.access_segments.insert_many(
                    source
                        .access_segments
                        .span_or_empty(loan.segments)
                        .iter()
                        .copied(),
                ),
            },
        );
    }

    remapped_loans
}

fn append_remapped_borrow_activations(
    target: &mut StateGraph,
    source: &SourceBorrowArenas<'_>,
    activations: HandleSpan<StateBorrowActivation>,
) -> HandleSpan<StateBorrowActivation> {
    let mut remapped = HandleSpan::empty();
    let mut loan_map: Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)> = Vec::new();

    for activation in source.activations.span_or_empty(activations) {
        let loan = remapped_loan_handle(target, source, activation.loan, &mut loan_map);
        target.semantics.borrow.activations.append_to_span(
            &mut remapped,
            StateBorrowActivation {
                source: activation.source.clone(),
                loan,
            },
        );
    }

    remapped
}

fn append_remapped_borrow_weakenings(
    target: &mut StateGraph,
    source: &SourceBorrowArenas<'_>,
    weakenings: HandleSpan<StateBorrowWeakening>,
) -> HandleSpan<StateBorrowWeakening> {
    let mut remapped = HandleSpan::empty();
    let mut loan_map: Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)> = Vec::new();

    for weakening in source.weakenings.span_or_empty(weakenings) {
        let loan = remapped_loan_handle(target, source, weakening.loan, &mut loan_map);
        target.semantics.borrow.weakenings.append_to_span(
            &mut remapped,
            StateBorrowWeakening {
                source: weakening.source.clone(),
                loan,
                reason: weakening.reason,
            },
        );
    }

    remapped
}

fn remapped_loan_handle(
    target: &mut StateGraph,
    source: &SourceBorrowArenas<'_>,
    source_loan: Handle<StateBorrowLoan>,
    loan_map: &mut Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)>,
) -> Handle<StateBorrowLoan> {
    if let Some((_, mapped)) = loan_map
        .iter()
        .find(|(candidate, _)| *candidate == source_loan)
    {
        return *mapped;
    }

    let loan = source.loans.get(source_loan);
    let mapped = target.semantics.borrow.loans.append(StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: target.semantics.borrow.access_segments.insert_many(
            source
                .access_segments
                .span_or_empty(loan.segments)
                .iter()
                .copied(),
        ),
    });
    loan_map.push((source_loan, mapped));
    mapped
}

#[cfg(test)]
mod tests;
