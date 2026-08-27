use super::*;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[test]
fn remaps_write_only_argument_access_without_widening() {
    let remapped =
        remap_borrow_argument_access_owned(omega_state_graph::StateBorrowArgumentAccess {
            root_symbol: SymbolHandle::from_arena_index(1),
            segments: HandleSpan::empty(),
            kind: omega_state_graph::StateBorrowAccessKind::WriteOnly,
        });

    assert_eq!(
        remapped.kind,
        omega_control_flow::StateBorrowAccessKind::WriteOnly
    );
}

#[test]
fn remap_borrow_summary_preserves_all_borrow_spans() {
    let mut writable_roots = Arena::new();
    let mut calls = Arena::new();
    let mut loans = Arena::new();
    let mut activations = Arena::new();
    let mut weakenings = Arena::new();

    let mut writable_root_span = HandleSpan::empty();
    let mut call_span = HandleSpan::empty();
    let mut loan_span = HandleSpan::empty();
    let mut activation_span = HandleSpan::empty();
    let mut weakening_span = HandleSpan::empty();

    writable_roots.append_to_span(
        &mut writable_root_span,
        omega_state_graph::StateBorrowWritableRoot {
            symbol: SymbolHandle::from_arena_index(1),
            kind: omega_state_graph::StateBorrowRootKind::MutableParameter,
        },
    );
    calls.append_to_span(
        &mut call_span,
        omega_state_graph::StateBorrowCall {
            statement_index: 2,
            call_ordinal: 3,
            receiver_symbol: SymbolHandle::from_arena_index(4),
            target_symbol: SymbolHandle::from_arena_index(5),
            has_receiver: true,
            accesses: HandleSpan::empty(),
        },
    );
    let loan = loans.append_to_span(
        &mut loan_span,
        omega_state_graph::StateBorrowLoan {
            statement_index: 6,
            last_use_statement_index: 7,
            owner_symbol: SymbolHandle::from_arena_index(8),
            root_symbol: SymbolHandle::from_arena_index(9),
            segments: HandleSpan::empty(),
        },
    );
    activations.append_to_span(
        &mut activation_span,
        omega_state_graph::StateBorrowActivation {
            source: omega_state_graph::StateBorrowEventSource::Statement {
                statement_index: 10,
            },
            loan,
        },
    );
    weakenings.append_to_span(
        &mut weakening_span,
        omega_state_graph::StateBorrowWeakening {
            source: omega_state_graph::StateBorrowEventSource::Call {
                statement_index: 11,
                call_ordinal: 12,
                target_symbol: SymbolHandle::from_arena_index(13),
            },
            loan,
            reason: omega_state_graph::StateBorrowWeakeningReason::StateExit,
        },
    );

    let summary = remap_borrow_summary(&omega_state_graph::StateBorrowSummary {
        writable_roots: writable_root_span,
        mutable_parameter_count: 1,
        calls: call_span,
        active_loans: loan_span,
        activations: activation_span,
        weakenings: weakening_span,
    });

    assert_eq!(summary.mutable_parameter_count, 1);
    assert_same_span(summary.writable_roots, writable_root_span);
    assert_same_span(summary.calls, call_span);
    assert_same_span(summary.active_loans, loan_span);
    assert_same_span(summary.activations, activation_span);
    assert_same_span(summary.weakenings, weakening_span);
}

fn assert_same_span<Actual, Expected>(actual: HandleSpan<Actual>, expected: HandleSpan<Expected>) {
    assert_eq!(actual.count(), expected.count());
    assert_eq!(actual.start().arena_index(), expected.start().arena_index());
}
