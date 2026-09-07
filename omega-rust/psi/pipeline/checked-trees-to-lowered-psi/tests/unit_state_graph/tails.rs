use super::*;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
    machine relay(bytes: &[u8]) reaches Output {
        transition bytes.len > 0 {
            true -> tail(bytes[1..], bytes[0] as i32)
            false -> empty()
        }
        state tail(bytes: &[u8], head: i32) {
            Output::write(bytes, head);
            transition bytes.len > 0 {
                true -> final_tail(bytes[1..], bytes[0] as i32)
                false -> empty()
            }
        }
        state final_tail(bytes: &[u8], head: i32) { Output::write(bytes, head); }
        state empty() { }
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80AB");
        relay("");
        relay("Z");
        Output::write("last", 3i32);
    }
"#;

#[test]
fn selected_tail_views_preserve_bytes_head_values_and_caller_continuation() {
    assert_eq!(
        effects(SOURCE),
        vec![
            (b"AB".to_vec(), 128),
            (b"B".to_vec(), 65),
            (Vec::new(), 90),
            (b"last".to_vec(), 3),
        ]
    );
}

#[test]
fn repeated_tail_state_preserves_each_selected_head_and_caller_continuation() {
    assert_eq!(
        effects(&loop_source()),
        vec![
            (b"AB".to_vec(), 128),
            (b"B".to_vec(), 65),
            (Vec::new(), 66),
            (Vec::new(), 90),
            (b"last".to_vec(), 3),
        ]
    );
}

fn loop_source() -> String {
    SOURCE
        .replace(
            "true -> final_tail(bytes[1..], bytes[0] as i32)",
            "true -> tail(bytes[1..], bytes[0] as i32)",
        )
        .replace(
            "state final_tail(bytes: &[u8], head: i32) { Output::write(bytes, head); }",
            "",
        )
}

#[test]
fn cyclic_tail_operations_resume_once_per_iteration_from_serialized_proofs() {
    use terminal_fuel::TerminalFuelMeter;
    use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
    let lowered =
        checked_trees_to_lowered_psi::lower_machine(&checked(&loop_source()), "Root::enter")
            .unwrap();
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let profile = AdmissionProfile::default();
    let unlimited = interpret_terminal_artifact_measured(&semantic, &proof, &profile, &[]).unwrap();
    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &profile, &[]).unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(1);
    let mut completed = false;
    for _ in 0..=unlimited.usage().total_units() {
        let status = execution.resume(&mut meter).unwrap();
        assert!(unlimited.effects().starts_with(execution.effects()));
        match status {
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(value) => {
                assert_eq!(value, TerminalExecutionResult::Unit);
                completed = true;
                break;
            }
            TerminalExecutionStatus::Crashed(crash) => panic!("unexpected crash {crash:?}"),
        }
    }
    assert!(completed);
    assert_eq!(execution.effects(), unlimited.effects());
    assert_eq!(meter.usage().total_units(), unlimited.usage().total_units());
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.effects(), unlimited.effects());
    assert_eq!(meter.usage().total_units(), unlimited.usage().total_units());
}

#[test]
fn unguarded_cyclic_byte_operations_reject_at_source_checking() {
    let source = loop_source().replace("transition bytes.len > 0 {", "transition true {");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let errors =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect_err("bounds remain required");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cannot prove index"))
    );
}

#[test]
fn replacing_the_loop_guard_cannot_reuse_the_original_bounds_certificate() {
    use terminal_psi::OperationKind;
    let lowered =
        checked_trees_to_lowered_psi::lower_machine(&checked(&loop_source()), "Root::enter")
            .unwrap();
    let mut changed = lowered.semantic_module;
    let guard = changed
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::IntegerLessThan { .. }))
        .last()
        .expect("loop extent guard");
    guard.kind = OperationKind::BooleanConstant { value: true };
    terminal_verifier::validate_module(&changed).expect("the forged guard is still well-typed");
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence { .. })
    ));
}

#[test]
fn a_ranked_writer_cannot_silently_become_an_unranked_loop() {
    let source = loop_source().replace(
        "machine relay(bytes: &[u8]) reaches Output {",
        "machine relay(bytes: &[u8]) terminates by bytes -> Slice::Length; reaches Output {",
    );
    let error = checked_trees_to_lowered_psi::lower_machine(&checked(&source), "Root::enter")
        .expect_err("free ranking evidence must survive selection before lowering");
    assert!(
        matches!(
            error,
            checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "Unit-effect member has an invalid checked terminal selection"
            )
        ),
        "{error:?}"
    );
    let attached = source
        .replace("machine relay(", "data Writer {} machine Writer::relay(")
        .replace("        relay(\"", "        Writer::relay(\"");
    let error = checked_trees_to_lowered_psi::lower_machine(&checked(&attached), "Root::enter")
        .expect_err("attached ranking evidence must survive the shared graph route");
    assert!(
        matches!(
            error,
            checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "Unit graph ranking certificate is not retained"
            )
        ),
        "{error:?}"
    );
}

#[test]
fn reentered_entry_binds_the_current_view_without_changing_invocation_parameters() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8]) reaches Output {
            transition bytes.len > 0 {
                true -> emit(bytes[0] as i32, bytes[1..])
                false -> done()
            }
            state emit(head: i32, bytes: &[u8]) {
                Output::write(bytes, head);
                transition { _ -> relay(bytes) }
            }
            state done() { }
        }
        data Root {}
        machine Root::enter() reaches Output {
            relay("\x80AB"); relay(""); relay("Z"); Output::write("last", 3i32);
        }
    "#;
    assert_eq!(
        effects(source),
        vec![
            (b"AB".to_vec(), 128),
            (b"B".to_vec(), 65),
            (Vec::new(), 66),
            (Vec::new(), 90),
            (b"last".to_vec(), 3),
        ]
    );
}

fn effects(source: &str) -> Vec<(Vec<u8>, i128)> {
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("selected tail descriptors reach later states");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("tail bounds and descriptor custody independently verify and execute");
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall {
                byte_sequence_arguments,
                arguments,
                ..
            } = effect
            else {
                panic!("expected output boundary");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Signed(marker),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one signed marker")
            };
            (byte_sequence_arguments[0].clone().unwrap(), *marker)
        })
        .collect()
}

#[test]
fn head_before_tail_and_reverse_authored_state_order_preserve_execution() {
    let source = SOURCE
        .replace(
            "tail(bytes[1..], bytes[0] as i32)",
            "tail(bytes[0] as i32, bytes[1..])",
        )
        .replace(
            "tail(bytes: &[u8], head: i32)",
            "tail(head: i32, bytes: &[u8])",
        );
    let final_state = "state final_tail(head: i32, bytes: &[u8]) { Output::write(bytes, head); }";
    let reversed = source.replace(final_state, "").replace(
        "state tail(head:",
        &format!("{final_state}\nstate tail(head:"),
    );
    assert_eq!(effects(&source), effects(SOURCE));
    assert_eq!(effects(&reversed), effects(SOURCE));
    for (source, expected) in [
        (SOURCE, ["tail", "head"]),
        (source.as_str(), ["head", "tail"]),
        (reversed.as_str(), ["head", "tail"]),
    ] {
        let lowered =
            checked_trees_to_lowered_psi::lower_machine(&checked(source), "Root::enter").unwrap();
        let mut selected_edges = 0;
        for block in lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
        {
            let sequence = block
                .operations
                .iter()
                .filter_map(|operation| match operation.kind {
                    terminal_psi::OperationKind::ByteSequenceSubslice { .. } => Some("tail"),
                    terminal_psi::OperationKind::ByteSequenceRead { .. } => Some("head"),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !sequence.is_empty() {
                assert_eq!(sequence, expected);
                selected_edges += 1;
            }
        }
        assert_eq!(selected_edges, 2);
    }
}

#[test]
fn unconditional_windows_preserve_omitted_endpoints_and_empty_views() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8]) reaches Output {
            transition { _ -> copied(bytes[..]) }
            state copied(bytes: &[u8]) { transition { _ -> prefix(bytes[..bytes.len]) } }
            state prefix(bytes: &[u8]) {
                Output::write(bytes, 1i32);
                transition { _ -> empty(bytes[bytes.len..bytes.len]) }
            }
            state empty(bytes: &[u8]) { Output::write(bytes, 2i32); }
        }
        data Root {}
        machine Root::enter() reaches Output { relay("\x80AB"); relay(""); }
    "#;
    assert_eq!(
        effects(source),
        vec![
            (vec![128, 65, 66], 1),
            (Vec::new(), 2),
            (Vec::new(), 1),
            (Vec::new(), 2),
        ]
    );
}

#[test]
fn endpoint_bindings_cannot_move_between_state_edges() {
    let checked = checked(SOURCE);
    let role = checked_trees::CheckedScalarExpressionRole::TransitionSubsliceStart {
        argument_ordinal: 0,
    };
    let handles = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .filter(|(_, binding)| binding.role == role)
        .map(|(handle, binding)| (handle, binding.expression))
        .collect::<Vec<_>>();
    assert_eq!(handles.len(), 2);
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .get_mut(handles[0].0)
        .expression = handles[1].1;
    checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter")
        .expect_err("equal endpoint values from different source edges are not interchangeable");
}

#[test]
fn unguarded_tail_cannot_gain_a_bounds_proof_from_edge_selection() {
    let source = SOURCE.replace("bytes.len > 0", "bytes.len >= 0");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect_err("selected edges still need actual tail bounds");
    assert!(format!("{diagnostics:?}").contains("cannot prove subslice range"));
}

#[test]
fn tail_transfer_cannot_be_replaced_with_an_unchanged_parameter() {
    let mut checked = checked(SOURCE);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| plan.states.len() == 4)
        .expect("relay graph");
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut plan.states[0].terminator
    else {
        panic!("guarded entry");
    };
    when_true.transfers[0].source =
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 };
    checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect_err("source subslice cannot be replaced by the original whole view");
}

#[test]
fn different_descriptors_at_a_join_preserve_the_selected_view() {
    let source = SOURCE
        .replace("false -> empty()", "false -> empty(bytes)")
        .replace(
            "state empty() { }",
            "state empty(bytes: &[u8]) { Output::write(bytes, 4i32); }",
        );
    assert_eq!(
        effects(&source),
        vec![
            (b"AB".to_vec(), 128),
            (b"B".to_vec(), 65),
            (Vec::new(), 4),
            (Vec::new(), 90),
            (Vec::new(), 4),
            (b"last".to_vec(), 3),
        ],
    );
}

#[test]
fn one_derived_descriptor_can_cross_both_sides_of_a_join() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8], selected: bool) reaches Output {
            transition { _ -> fork(bytes[..], selected) }
            state fork(bytes: &[u8], selected: bool) {
                transition selected { true -> left(bytes) false -> right(bytes) }
            }
            state left(bytes: &[u8]) { transition { _ -> joined(bytes) } }
            state right(bytes: &[u8]) { transition { _ -> joined(bytes) } }
            state joined(bytes: &[u8]) { Output::write(bytes, 1i32); }
        }
        data Root {}
        machine Root::enter() reaches Output { relay("\x80AB", true); relay("", false); }
    "#;
    assert_eq!(
        effects(source),
        vec![(vec![128, 65, 66], 1), (Vec::new(), 1)]
    );
}

#[test]
fn scalar_length_arguments_preserve_the_guards_exact_extent_observation() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8]) reaches Output {
            transition bytes.len > 0 {
                true -> tail(bytes.len, bytes[1..])
                false -> empty()
            }
            state tail(length: u64, bytes: &[u8]) { Output::write(bytes, 1i32); }
            state empty() {}
        }
        data Root {}
        machine Root::enter() reaches Output { relay("\x80AB"); relay(""); relay("Z"); }
    "#;
    for argument in ["bytes.len", "bytes.len + 0u64"] {
        let source = source.replace("tail(bytes.len,", &format!("tail({argument},"));
        assert_eq!(effects(&source), vec![(b"AB".to_vec(), 1), (Vec::new(), 1)]);
    }
}
