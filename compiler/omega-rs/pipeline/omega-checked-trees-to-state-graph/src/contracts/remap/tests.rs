use super::*;
use psi_symbols::SymbolHandle;

#[test]
fn remaps_contract_summary_from_source_roots_into_target_roots() {
    let mut target = StateGraph::default();
    let mut fact_refs = Arena::new();
    let mut calls = Arena::new();
    let mut exits = Arena::new();

    let mut requires = HandleSpan::empty();
    let mut ensures = HandleSpan::empty();
    let mut call_span = HandleSpan::empty();
    let mut exit_span = HandleSpan::empty();

    fact_refs.append_to_span(
        &mut requires,
        StateContractFactRef {
            kind: omega_state_graph::StateContractFactKind::Requires,
            fact: Default::default(),
        },
    );
    fact_refs.append_to_span(
        &mut ensures,
        StateContractFactRef {
            kind: omega_state_graph::StateContractFactKind::Ensures,
            fact: Default::default(),
        },
    );
    calls.append_to_span(
        &mut call_span,
        StateContractCall {
            statement_index: 1,
            call_ordinal: 2,
            target_machine_symbol: SymbolHandle::from_arena_index(3),
            target_state_symbol: SymbolHandle::from_arena_index(4),
            requires,
            ensures,
        },
    );
    exits.append_to_span(
        &mut exit_span,
        StateContractExit {
            statement_index: 5,
            ensures,
        },
    );

    let remapped = remap_state_contract_summary(
        &mut target,
        &SourceContractArenas {
            fact_refs: &fact_refs,
            calls: &calls,
            exits: &exits,
        },
        &StateContractSummary {
            calls: call_span,
            exits: exit_span,
        },
    );

    assert_eq!(remapped.calls.count(), 1);
    assert_eq!(remapped.exits.count(), 1);
    assert_eq!(target.semantics.contracts.calls.len(), 1);
    assert_eq!(target.semantics.contracts.exits.len(), 1);
    assert_eq!(target.semantics.contracts.fact_refs.len(), 3);

    let call = target
        .semantics
        .contracts
        .calls
        .span_or_empty(remapped.calls)
        .first()
        .unwrap();
    assert_eq!(call.requires.count(), 1);
    assert_eq!(call.ensures.count(), 1);
}
