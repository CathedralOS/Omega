use super::semantic_caller_state;
use checked_trees::{FlowControlFacts, FlowStateFact};
use symbols::SymbolHandle;

fn original(
    control: &FlowControlFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Result<&FlowStateFact, &'static str> {
    let mut rows = control
        .states
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.machine_symbol == machine && row.state_symbol == state);
    let row = rows.next().ok_or("structural call has no semantic state")?;
    if rows.next().is_some() {
        return Err("structural call has ambiguous semantic states");
    }
    Ok(row)
}

#[test]
fn caller_state_selection_preserves_exact_rows_errors_and_replacement() {
    let machine = SymbolHandle::from_parts(7, 1);
    let next_machine = SymbolHandle::from_parts(7, 2);
    let state = SymbolHandle::from_parts(9, 1);
    let next_state = SymbolHandle::from_parts(9, 2);
    let check = |facts: &FlowControlFacts| {
        for (owner, symbol) in [
            (machine, state),
            (next_machine, state),
            (machine, next_state),
            (SymbolHandle::invalid(), SymbolHandle::invalid()),
        ] {
            assert_eq!(
                semantic_caller_state(facts, owner, symbol).map(std::ptr::from_ref),
                original(facts, owner, symbol).map(std::ptr::from_ref),
            );
        }
    };
    let row = |owner, symbol| FlowStateFact {
        machine_symbol: owner,
        state_symbol: symbol,
        ..Default::default()
    };
    let mut facts = FlowControlFacts::default();
    check(&facts);
    assert_eq!(
        semantic_caller_state(&facts, machine, state).unwrap_err(),
        "structural call has no semantic state"
    );
    let first = facts.states.insert(row(machine, state));
    facts.states.insert(row(next_machine, state));
    facts.states.insert(row(machine, next_state));
    facts
        .states
        .insert(row(SymbolHandle::invalid(), SymbolHandle::invalid()));
    check(&facts);
    assert!(std::ptr::eq(
        semantic_caller_state(&facts, machine, state).unwrap(),
        facts.states.get(first)
    ));
    facts.states.insert(row(machine, state));
    check(&facts);
    assert_eq!(
        semantic_caller_state(&facts, machine, state).unwrap_err(),
        "structural call has ambiguous semantic states"
    );
    let snapshot = facts.clone();
    facts.states.get_mut(first).machine_symbol = next_machine;
    check(&facts);
    check(&snapshot);

    // A whole mutable-arena replacement cannot inherit the previous grouping,
    // including a second independent arena with equal revision and row count.
    let mut old = arena::PagedArenaSet::default();
    old.insert(row(machine, state));
    *facts.states = old;
    check(&facts);
    let revision = facts.states.revision();
    let mut replacement = arena::PagedArenaSet::default();
    replacement.insert(row(next_machine, next_state));
    assert_eq!(replacement.revision(), revision);
    *facts.states = replacement;
    check(&facts);
    assert_eq!(
        semantic_caller_state(&facts, machine, state).unwrap_err(),
        "structural call has no semantic state"
    );
    assert!(semantic_caller_state(&facts, next_machine, next_state).is_ok());
}
