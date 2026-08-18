use super::*;

#[test]
fn checked_behavior_summaries_keep_operational_axes_independent() {
    let suspending_machine = SymbolHandle::from_arena_index(1);
    let blocking_machine = SymbolHandle::from_arena_index(2);
    let unknown_machine = SymbolHandle::from_arena_index(3);
    let mut program = CheckedTrees::default();

    push_behavior_contract(&mut program, suspending_machine, true, false);
    push_behavior_flow_state(
        &mut program,
        suspending_machine,
        SymbolHandle::from_arena_index(11),
        SuspensionSummary {
            direct_may_suspend: true,
            transitive_may_suspend: false,
        },
        BlockingSummary::default(),
    );
    push_behavior_contract(&mut program, blocking_machine, false, true);
    push_behavior_flow_state(
        &mut program,
        blocking_machine,
        SymbolHandle::from_arena_index(12),
        SuspensionSummary::default(),
        BlockingSummary {
            direct_may_block: true,
            transitive_may_block: false,
        },
    );

    assert_eq!(
        machine_suspension_summary(&program, suspending_machine),
        SuspensionSummary {
            direct_may_suspend: true,
            transitive_may_suspend: true,
        }
    );
    assert_eq!(
        machine_blocking_summary(&program, suspending_machine),
        BlockingSummary::default()
    );
    assert_eq!(
        machine_suspension_summary(&program, blocking_machine),
        SuspensionSummary::default()
    );
    assert_eq!(
        machine_blocking_summary(&program, blocking_machine),
        BlockingSummary {
            direct_may_block: true,
            transitive_may_block: true,
        }
    );
    assert_eq!(
        machine_suspension_summary(&program, unknown_machine),
        SuspensionSummary::default()
    );
    assert_eq!(
        machine_blocking_summary(&program, unknown_machine),
        BlockingSummary::default()
    );
}

pub(super) fn claim_outcome_validation_fixture() -> (
    CheckedTrees,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
) {
    let machine_symbol = SymbolHandle::from_arena_index(100);
    let state_symbol = SymbolHandle::from_arena_index(101);
    let parameter_symbol = SymbolHandle::from_arena_index(102);
    let other_machine_symbol = SymbolHandle::from_arena_index(103);
    let other_state_symbol = SymbolHandle::from_arena_index(104);
    let mut program = CheckedTrees::default();
    for (machine, state, machine_name, state_name) in [
        (machine_symbol, state_symbol, "Claims::map", "map"),
        (
            other_machine_symbol,
            other_state_symbol,
            "OtherClaims::map",
            "map",
        ),
    ] {
        let mut machine_definition = Machine {
            symbol: machine,
            name: Identifier::generated(machine_name),
            ..Default::default()
        };
        let mut state_definition = State {
            symbol: state,
            name: Identifier::generated(state_name),
            ..Default::default()
        };
        if machine == machine_symbol {
            program.typed.push_state_parameter(
                &mut state_definition,
                StateParameter {
                    symbol: parameter_symbol,
                    name: Identifier::generated("resource"),
                    ..Default::default()
                },
            );
            for _ in 0..5 {
                program
                    .typed
                    .statement_table
                    .push_statement(&mut state_definition.statement_nodes, Default::default());
            }
        }
        program
            .typed
            .push_machine_state(&mut machine_definition, state_definition);
        program.typed.push_machine(machine_definition);
    }
    let input_identity = PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::StateEntry,
        ordinal: 0,
    };
    let input_provenance = PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::StateEntry,
    };
    program
        .facts
        .flow
        .ownership
        .permissions
        .insert(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::StateEntry,
            kind: PermissionEventKind::Establish,
            access: PermissionAccess::Owned,
            claim_identity: input_identity,
            provenance: input_provenance,
            root: PlaceRoot::Symbol(parameter_symbol),
            obligation_live: true,
            ..Default::default()
        });
    let established_identity = PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Statement { statement_index: 0 },
        ordinal: 1,
    };
    let established_provenance = PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Statement { statement_index: 0 },
    };
    program
        .facts
        .flow
        .ownership
        .permissions
        .insert(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index: 0 },
            kind: PermissionEventKind::Transfer,
            access: PermissionAccess::Owned,
            claim_identity: established_identity,
            provenance: established_provenance,
            root: PlaceRoot::Symbol(state_symbol),
            obligation_live: true,
            ..Default::default()
        });
    let established_output = program
        .facts
        .flow
        .ownership
        .segments
        .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 1 }]);
    let entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .insert_many([
            FlowClaimOutcomeEntryFact {
                output_segments: Default::default(),
                source: FlowClaimOutcomeSource::Input {
                    parameter_symbol,
                    segments: Default::default(),
                },
            },
            FlowClaimOutcomeEntryFact {
                output_segments: established_output,
                source: FlowClaimOutcomeSource::Established {
                    claim_identity: established_identity,
                    provenance: established_provenance,
                },
            },
        ]);
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .insert(FlowClaimOutcomeMapFact {
            machine_symbol,
            state_symbol,
            entries,
        });
    (
        program,
        machine_symbol,
        state_symbol,
        parameter_symbol,
        other_machine_symbol,
        other_state_symbol,
    )
}

pub(super) fn first_claim_outcome_entries_mut(
    program: &mut CheckedTrees,
) -> &mut [FlowClaimOutcomeEntryFact] {
    let entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .next()
        .expect("fixture map")
        .1
        .entries;
    program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_mut(entries)
        .expect("fixture entries")
}

#[test]
fn claim_outcome_manifest_accepts_exact_sources_and_explicit_empty_map() {
    let (mut program, _, _, _, other_machine, other_state) = claim_outcome_validation_fixture();
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .insert(FlowClaimOutcomeMapFact {
            machine_symbol: other_machine,
            state_symbol: other_state,
            entries: Default::default(),
        });

    let json = claim_outcome_manifest_json(&program);
    assert!(json.contains("\"kind\": \"input\""));
    assert!(json.contains("\"kind\": \"established\""));
    assert_eq!(json.matches("\"entries\": [").count(), 2);
    assert!(json.contains("\"entries\": [\n      ]"));
}

#[test]
#[should_panic(expected = "state must belong to its exact typed machine")]
fn claim_outcome_manifest_rejects_cross_machine_state() {
    let (mut program, _, _, _, _, other_state) = claim_outcome_validation_fixture();
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .for_each_mut(|_, map| map.state_symbol = other_state);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "one row per exact machine and state")]
fn claim_outcome_manifest_rejects_duplicate_map_coordinate() {
    let (mut program, ..) = claim_outcome_validation_fixture();
    let duplicate = program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .next()
        .expect("fixture map")
        .1
        .clone();
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .insert(duplicate);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "exact valid entry span")]
fn claim_outcome_manifest_rejects_invalid_entry_span() {
    let (mut program, ..) = claim_outcome_validation_fixture();
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .for_each_mut(|_, map| {
            map.entries =
                psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
        });
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "exact valid output path span")]
fn claim_outcome_manifest_rejects_invalid_output_path_span() {
    let (mut program, ..) = claim_outcome_validation_fixture();
    first_claim_outcome_entries_mut(&mut program)[0].output_segments =
        psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "one entry per exact output path")]
fn claim_outcome_manifest_rejects_duplicate_output_path() {
    let (mut program, ..) = claim_outcome_validation_fixture();
    let entries = first_claim_outcome_entries_mut(&mut program);
    entries[1].output_segments = entries[0].output_segments;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain an exact known source")]
fn claim_outcome_manifest_rejects_unknown_source() {
    let (mut program, ..) = claim_outcome_validation_fixture();
    first_claim_outcome_entries_mut(&mut program)[0].source = FlowClaimOutcomeSource::Unknown;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "exact parameter owned by its state")]
fn claim_outcome_manifest_rejects_missing_input_parameter() {
    let (mut program, ..) = claim_outcome_validation_fixture();
    first_claim_outcome_entries_mut(&mut program)[0].source = FlowClaimOutcomeSource::Input {
        parameter_symbol: SymbolHandle::from_arena_index(999),
        segments: Default::default(),
    };
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "input source must retain an exact valid path span")]
fn claim_outcome_manifest_rejects_invalid_input_path_span() {
    let (mut program, _, _, parameter, ..) = claim_outcome_validation_fixture();
    first_claim_outcome_entries_mut(&mut program)[0].source = FlowClaimOutcomeSource::Input {
        parameter_symbol: parameter,
        segments: psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1),
    };
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "one distinct live retained permission origin")]
fn claim_outcome_manifest_rejects_absent_input_origin() {
    let (mut program, ..) = claim_outcome_validation_fixture();
    program.facts.flow.ownership.permissions = Default::default();
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "one distinct live retained permission origin")]
fn claim_outcome_manifest_rejects_ambiguous_input_origin() {
    let (mut program, machine, state, parameter, ..) = claim_outcome_validation_fixture();
    program
        .facts
        .flow
        .ownership
        .permissions
        .insert(FlowPermissionEventFact {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::StateEntry,
            kind: PermissionEventKind::Establish,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: PermissionEventSource::StateEntry,
                ordinal: 9,
            },
            provenance: PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: PermissionEventSource::StateEntry,
            },
            root: PlaceRoot::Symbol(parameter),
            obligation_live: true,
            ..Default::default()
        });
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "non-unknown claim identity")]
fn claim_outcome_manifest_rejects_unknown_established_identity() {
    let (mut program, machine, state, ..) = claim_outcome_validation_fixture();
    first_claim_outcome_entries_mut(&mut program)[1].source = FlowClaimOutcomeSource::Established {
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance: PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement { statement_index: 0 },
        },
    };
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain non-unknown provenance")]
fn claim_outcome_manifest_rejects_unknown_established_provenance() {
    let (mut program, machine, state, ..) = claim_outcome_validation_fixture();
    first_claim_outcome_entries_mut(&mut program)[1].source = FlowClaimOutcomeSource::Established {
        claim_identity: PermissionClaimIdentity::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement { statement_index: 0 },
            ordinal: 1,
        },
        provenance: PermissionProvenance::Unknown,
    };
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must match one retained permission event")]
fn claim_outcome_manifest_rejects_detached_established_pair() {
    let (mut program, machine, state, ..) = claim_outcome_validation_fixture();
    first_claim_outcome_entries_mut(&mut program)[1].source = FlowClaimOutcomeSource::Established {
        claim_identity: PermissionClaimIdentity::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement { statement_index: 0 },
            ordinal: 99,
        },
        provenance: PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement { statement_index: 0 },
        },
    };
    claim_outcome_manifest_json(&program);
}
