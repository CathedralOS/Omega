use super::*;
use std::cell::Cell;
use terminal_psi::{OperationKind, OperationResult, ValueDeclaration};

fn site(position: u32) -> NominalMachineUseSite {
    NominalMachineUseSite::Statement(arena::Handle::from_parts(position, 3))
}

fn source_call(position: u32) -> LoweredSourceCallOccurrence {
    LoweredSourceCallOccurrence {
        source_site: Some(site(position)),
        source_state: SymbolHandle::from_arena_index(1),
        statement_index: position as usize,
        call_ordinal: 0,
        terminal_operation: OperationId::new(u64::from(position)).expect("nonzero operation"),
        source_target: SymbolHandle::from_parts(7, 2),
        source_values_before_call: Vec::<ValueDeclaration>::new(),
    }
}

fn operation(position: u32) -> Operation {
    Operation {
        static_reach_binding: None,
        id: OperationId::new(u64::from(position)).expect("nonzero operation"),
        result: OperationResult::Unit,
        kind: OperationKind::BooleanConstant { value: false },
    }
}

#[test]
fn source_joins_retain_only_requested_keys_and_preserve_exact_matches() {
    let mut calls = (1..=4096).map(source_call).collect::<Vec<_>>();
    calls.reverse();
    let target = calls[0].source_target;
    let requested = [3000, 1, 40, 3000];
    let matches = CallbackSourceCalls::new(
        requested.iter().map(|position| (site(*position), target)),
        &calls,
    );
    assert_eq!(matches.requested.len(), 3);
    for position in requested {
        let actual = matches
            .find(site(position), target)
            .expect("one exact source call");
        let expected = calls
            .iter()
            .find(|call| call.source_site == Some(site(position)))
            .unwrap();
        assert!(std::ptr::eq(actual, expected));
    }
    assert_eq!(matches.find(site(5000), target), Err(0));
}

#[test]
fn source_join_identity_includes_site_variant_and_both_generations() {
    let exact = source_call(1);
    let mut statement_generation = exact.clone();
    statement_generation.source_site = Some(NominalMachineUseSite::Statement(
        arena::Handle::from_parts(1, 4),
    ));
    let mut expression = exact.clone();
    expression.source_site = Some(NominalMachineUseSite::Expression(
        arena::Handle::from_parts(1, 3),
    ));
    let mut target_generation = exact.clone();
    target_generation.source_target = SymbolHandle::from_parts(7, 3);
    let mut absent_site = exact.clone();
    absent_site.source_site = None;
    let calls = [
        statement_generation,
        expression,
        target_generation,
        absent_site,
        exact,
    ];
    let matches =
        CallbackSourceCalls::new(std::iter::once((site(1), calls[4].source_target)), &calls);
    assert!(std::ptr::eq(
        matches.find(site(1), calls[4].source_target).unwrap(),
        &calls[4]
    ));
}

#[test]
fn source_join_retains_duplicate_counts_instead_of_selecting_a_winner() {
    let calls = [
        source_call(2),
        source_call(1),
        source_call(1),
        source_call(1),
        source_call(4),
        source_call(4),
    ];
    let target = calls[0].source_target;
    let matches = CallbackSourceCalls::new(
        [1, 2, 3]
            .into_iter()
            .map(|position| (site(position), target)),
        &calls,
    );
    assert_eq!(matches.find(site(1), target), Err(3));
    assert_eq!(matches.find(site(3), target), Err(0));
    assert!(std::ptr::eq(
        matches.find(site(2), target).unwrap(),
        &calls[0]
    ));
}

#[test]
fn terminal_join_visits_input_once_and_keeps_only_requested_operations() {
    let operations = (1..=4096).rev().map(operation).collect::<Vec<_>>();
    let requested = [3000, 1, 40, 3000].map(|position| OperationId::new(position).unwrap());
    let visited = Cell::new(0);
    let matches = CallbackTerminalOperations::new(
        requested.into_iter(),
        operations
            .iter()
            .inspect(|_| visited.set(visited.get() + 1)),
    );
    assert_eq!(visited.get(), operations.len());
    assert_eq!(matches.requested.len(), 3);
    for id in requested {
        let expected = operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap();
        assert!(std::ptr::eq(matches.find(id).unwrap(), expected));
    }
    assert_eq!(matches.find(OperationId::new(5000).unwrap()), Err(0));
}

#[test]
fn terminal_join_counts_duplicates_and_retains_nonboundary_operations_for_rejection() {
    let operations = [
        operation(2),
        operation(1),
        operation(1),
        operation(4),
        operation(4),
    ];
    let matches = CallbackTerminalOperations::new(
        [1, 2, 3]
            .map(|position| OperationId::new(position).unwrap())
            .into_iter(),
        operations.iter(),
    );
    assert_eq!(matches.find(operations[1].id), Err(2));
    assert_eq!(matches.find(OperationId::new(3).unwrap()), Err(0));
    assert!(matches!(
        matches.find(operations[0].id).unwrap().kind,
        OperationKind::BooleanConstant { .. }
    ));
}

#[test]
fn empty_callback_roster_does_not_visit_terminal_operations() {
    let operations = [operation(1)];
    let matches = CallbackTerminalOperations::new(
        std::iter::empty(),
        operations.iter().inspect(|_| panic!("no callback demands")),
    );
    assert!(matches.requested.is_empty());
    assert!(
        CallbackSourceCalls::new(std::iter::empty(), &[])
            .requested
            .is_empty()
    );
}
