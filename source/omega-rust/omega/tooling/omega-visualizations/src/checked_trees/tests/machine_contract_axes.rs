use super::*;

#[test]
fn machine_contract_manifest_exact_rows_preserve_axis_orthogonality() {
    let (mut program, machine) = machine_contract_exact_rows_fixture();
    let empty = psi_language_semantics::ServiceReachRowTable::EMPTY_ROW;
    program
        .facts
        .service_reaches
        .machines
        .for_each_mut(|_, fact| {
            if fact.machine == machine {
                fact.interface =
                    psi_language_semantics::ServiceReachInterface::PublishedCeiling(empty);
            }
        });
    program
        .facts
        .synchronous_invocations
        .machines
        .iter_mut()
        .find(|fact| fact.machine == machine)
        .expect("exact synchronous-invocation row")
        .plan = psi_language_semantics::SynchronousInvocationPlan {
        interface: psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling,
        published: Vec::new(),
        checked_inferred: vec!["service:Clock".to_owned()],
    };
    program
        .facts
        .suspensions
        .machines
        .iter_mut()
        .find(|fact| fact.machine == machine)
        .expect("exact suspension row")
        .plan = SuspensionPlan {
        interface: SuspensionInterface::PublishedMaySuspend(false),
        checked_may_suspend: true,
    };
    program
        .facts
        .blocking
        .machines
        .iter_mut()
        .find(|fact| fact.machine == machine)
        .expect("exact blocking row")
        .plan = BlockingPlan {
        interface: BlockingInterface::PublishedMayBlock(true),
        checked_may_block: false,
    };

    let json = machine_contract_manifest_json(&program);

    assert!(json.contains("\"fingerprint\": \"0x0000000000001234\""));
    assert!(
        json.contains(
            "\"service_reach\": {\"interface\": \"published_ceiling\", \"services\": []}"
        )
    );
    assert!(json.contains(
        "\"synchronous_invocation\": {\"interface\": \"published_ceiling\", \"targets\": []}"
    ));
    assert!(json.contains(
        "\"suspension\": {\"interface\": \"published_ceiling\", \"may_suspend\": false}"
    ));
    assert!(
        json.contains("\"blocking\": {\"interface\": \"published_ceiling\", \"may_block\": true}")
    );
    assert!(json.contains("\"checked_may_suspend\": true"));
    assert!(json.contains("\"checked_may_block\": false"));
    assert!(json.contains("\"checked_synchronous_invocations\": [\"service:Clock\"]"));
}

#[test]
fn machine_contract_manifest_exact_rows_reject_every_missing_axis() {
    for axis in ManifestExactAxis::ALL {
        let (mut program, machine) = machine_contract_exact_rows_fixture();
        remove_manifest_exact_row(&mut program, machine, axis);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            machine_contract_manifest_json(&program)
        }))
        .expect_err("missing exact manifest row must fail closed");
        let message = panic_message(panic);
        assert!(
            message.contains(&format!("missing its exact {} row", axis.label())),
            "unexpected {axis:?} missing-row diagnostic: {message}"
        );
    }
}

#[test]
fn machine_contract_manifest_exact_rows_reject_every_duplicate_axis() {
    for axis in ManifestExactAxis::ALL {
        let (mut program, machine) = machine_contract_exact_rows_fixture();
        duplicate_manifest_exact_row(&mut program, machine, machine, axis);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            machine_contract_manifest_json(&program)
        }))
        .expect_err("duplicate exact manifest row must fail closed");
        let message = panic_message(panic);
        assert!(
            message.contains(&format!("duplicate exact {} rows", axis.label())),
            "unexpected {axis:?} duplicate-row diagnostic: {message}"
        );
    }
}

#[test]
fn machine_contract_manifest_exact_rows_ignore_unrelated_duplicates() {
    let (mut program, machine) = machine_contract_exact_rows_fixture();
    let unrelated = SymbolHandle::from_arena_index(62);
    for axis in ManifestExactAxis::ALL {
        duplicate_manifest_exact_row(&mut program, machine, unrelated, axis);
        duplicate_manifest_exact_row(&mut program, machine, unrelated, axis);
    }

    let json = machine_contract_manifest_json(&program);

    assert!(json.contains("\"machine\": \"Exact::run\""));
    assert!(json.contains("\"fingerprint\": \"0x0000000000001234\""));
    assert_eq!(json.matches("\"machine\": \"Exact::run\"").count(), 1);
}

#[test]
fn machine_contract_manifest_reads_independent_mutation_facts() {
    let (mut program, machine_symbol, state_symbol, _) = mutation_state_owner_fixture();
    push_behavior_contract(&mut program, machine_symbol, false, false);
    push_behavior_contract(
        &mut program,
        SymbolHandle::from_arena_index(52),
        false,
        false,
    );

    let baseline = machine_contract_manifest_json(&program);
    assert!(baseline.contains(
        "\"inferred_write_frames\": [\n          {\"state\": \"entry\", \"completeness\": \"opaque\""
    ));
    assert!(baseline.contains("{\"state\": \"next\", \"completeness\": \"opaque\""));

    let retained_frame = program
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|fact| fact.machine == machine_symbol)
        .expect("exact mutation row")
        .state_write_frames
        .iter_mut()
        .find(|frame| frame.state == state_symbol)
        .expect("exact entry frame");
    retained_frame.frame = psi_facts::NormalizedWriteFrame::complete(vec!["self.value".to_owned()]);
    let retained_fingerprint = retained_frame.frame.compatibility_report_fingerprint();
    let with_mutation = machine_contract_manifest_json(&program);
    let baseline_contract_start = baseline.find("\"contract\"").expect("baseline contract");
    let baseline_implementation_start = baseline
        .find("\"implementation\"")
        .expect("baseline implementation");
    let contract_start = with_mutation.find("\"contract\"").expect("contract object");
    let implementation_start = with_mutation
        .find("\"implementation\"")
        .expect("implementation object");
    assert_eq!(
        &baseline[baseline_contract_start..baseline_implementation_start],
        &with_mutation[contract_start..implementation_start]
    );
    assert!(!with_mutation[contract_start..implementation_start].contains("inferred_write_frames"));
    assert!(with_mutation[implementation_start..].contains(
        "\"inferred_write_frames\": [\n          {\"state\": \"entry\", \"completeness\": \"complete\""
    ));
    assert!(with_mutation[implementation_start..].contains("\"paths\": [\"self.value\"]"));
    assert!(with_mutation[implementation_start..].contains(&format!(
        "\"fingerprint\": \"0x{retained_fingerprint:016x}\""
    )));
}

#[test]
#[should_panic(expected = "mutation frames must cover its exact typed state table one-for-one")]
fn machine_contract_manifest_mutation_frames_reject_missing_state() {
    let (mut program, owner, _, _) = mutation_state_owner_fixture();
    push_behavior_contract(&mut program, owner, false, false);
    push_behavior_contract(
        &mut program,
        SymbolHandle::from_arena_index(52),
        false,
        false,
    );
    program
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|fact| fact.machine == owner)
        .expect("exact mutation row")
        .state_write_frames
        .pop();

    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "mutation frames must cover its exact typed state table one-for-one")]
fn machine_contract_manifest_mutation_frames_reject_extra_state() {
    let (mut program, owner, state, _) = mutation_state_owner_fixture();
    push_behavior_contract(&mut program, owner, false, false);
    push_behavior_contract(
        &mut program,
        SymbolHandle::from_arena_index(52),
        false,
        false,
    );
    let duplicate = program
        .facts
        .mutation
        .machines
        .iter()
        .find(|fact| fact.machine == owner)
        .expect("exact mutation row")
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state)
        .expect("entry frame")
        .clone();
    program
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|fact| fact.machine == owner)
        .expect("exact mutation row")
        .state_write_frames
        .push(duplicate);

    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "mutation frames must retain exact typed state-table carrier order")]
fn machine_contract_manifest_mutation_frames_reject_duplicate_state() {
    let (mut program, owner, state, _) = mutation_state_owner_fixture();
    push_behavior_contract(&mut program, owner, false, false);
    push_behavior_contract(
        &mut program,
        SymbolHandle::from_arena_index(52),
        false,
        false,
    );
    program
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|fact| fact.machine == owner)
        .expect("exact mutation row")
        .state_write_frames[1]
        .state = state;

    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "mutation frames must retain exact typed state-table carrier order")]
fn machine_contract_manifest_mutation_frames_reject_cross_machine_state() {
    let (mut program, owner, _, other_state) = mutation_state_owner_fixture();
    push_behavior_contract(&mut program, owner, false, false);
    push_behavior_contract(
        &mut program,
        SymbolHandle::from_arena_index(52),
        false,
        false,
    );
    program
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|fact| fact.machine == owner)
        .expect("exact mutation row")
        .state_write_frames[0]
        .state = other_state;

    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "mutation frames must retain exact typed state-table carrier order")]
fn machine_contract_manifest_mutation_frames_reject_out_of_order_state() {
    let (mut program, owner, _, _) = mutation_state_owner_fixture();
    push_behavior_contract(&mut program, owner, false, false);
    push_behavior_contract(
        &mut program,
        SymbolHandle::from_arena_index(52),
        false,
        false,
    );
    program
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|fact| fact.machine == owner)
        .expect("exact mutation row")
        .state_write_frames
        .swap(0, 1);

    machine_contract_manifest_json(&program);
}

#[test]
fn machine_contract_manifest_distinguishes_published_empty_from_internal_empty_reach() {
    let public_symbol = SymbolHandle::from_arena_index(20);
    let private_symbol = SymbolHandle::from_arena_index(21);
    let mut program = CheckedTrees::default();
    for (machine_symbol, state_symbol, name) in [
        (
            public_symbol,
            SymbolHandle::from_arena_index(22),
            "Public::run",
        ),
        (
            private_symbol,
            SymbolHandle::from_arena_index(23),
            "Private::run",
        ),
    ] {
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated(name),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
        push_behavior_contract(&mut program, machine_symbol, false, false);
    }

    let empty = psi_language_semantics::ServiceReachRowTable::EMPTY_ROW;
    for (machine, interface) in [
        (
            public_symbol,
            psi_language_semantics::ServiceReachInterface::PublishedCeiling(empty),
        ),
        (
            private_symbol,
            psi_language_semantics::ServiceReachInterface::InternalInferred,
        ),
    ] {
        program
            .facts
            .service_reaches
            .machines
            .for_each_mut(|_, fact| {
                if fact.machine == machine {
                    fact.interface = interface;
                }
            });
    }

    let json = machine_contract_manifest_json(&program);
    let public_start = json
        .find("\"machine\": \"Public::run\"")
        .expect("public row");
    let private_start = json
        .find("\"machine\": \"Private::run\"")
        .expect("private row");
    assert!(
        json[public_start..private_start].contains(
            "\"service_reach\": {\"interface\": \"published_ceiling\", \"services\": []}"
        )
    );
    assert!(
        json[private_start..].contains("\"service_reach\": {\"interface\": \"internal_inferred\"}")
    );
}

pub(super) fn crash_source_coordinate_fixture()
-> (CheckedTrees, SymbolHandle, SymbolHandle, SymbolHandle) {
    let machine_symbol = SymbolHandle::from_arena_index(90);
    let state_symbol = SymbolHandle::from_arena_index(91);
    let other_state_symbol = SymbolHandle::from_arena_index(93);
    let mut program = CheckedTrees::default();
    for (machine, state, machine_name) in [
        (machine_symbol, state_symbol, "Crash::run"),
        (
            SymbolHandle::from_arena_index(92),
            other_state_symbol,
            "Other::run",
        ),
    ] {
        let mut definition = Machine {
            symbol: machine,
            name: Identifier::generated(machine_name),
            ..Default::default()
        };
        let mut state_definition = State {
            symbol: state,
            name: Identifier::generated("entry"),
            ..Default::default()
        };
        for _ in 0..3 {
            program
                .typed
                .statement_table
                .push_statement(&mut state_definition.statement_nodes, Default::default());
        }
        program
            .typed
            .push_machine_state(&mut definition, state_definition);
        program.typed.push_machine(definition);
    }
    let calls = program.facts.flow.control.calls.insert_many([FlowCallFact {
        statement_index: 2,
        call_ordinal: 1,
        target_symbol: state_symbol,
        ..Default::default()
    }]);
    program.facts.flow.control.states.insert(FlowStateFact {
        machine_symbol,
        state_symbol,
        calls,
        ..Default::default()
    });
    (program, machine_symbol, state_symbol, other_state_symbol)
}
