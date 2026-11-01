use super::OutgoingRows;
use checked_trees::{FlowControlFacts, FlowStateFact, FlowStateOutgoingFact};
use symbols::SymbolHandle;

fn compare(rows: &OutgoingRows<'_>, control: &FlowControlFacts, state: SymbolHandle) {
    let expected: Vec<_> = control
        .outgoing
        .iter()
        .filter(|(_, row)| row.state_symbol == state)
        .collect();
    let actual = rows.for_state(state);
    assert_eq!(actual.len(), expected.len());
    for ((actual_handle, actual_row), (expected_handle, expected_row)) in
        actual.iter().zip(&expected)
    {
        assert_eq!(actual_handle, expected_handle);
        assert!(std::ptr::eq(*actual_row, *expected_row));
    }
}

#[test]
fn outgoing_candidates_match_whole_arena_filter_with_exact_keys_and_order() {
    let machine = SymbolHandle::from_parts(11, 2);
    let other_machine = SymbolHandle::from_parts(11, 3);
    let state = SymbolHandle::from_parts(12, 4);
    let other_generation = SymbolHandle::from_parts(12, 5);
    let orphan = SymbolHandle::from_parts(13, 6);
    let missing = SymbolHandle::from_parts(14, 7);
    let mut control = FlowControlFacts::default();
    // This owner's empty span intentionally excludes every raw outgoing row.
    // The old whole-arena filter never required an owner or span membership.
    control.states.insert(FlowStateFact {
        machine_symbol: machine,
        state_symbol: state,
        ..Default::default()
    });
    let base = FlowStateOutgoingFact {
        machine_symbol: machine,
        state_symbol: state,
        ..Default::default()
    };
    for row in [
        base.clone(),
        FlowStateOutgoingFact {
            state_symbol: other_generation,
            ..base.clone()
        },
        FlowStateOutgoingFact {
            machine_symbol: other_machine,
            ..base.clone()
        },
        FlowStateOutgoingFact {
            state_symbol: orphan,
            ..base.clone()
        },
        base.clone(),
        FlowStateOutgoingFact {
            transition_target: arena::Handle::from_parts(21, 8),
            ..base.clone()
        },
        FlowStateOutgoingFact {
            statement_index: usize::MAX,
            ..base.clone()
        },
        FlowStateOutgoingFact {
            state_symbol: SymbolHandle::invalid(),
            ..base.clone()
        },
    ] {
        control.outgoing.insert(row);
    }
    assert!(control.state_by_symbol(orphan).is_none());
    {
        let rows = OutgoingRows::new(&control.outgoing);
        for _ in 0..3 {
            for selected in [
                state,
                other_generation,
                orphan,
                missing,
                SymbolHandle::invalid(),
            ] {
                compare(&rows, &control, selected);
            }
        }
    }
    // A later immutable request must read its own complete arena, including
    // newly appended duplicate rows, rather than an earlier request's roster.
    control.outgoing.insert(base);
    let rows = OutgoingRows::new(&control.outgoing);
    compare(&rows, &control, state);
    // An unrelated same-sized arena has a separate reader and never reuses
    // another arena's revision-derived grouping.
    let mut other = FlowControlFacts::default();
    for (_, row) in control.outgoing.iter() {
        other.outgoing.insert(FlowStateOutgoingFact {
            state_symbol: missing,
            ..row.clone()
        });
    }
    let other_rows = OutgoingRows::new(&other.outgoing);
    compare(&other_rows, &other, state);
    compare(&other_rows, &other, missing);
    compare(&rows, &control, missing);
}
