//! Compare the actual frozen declaration reader with its literal predecessor.
//! Raw malformed/duplicate rows test lookup only; they grant no checking custody.
use super::*;
use arena::{Arena, Handle, HandleSpan};
use checked_trees::{machine::Machine, state::State};
use typed_trees::TypedTrees;

fn rows<T: Default>(values: Vec<T>, generation: u32) -> Arena<T> {
    Arena::from_storage(T::default(), values, None, generation, true).unwrap()
}
fn span<T>(first: u32, count: u32, generation: u32) -> HandleSpan<T> {
    HandleSpan::from_parts(Handle::from_parts(first, generation), count)
}
fn fixture() -> TypedTrees {
    let state = |index, generation| State {
        symbol: SymbolHandle::from_parts(index, generation),
        ..Default::default()
    };
    let mut program = TypedTrees::default();
    program.tables.machine_states = rows(
        vec![
            state(3, 1),
            state(4, 1),
            state(3, 2),
            State::default(),
            state(4, 1),
        ],
        3,
    );
    program.tables.machines = rows(
        vec![
            Machine {
                symbol: SymbolHandle::from_parts(10, 1),
                states: span(1, 2, 3),
                ..Default::default()
            },
            Machine {
                symbol: SymbolHandle::invalid(),
                states: span(3, 2, 3),
                ..Default::default()
            },
            Machine {
                symbol: SymbolHandle::from_parts(10, 2),
                states: span(1, 1, 3),
                ..Default::default()
            },
        ],
        7,
    );
    program.roots.machines = span(1, 3, 7);
    program
}
fn compare(checked: &CheckedTrees) {
    for symbol in [
        SymbolHandle::from_parts(3, 1),
        SymbolHandle::from_parts(3, 2),
        SymbolHandle::from_parts(4, 1),
        SymbolHandle::from_parts(4, 2),
        SymbolHandle::from_parts(30, 1),
        SymbolHandle::invalid(),
    ] {
        match (
            state_declaration(checked, symbol),
            original_scan(checked, symbol),
        ) {
            (Ok((machine, state)), Ok((expected_machine, expected_state))) => {
                assert!(std::ptr::eq(machine, expected_machine));
                assert!(std::ptr::eq(state, expected_state));
            }
            (Err(actual), Err(expected)) => assert_eq!(actual, expected),
            other => panic!("different lookup outcome: {other:?}"),
        }
    }
}

#[test]
fn frozen_state_selection_matches_original_scan_after_equal_revision_replacements() {
    let program = fixture();
    let checked = CheckedTrees::with_roots(program.clone(), Default::default());
    compare(&checked);
    assert_eq!(
        state_declaration(&checked, SymbolHandle::from_parts(3, 1)).unwrap_err(),
        LoweringError::Unsupported("state symbol names more than one declared machine state")
    );
    assert_eq!(
        state_declaration(&checked, SymbolHandle::invalid()).unwrap_err(),
        LoweringError::Unsupported("state symbol names no declared machine state")
    );
    // The unspanned duplicate is not a declaration. Invalid machine symbols are
    // not a new rejection here, and same-index/different-generation states differ.
    assert_eq!(
        state_declaration(&checked, SymbolHandle::from_parts(4, 1))
            .unwrap()
            .0
            .symbol,
        SymbolHandle::from_parts(10, 1)
    );
    assert!(
        !state_declaration(&checked, SymbolHandle::from_parts(3, 2))
            .unwrap()
            .0
            .symbol
            .is_valid()
    );

    let mut warmed = program.clone();
    assert_eq!(
        warmed
            .machine_states_named(SymbolHandle::from_parts(4, 1))
            .count(),
        1
    );
    let old_revision = warmed.tables.machine_states.revision();
    let mut replacement = fixture().tables.machine_states;
    // Every storage row, including the unspanned one, changes its named state.
    for (_, state) in replacement.iter_mut() {
        state.symbol = SymbolHandle::from_parts(30, 1);
    }
    // Reconstruct storage to retain the original local revision deliberately.
    let replacement = rows(
        replacement.iter().map(|(_, state)| state.clone()).collect(),
        3,
    );
    assert_eq!(replacement.revision(), old_revision);
    warmed.tables.machine_states = replacement;
    // Also replace the machine table at equal revision/root identity, retaining
    // root membership but changing its state spans. This bypasses DerefMut.
    let machine_revision = warmed.tables.machines.revision();
    warmed.tables.machines = rows(
        vec![
            Machine {
                symbol: SymbolHandle::from_parts(11, 1),
                states: span(1, 1, 3),
                ..Default::default()
            },
            Machine::default(),
            Machine::default(),
        ],
        7,
    );
    assert_eq!(warmed.tables.machines.revision(), machine_revision);
    let rebuilt = CheckedTrees::with_roots(warmed, Default::default());
    compare(&rebuilt);
    assert!(state_declaration(&rebuilt, SymbolHandle::from_parts(4, 1)).is_err());
    assert_eq!(
        state_declaration(&rebuilt, SymbolHandle::from_parts(30, 1))
            .unwrap()
            .0
            .symbol,
        SymbolHandle::from_parts(11, 1)
    );
    compare(&checked); // retained frozen input remains unchanged.

    let mut wrong_root = program.clone();
    wrong_root.roots.machines = span(1, 3, 8);
    compare(&CheckedTrees::with_roots(wrong_root, Default::default()));
    let mut vacant_state = program.clone();
    assert!(
        vacant_state
            .tables
            .machine_states
            .free(Handle::from_parts(2, 3))
    );
    compare(&CheckedTrees::with_roots(vacant_state, Default::default()));
    let mut vacant_machine = program.clone();
    assert!(
        vacant_machine
            .tables
            .machines
            .free(Handle::from_parts(2, 7))
    );
    compare(&CheckedTrees::with_roots(
        vacant_machine,
        Default::default(),
    ));
    let mut wrong_states = program;
    wrong_states
        .tables
        .machines
        .get_mut(Handle::from_parts(1, 7))
        .states = span(1, 2, 4);
    compare(&CheckedTrees::with_roots(wrong_states, Default::default()));
}

fn original_scan(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
) -> Result<
    (
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    ),
    LoweringError,
> {
    let program = &checked.typed;
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |candidate| {
                (state.is_valid() && candidate.symbol == state).then_some((machine, candidate))
            })
    });
    let (machine, state) = states.next().ok_or(LoweringError::Unsupported(
        "state symbol names no declared machine state",
    ))?;
    if states.next().is_some() {
        return unsupported("state symbol names more than one declared machine state");
    }
    Ok((machine, state))
}
