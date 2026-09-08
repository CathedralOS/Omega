use super::*;

#[test]
fn deleting_only_the_last_assignment_cannot_skip_its_store_before_return() {
    let source = SOURCE.replace("\n    snapshot\n", "\n    slot\n");
    execute(
        &source,
        &[unsigned(7), unsigned(11)],
        Expectations::scalar(unsigned(7), 2),
    );
    let mut changed = publish_original(&source);
    let root = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap()
        .symbol;
    let graph = changed
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter_mut()
        .find(|graph| graph.machine == root)
        .expect("scalar root graph");
    assert_eq!(graph.states.len(), 1);
    let state = &mut graph.states[0];
    assert_eq!(state.bindings.len(), 4);
    assert_eq!(state.primitive_locals.len(), 1);
    assert_eq!(
        state.terminator,
        checked_trees::CheckedScalarStateTerminator::Return {
            statement_ordinal: 4,
        }
    );
    let removed = state.bindings.pop().expect("trailing local assignment");
    assert_eq!(removed.statement_ordinal, 3);
    assert_eq!(
        removed.destination,
        CheckedScalarBindingDestination::StorageAssign {
            symbol: state.primitive_locals[0].symbol,
        }
    );
    // Source, return coordinate, expression custody, and local establishment
    // remain intact. Omitting this store would return the second stamp's 11.
    reject(
        &changed,
        "deleted trailing assignment before the unchanged return",
    );
}

#[test]
fn primitive_local_roster_rejects_missing_duplicate_and_crossed_identities() {
    let original = publish_original(TWO_LOCALS);
    let root = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap()
        .symbol;
    for mutation in [
        "missing first local",
        "missing all locals",
        "duplicate local row",
        "swapped whole rows",
        "swapped symbols at fixed declaration ordinals",
    ] {
        let mut changed = original.clone();
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == root)
            .expect("scalar root graph");
        assert_eq!(graph.states.len(), 1);
        let locals = &mut graph.states[0].primitive_locals;
        assert_eq!(locals.len(), 2);
        assert_eq!(locals[0].statement_ordinal, 0);
        assert_eq!(locals[1].statement_ordinal, 1);
        assert_ne!(locals[0].symbol, locals[1].symbol);
        assert_eq!(locals[0].primitive_type, locals[1].primitive_type);
        assert_eq!(locals[0].type_identity, locals[1].type_identity);
        match mutation {
            "missing first local" => {
                locals.remove(0);
            }
            "missing all locals" => locals.clear(),
            "duplicate local row" => locals.push(locals[0].clone()),
            "swapped whole rows" => locals.swap(0, 1),
            "swapped symbols at fixed declaration ordinals" => {
                let first = locals[0].symbol;
                locals[0].symbol = locals[1].symbol;
                locals[1].symbol = first;
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}
