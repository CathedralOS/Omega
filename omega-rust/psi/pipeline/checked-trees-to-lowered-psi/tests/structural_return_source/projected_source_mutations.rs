//! Semantic ownership facts must rejoin retained plans, not merely agree in size.

use checked_trees::FlowClaimOutcomeSource;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};

#[test]
fn projected_returns_reject_changed_semantic_outcome_paths_and_rosters() {
    let checked = super::projected_claims::checked(2);
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
        .produce_artifact()
        .expect("valid projected customer publishes before mutation");
    let callee = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::forward")
        .unwrap();
    let state = checked.machine_states(callee)[0].symbol;
    let maps = &checked.facts.flow.ownership.claim_outcome_maps;
    let (map_handle, outcome) = maps
        .iter()
        .find(|(_, map)| map.machine_symbol == callee.symbol && map.state_symbol == state)
        .expect("exact closed forward outcome");
    let original = checked
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(outcome.entries)
        .to_vec();
    assert_eq!(original.len(), 2);
    for (index, entry) in original.iter().enumerate() {
        assert_eq!(
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(entry.output_segments),
            &[facts::PlaceSegment::FixedIndex { index }]
        );
        let FlowClaimOutcomeSource::Input { segments, .. } = entry.source else {
            panic!("closed forwarding continues an input")
        };
        assert_eq!(
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(segments),
            &[facts::PlaceSegment::FixedIndex { index }]
        );
    }
    for mutation in [
        "input_index",
        "output_index",
        "duplicate_outcome",
        "missing_outcome",
    ] {
        let mut invalid = checked.clone();
        let ownership = &mut invalid.facts.flow.ownership;
        let mut entries = original.clone();
        match mutation {
            "input_index" | "output_index" => {
                // Allocate a fresh path: input/output spans may share storage,
                // and this control must change only one side of the equation.
                let changed = ownership
                    .segments
                    .insert_many([facts::PlaceSegment::FixedIndex { index: 1 }]);
                if mutation == "input_index" {
                    let FlowClaimOutcomeSource::Input {
                        parameter_symbol, ..
                    } = entries[0].source
                    else {
                        unreachable!()
                    };
                    entries[0].source = FlowClaimOutcomeSource::Input {
                        parameter_symbol,
                        segments: changed,
                    };
                } else {
                    entries[0].output_segments = changed;
                }
            }
            "duplicate_outcome" => entries.push(entries[0].clone()),
            "missing_outcome" => {
                entries.pop();
            }
            _ => unreachable!(),
        }
        let entries = ownership.claim_outcome_entries.insert_many(entries);
        ownership.claim_outcome_maps.get_mut(map_handle).entries = entries;
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Main::demand")
                .produce_artifact()
                .is_err(),
            "semantic {mutation} must reject"
        );
    }
}

#[test]
fn projected_returns_reject_same_path_semantic_claim_identity_swaps() {
    let checked = super::projected_claims::checked(2);
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
        .produce_artifact()
        .expect("valid projected customer publishes before mutation");
    for entry in [true, false] {
        let name = if entry {
            "Main::forward"
        } else {
            "Main::demand"
        };
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let state = checked.machine_states(machine)[0].symbol;
        let events = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state
                    && event.access == PermissionAccess::Owned
                    && event.multiplicity == Multiplicity::Linear
                    && event.obligation_live
                    && if entry {
                        event.source == PermissionEventSource::StateEntry
                            && event.kind == PermissionEventKind::Establish
                    } else {
                        matches!(event.source, PermissionEventSource::Call { .. })
                            && event.kind == PermissionEventKind::Transfer
                    }
            })
            .map(|(handle, event)| (handle, event.clone()))
            .collect::<Vec<_>>();
        let [(first_handle, first), (second_handle, second)] = events.as_slice() else {
            panic!("the selected state has exactly two sibling claim events")
        };
        assert_eq!(first.root, second.root);
        assert_eq!(first.source, second.source);
        assert_ne!(first.claim_identity, second.claim_identity);
        assert_ne!(
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(first.segments),
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(second.segments)
        );
        let mut invalid = checked.clone();
        invalid
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(*first_handle)
            .claim_identity = second.claim_identity;
        invalid
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(*second_handle)
            .claim_identity = first.claim_identity;
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Main::demand")
                .produce_artifact()
                .is_err(),
            "same-path semantic identity swap entry={entry} must reject"
        );
    }
}
