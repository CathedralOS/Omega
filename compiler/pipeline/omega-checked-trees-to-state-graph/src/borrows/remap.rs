use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_state_graph::{
    StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall, StateBorrowLoan,
    StateBorrowSummary, StateBorrowWeakening, StateBorrowWritableRoot, StateGraph,
};

pub(crate) fn remap_state_borrow_summary(
    target: &mut StateGraph,
    source_writable_roots: &Arena<StateBorrowWritableRoot>,
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_calls: &Arena<StateBorrowCall>,
    source_loans: &Arena<StateBorrowLoan>,
    source_activations: &Arena<StateBorrowActivation>,
    source_weakenings: &Arena<StateBorrowWeakening>,
    borrow: &StateBorrowSummary,
) -> StateBorrowSummary {
    let writable_roots = target.semantics.borrow_writable_roots.insert_many(
        source_writable_roots
            .span_or_empty(borrow.writable_roots)
            .iter()
            .cloned(),
    );

    let calls = append_remapped_borrow_calls(
        target,
        source_access_segments,
        source_argument_accesses,
        source_calls,
        borrow.calls,
    );
    let active_loans = append_remapped_borrow_loans(
        target,
        source_access_segments,
        source_loans,
        borrow.active_loans,
    );
    let activations = append_remapped_borrow_activations(
        target,
        source_access_segments,
        source_loans,
        source_activations,
        borrow.activations,
    );
    let weakenings = append_remapped_borrow_weakenings(
        target,
        source_access_segments,
        source_loans,
        source_weakenings,
        borrow.weakenings,
    );

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
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_calls: &Arena<StateBorrowCall>,
    calls: HandleSpan<StateBorrowCall>,
) -> HandleSpan<StateBorrowCall> {
    let mut remapped_calls = HandleSpan::empty();

    for call in source_calls.span_or_empty(calls) {
        let accesses = target.semantics.borrow_argument_accesses.insert_many(
            source_argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .map(|access| StateBorrowArgumentAccess {
                    root_symbol: access.root_symbol,
                    segments: target.semantics.borrow_access_segments.insert_many(
                        source_access_segments
                            .span_or_empty(access.segments)
                            .iter()
                            .copied(),
                    ),
                    kind: access.kind.clone(),
                }),
        );

        target.semantics.borrow_calls.append_to_span(
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
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    loans: HandleSpan<StateBorrowLoan>,
) -> HandleSpan<StateBorrowLoan> {
    let mut remapped_loans = HandleSpan::empty();

    for loan in source_loans.span_or_empty(loans) {
        target.semantics.borrow_loans.append_to_span(
            &mut remapped_loans,
            StateBorrowLoan {
                statement_index: loan.statement_index,
                last_use_statement_index: loan.last_use_statement_index,
                owner_symbol: loan.owner_symbol,
                root_symbol: loan.root_symbol,
                segments: target.semantics.borrow_access_segments.insert_many(
                    source_access_segments
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
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    source_activations: &Arena<StateBorrowActivation>,
    activations: HandleSpan<StateBorrowActivation>,
) -> HandleSpan<StateBorrowActivation> {
    let mut remapped = HandleSpan::empty();
    let mut loan_map: Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)> = Vec::new();

    for activation in source_activations.span_or_empty(activations) {
        let loan = remapped_loan_handle(
            target,
            source_access_segments,
            source_loans,
            activation.loan,
            &mut loan_map,
        );
        target.semantics.borrow_activations.append_to_span(
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
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    source_weakenings: &Arena<StateBorrowWeakening>,
    weakenings: HandleSpan<StateBorrowWeakening>,
) -> HandleSpan<StateBorrowWeakening> {
    let mut remapped = HandleSpan::empty();
    let mut loan_map: Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)> = Vec::new();

    for weakening in source_weakenings.span_or_empty(weakenings) {
        let loan = remapped_loan_handle(
            target,
            source_access_segments,
            source_loans,
            weakening.loan,
            &mut loan_map,
        );
        target.semantics.borrow_weakenings.append_to_span(
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
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    source_loan: Handle<StateBorrowLoan>,
    loan_map: &mut Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)>,
) -> Handle<StateBorrowLoan> {
    if let Some((_, mapped)) = loan_map
        .iter()
        .find(|(candidate, _)| *candidate == source_loan)
    {
        return *mapped;
    }

    let loan = source_loans.get(source_loan);
    let mapped = target.semantics.borrow_loans.append(StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: target.semantics.borrow_access_segments.insert_many(
            source_access_segments
                .span_or_empty(loan.segments)
                .iter()
                .copied(),
        ),
    });
    loan_map.push((source_loan, mapped));
    mapped
}
