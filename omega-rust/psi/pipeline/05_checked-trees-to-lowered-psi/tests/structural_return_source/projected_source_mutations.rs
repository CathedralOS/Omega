//! Semantic ownership facts must rejoin retained plans, not merely agree in size.

use checked_trees::FlowClaimOutcomeSource;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[test]
fn projected_returns_reject_changed_semantic_outcome_paths_and_rosters() {
    let checked = super::projected_claims::checked(2);
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::demand"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("valid projected customer publishes before mutation")
    .into_artifact();
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
            terminal_production::TerminalProductionRequest::new(
                &invalid,
                TerminalMachineSelection::Name("Main::demand")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "semantic {mutation} must reject"
        );
    }
}
