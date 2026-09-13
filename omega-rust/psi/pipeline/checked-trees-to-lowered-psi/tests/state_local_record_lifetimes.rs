//! State-local records remain available to calls, then retire on the selected edge.

use terminal_interpreter::{
    TerminalEffect, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue,
};

const SOURCE: &str = r#"
    boundary trait Sink { machine record(value: u64); }
    data Region { value: u64; }
    data Filter { region: Region; }
    machine Filter::new(value: u64) -> Filter {
        Filter { region: Region { value: value } }
    }
    machine Region::get(&self) -> u64 { self.value }
    machine Filter::get(&self) -> u64 { self.region.get() }
    data Main {}
    machine Main::main(input: u64) reaches Sink {
        let local: Filter = Filter::new(input);
        let observed: u64 = local.get();
        transition observed == 7 {
            true -> selected(observed)
            false -> other(observed)
        }
        state selected(value: u64) {
            Sink::record(value ^ 1);
        }
        state other(value: u64) {
            Sink::record(value ^ 2);
        }
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

#[test]
fn local_nested_record_getter_composes_with_conditional_state_exit() {
    let checked = checked(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("state-local records are not required successor arguments");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    for input in [0, 7, u64::MAX] {
        assert_execution(
            &artifact,
            input,
            u128::from(input ^ if input == 7 { 1 } else { 2 }),
        );
    }
}

#[test]
fn mutable_call_result_records_keep_storage_through_state_local_receivers() {
    let source = r#"
        boundary trait Sink { machine record(value: u64); }
        data Region { value: u64; }
        data Filter { region: Region; }
        machine Filter::new(value: u64) -> Filter {
            Filter { region: Region { value: value } }
        }
        machine Region::get(&self) -> u64 { self.value }
        machine Region::set(&mut self, value: u64) { self.value = value; }
        machine Filter::get(&self) -> u64 { self.region.get() }
        machine Filter::set(&mut self, value: u64) { self.region.set(value); }
        data Main {}
        machine Main::main(input: u64) reaches Sink {
            let mut local: Filter = Filter::new(input);
            let before: u64 = local.get();
            local.set(before ^ 1);
            let observed: u64 = local.get();
            transition observed == 7 && before == 6 {
                true -> selected(observed)
                false -> other(observed)
            }
            state selected(value: u64) {
                let mut local: Filter = Filter::new(value);
                let before: u64 = local.get();
                local.set(before ^ 4);
                let after: u64 = local.get();
                Sink::record(after);
            }
            state other(value: u64) {
                let mut local: Filter = Filter::new(value);
                let before: u64 = local.get();
                local.set(before ^ 8);
                let after: u64 = local.get();
                Sink::record(after);
            }
        }
    "#;
    let checked = checked(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("mutable call results retain their exact state-local storage");
    let entry = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let initializer = checked.machine_states(entry)[0].statement_nodes.start();
    let mut immutable = checked.clone();
    let typed_trees::statement::StatementNode::LocalData(local) =
        immutable.typed.statement_table.statement_mut(initializer)
    else {
        panic!("mutable record initializer");
    };
    local.is_mutable = false;
    assert!(
        terminal_production::TerminalProductionRequest::new(&immutable, "Main::main")
            .produce_artifact()
            .is_err(),
        "a retained mutable receiver cannot borrow an immutable result destination"
    );
    let selected = checked
        .machine_states(entry)
        .iter()
        .find(|state| state.name.as_str() == "selected")
        .unwrap();
    let receipt = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.state_symbol == selected.symbol
                && event.kind == language_semantics::PermissionEventKind::AffineDrop
                && event.source == language_semantics::PermissionEventSource::StateExit)
                .then_some(handle)
        })
        .expect("selected successor has an exact local return-disposal receipt");
    let mut wrong_receipt = checked.clone();
    wrong_receipt
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(receipt)
        .source = language_semantics::PermissionEventSource::Statement { statement_index: 0 };
    assert!(
        terminal_production::TerminalProductionRequest::new(&wrong_receipt, "Main::main")
            .produce_artifact()
            .is_err(),
        "normal return cannot replace its local StateExit receipt with a statement receipt"
    );
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("encoded mutable record caller");
    for input in [0, 6, u64::MAX] {
        let observed = input ^ 1;
        assert_execution(
            &artifact,
            input,
            u128::from(observed ^ if observed == 7 { 4 } else { 8 }),
        );
    }
}

#[test]
fn copy_record_state_exit_needs_no_affine_receipt_or_disposal() {
    let source = SOURCE
        .replace("data Region {", "data Region [copy] {")
        .replace("data Filter {", "data Filter [copy] {");
    let artifact = artifact(&source);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::validate_module(&module).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    for block in &entry.blocks {
        match &block.terminator {
            terminal_psi::Terminator::Jump {
                trivial_affine_discards,
                ..
            } => assert!(trivial_affine_discards.is_empty()),
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                assert!(when_true.trivial_affine_discards.is_empty());
                assert!(when_false.trivial_affine_discards.is_empty());
            }
            _ => {}
        }
    }
    for input in [7, u64::MAX] {
        assert_execution(
            &artifact,
            input,
            u128::from(input ^ if input == 7 { 1 } else { 2 }),
        );
    }
}

fn artifact(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    terminal_production::TerminalProductionRequest::new(&checked(source), "Main::main")
        .produce_artifact()
        .expect("exact local lifetime publishes")
}

fn assert_execution(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    input: u64,
    expected: u128,
) {
    let argument = TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            64,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(input)),
    };
    for initial_allowance in [0, 2, 1000] {
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[argument],
        )
        .unwrap();
        let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(initial_allowance);
        let mut complete = false;
        for _ in 0..256 {
            match execution.resume(&mut meter).unwrap() {
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    let effects = execution.effects().to_vec();
                    assert!(matches!(
                        execution.resume(&mut meter).unwrap(),
                        TerminalExecutionStatus::SponsorExhausted(_)
                    ));
                    assert_eq!(
                        execution.effects(),
                        effects,
                        "retry without fuel cannot repeat an effect"
                    );
                    meter.replenish(1).unwrap();
                }
                other => panic!("unexpected execution {other:?}"),
            }
        }
        assert!(complete, "bounded fixture completes");
        let [TerminalEffect::BoundaryCall { arguments, .. }] = execution.effects() else {
            panic!("exactly one selected branch effect")
        };
        let [
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            },
        ] = arguments.as_slice()
        else {
            panic!("one unsigned getter observation")
        };
        assert_eq!(*value, expected);
    }
}

#[test]
fn local_record_direct_jump_and_asymmetric_transfer_preserve_observations() {
    let direct = SOURCE.replace("transition observed == 7 {\n            true -> selected(observed)\n            false -> other(observed)\n        }", "transition { _ -> selected(observed) }")
        .replace("state other(value: u64) {\n            Sink::record(value ^ 2);\n        }", "");
    for input in [7, u64::MAX] {
        assert_execution(&artifact(&direct), input, u128::from(input ^ 1));
    }
    let asymmetric = SOURCE
        .replace("true -> selected(observed)", "true -> selected(local)")
        .replace("state selected(value: u64) {\n            Sink::record(value ^ 1);", "state selected(value: Filter) {\n            let result: u64 = value.get();\n            Sink::record(result ^ 1);");
    let artifact = artifact(&asymmetric);
    for input in [7, u64::MAX] {
        assert_execution(
            &artifact,
            input,
            u128::from(input ^ if input == 7 { 1 } else { 2 }),
        );
    }
    let checked = checked(&asymmetric);
    let transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.kind == language_semantics::PermissionEventKind::Transfer
                && matches!(
                    event.provenance,
                    language_semantics::PermissionProvenance::Established { .. }
                )
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert_eq!(
        transfers.len(),
        1,
        "one local transfers on one authored successor"
    );
    for mutation in 0..2 {
        let mut changed = checked.clone();
        let event = changed
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(transfers[0]);
        if mutation == 0 {
            event.provenance = language_semantics::PermissionProvenance::Unknown;
        } else {
            event.source = language_semantics::PermissionEventSource::StateExit;
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "Main::main")
                .produce_artifact()
                .is_err(),
            "transfer authority mutation {mutation}"
        );
    }
}

#[test]
fn selected_edge_drops_two_locals_in_reverse_order_and_rejects_corruption() {
    let source = SOURCE.replace(
        "let observed: u64 = local.get();",
        "let later: Filter = Filter::new(input ^ 4);\n        let observed: u64 = local.get();",
    );
    let artifact = artifact(&source);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::validate_module(&module).unwrap();
    let entry = module
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    // Scalar successor operands may require selected staging blocks. The
    // disposal belongs to the final jump after those reads, not necessarily
    // to the conditional which chooses that block.
    let drops = module.machines[entry]
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(index, block)| match &block.terminator {
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } if when_true.trivial_affine_discards.len() == 2 => {
                assert_eq!(
                    when_true.trivial_affine_discards,
                    when_false.trivial_affine_discards
                );
                vec![
                    (index, Some(true), when_true.trivial_affine_discards.clone()),
                    (
                        index,
                        Some(false),
                        when_false.trivial_affine_discards.clone(),
                    ),
                ]
            }
            terminal_psi::Terminator::Jump {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.len() == 2 => {
                vec![(index, None, trivial_affine_discards.clone())]
            }
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(drops.len(), 2, "both selected routes dispose both locals");
    let (block, branch, expected) = &drops[0];
    assert_eq!(*expected, drops[1].2);
    let established = module.machines[entry]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.structural())
        .map(|result| result.place)
        .filter(|place| expected.contains(place))
        .collect::<Vec<_>>();
    assert_eq!(*expected, established.into_iter().rev().collect::<Vec<_>>());
    for mutation in 0..3 {
        let mut changed = module.clone();
        let discards = match &mut changed.machines[entry].blocks[*block].terminator {
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if *branch == Some(true) {
                    &mut when_true.trivial_affine_discards
                } else {
                    &mut when_false.trivial_affine_discards
                }
            }
            terminal_psi::Terminator::Jump {
                trivial_affine_discards,
                ..
            } => trivial_affine_discards,
            _ => panic!("selected disposal edge"),
        };
        match mutation {
            0 => {
                discards.pop();
            }
            1 => discards.reverse(),
            _ => discards.push(expected[0]),
        }
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "invalid selected-edge disposal {mutation}"
        );
    }
    assert_execution(&artifact, 7, 6);
}

#[test]
fn local_edge_cleanup_rejects_forged_permission_origins() {
    let checked = checked(SOURCE);
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main source machine")
        .symbol;
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && matches!(
                    event.kind,
                    language_semantics::PermissionEventKind::Establish
                        | language_semantics::PermissionEventKind::AffineDrop
                )
                && matches!(
                    event.provenance,
                    language_semantics::PermissionProvenance::Established { .. }
                )
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert!(!events.is_empty());
    for handle in events {
        let mut changed = checked.clone();
        changed
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(handle)
            .provenance = language_semantics::PermissionProvenance::Unknown;
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "Main::main")
                .produce_artifact()
                .is_err(),
            "forged local ownership provenance"
        );
    }
}

#[test]
fn one_selected_edge_cannot_transfer_the_same_affine_local_twice() {
    let source = SOURCE
        .replace("true -> selected(observed)", "true -> selected(local, local)")
        .replace("state selected(value: u64) {\n            Sink::record(value ^ 1);", "state selected(first: Filter, second: Filter) {\n            let observed: u64 = first.get();\n            Sink::record(observed);");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect_err("one selected edge cannot duplicate affine ownership");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("already")
                && (diagnostic.message.contains("consum")
                    || diagnostic.message.contains("transfer"))),
        "ownership diagnostic: {diagnostics:#?}"
    );
}
