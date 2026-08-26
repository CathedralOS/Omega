use super::*;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[test]
fn remap_contract_summary_preserves_call_and_exit_spans() {
    let mut calls = Arena::new();
    let mut exits = Arena::new();
    let mut call_span = HandleSpan::empty();
    let mut exit_span = HandleSpan::empty();

    calls.append_to_span(
        &mut call_span,
        omega_state_graph::StateContractCall {
            statement_index: 1,
            call_ordinal: 2,
            target_machine_symbol: SymbolHandle::from_arena_index(3),
            target_state_symbol: SymbolHandle::from_arena_index(4),
            requires: HandleSpan::empty(),
            ensures: HandleSpan::empty(),
        },
    );
    exits.append_to_span(
        &mut exit_span,
        omega_state_graph::StateContractExit {
            statement_index: 5,
            ensures: HandleSpan::empty(),
        },
    );

    let summary = remap_contract_summary(&omega_state_graph::StateContractSummary {
        calls: call_span,
        exits: exit_span,
    });

    assert_same_span(summary.calls, call_span);
    assert_same_span(summary.exits, exit_span);
}

#[test]
fn remap_owned_contract_call_preserves_fact_ref_spans() {
    let mut fact_refs = Arena::new();
    let mut requires = HandleSpan::empty();
    let mut ensures = HandleSpan::empty();

    fact_refs.append_to_span(
        &mut requires,
        omega_state_graph::StateContractFactRef {
            kind: omega_state_graph::StateContractFactKind::Requires,
            fact: Default::default(),
        },
    );
    fact_refs.append_to_span(
        &mut ensures,
        omega_state_graph::StateContractFactRef {
            kind: omega_state_graph::StateContractFactKind::Ensures,
            fact: Default::default(),
        },
    );

    let call = remap_contract_call_owned(omega_state_graph::StateContractCall {
        statement_index: 1,
        call_ordinal: 2,
        target_machine_symbol: SymbolHandle::from_arena_index(3),
        target_state_symbol: SymbolHandle::from_arena_index(4),
        requires,
        ensures,
    });

    assert_same_span(call.requires, requires);
    assert_same_span(call.ensures, ensures);
}

fn assert_same_span<Actual, Expected>(actual: HandleSpan<Actual>, expected: HandleSpan<Expected>) {
    assert_eq!(actual.count(), expected.count());
    assert_eq!(actual.start().arena_index(), expected.start().arena_index());
}
