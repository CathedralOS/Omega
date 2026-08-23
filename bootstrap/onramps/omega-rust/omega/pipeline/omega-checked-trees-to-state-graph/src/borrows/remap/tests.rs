use super::*;
use psi_symbols::SymbolHandle;

#[test]
fn remaps_borrow_summary_from_source_roots_into_target_roots() {
    let mut target = StateGraph::default();
    let mut writable_roots = Arena::new();
    let mut access_segments = Arena::new();
    let mut argument_accesses = Arena::new();
    let mut calls = Arena::new();
    let mut loans = Arena::new();
    let mut activations = Arena::new();
    let mut weakenings = Arena::new();

    let mut writable_root_span = HandleSpan::empty();
    let mut segment_span = HandleSpan::empty();
    let mut access_span = HandleSpan::empty();
    let mut call_span = HandleSpan::empty();
    let mut loan_span = HandleSpan::empty();
    let mut activation_span = HandleSpan::empty();
    let mut weakening_span = HandleSpan::empty();

    writable_roots.append_to_span(
        &mut writable_root_span,
        StateBorrowWritableRoot {
            symbol: SymbolHandle::from_arena_index(1),
            kind: omega_state_graph::StateBorrowRootKind::MutableParameter,
        },
    );
    access_segments.append_to_span(
        &mut segment_span,
        psi_facts::PlaceSegment::Field {
            symbol: SymbolHandle::from_arena_index(2),
        },
    );
    argument_accesses.append_to_span(
        &mut access_span,
        StateBorrowArgumentAccess {
            root_symbol: SymbolHandle::from_arena_index(3),
            segments: segment_span,
            kind: omega_state_graph::StateBorrowAccessKind::WriteOnly,
        },
    );
    calls.append_to_span(
        &mut call_span,
        StateBorrowCall {
            statement_index: 4,
            call_ordinal: 5,
            receiver_symbol: SymbolHandle::from_arena_index(6),
            target_symbol: SymbolHandle::from_arena_index(7),
            has_receiver: true,
            accesses: access_span,
        },
    );
    let loan = loans.append_to_span(
        &mut loan_span,
        StateBorrowLoan {
            statement_index: 8,
            last_use_statement_index: 9,
            owner_symbol: SymbolHandle::from_arena_index(10),
            root_symbol: SymbolHandle::from_arena_index(11),
            segments: segment_span,
        },
    );
    activations.append_to_span(
        &mut activation_span,
        StateBorrowActivation {
            source: omega_state_graph::StateBorrowEventSource::Statement {
                statement_index: 12,
            },
            loan,
        },
    );
    weakenings.append_to_span(
        &mut weakening_span,
        StateBorrowWeakening {
            source: omega_state_graph::StateBorrowEventSource::Call {
                statement_index: 13,
                call_ordinal: 14,
                target_symbol: SymbolHandle::from_arena_index(15),
            },
            loan,
            reason: omega_state_graph::StateBorrowWeakeningReason::StateExit,
        },
    );

    let remapped = remap_state_borrow_summary(
        &mut target,
        &SourceBorrowArenas {
            writable_roots: &writable_roots,
            access_segments: &access_segments,
            argument_accesses: &argument_accesses,
            calls: &calls,
            loans: &loans,
            activations: &activations,
            weakenings: &weakenings,
        },
        &StateBorrowSummary {
            writable_roots: writable_root_span,
            mutable_parameter_count: 1,
            calls: call_span,
            active_loans: loan_span,
            activations: activation_span,
            weakenings: weakening_span,
        },
    );

    assert_eq!(remapped.mutable_parameter_count, 1);
    assert_eq!(remapped.writable_roots.count(), 1);
    assert_eq!(remapped.calls.count(), 1);
    assert_eq!(remapped.active_loans.count(), 1);
    assert_eq!(remapped.activations.count(), 1);
    assert_eq!(remapped.weakenings.count(), 1);
    assert_eq!(target.semantics.borrow.writable_roots.len(), 1);
    assert_eq!(target.semantics.borrow.argument_accesses.len(), 1);
    assert_eq!(target.semantics.borrow.calls.len(), 1);
    assert_eq!(target.semantics.borrow.activations.len(), 1);
    assert_eq!(target.semantics.borrow.weakenings.len(), 1);

    let call = target
        .semantics
        .borrow
        .calls
        .span_or_empty(remapped.calls)
        .first()
        .unwrap();
    assert_eq!(call.accesses.count(), 1);
    let access = target
        .semantics
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .first()
        .unwrap();
    assert_eq!(
        access.kind,
        omega_state_graph::StateBorrowAccessKind::WriteOnly
    );
}
