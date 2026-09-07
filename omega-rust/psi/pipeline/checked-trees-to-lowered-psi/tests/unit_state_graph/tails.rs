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
fn different_descriptors_at_a_join_remain_explicitly_unsupported() {
    let source = SOURCE
        .replace("false -> empty()", "false -> empty(bytes)")
        .replace("state empty()", "state empty(bytes: &[u8])");
    let checked = checked(&source);
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect_err("different incoming descriptors require structural block bindings");
    assert!(
        format!("{error:?}").contains("structural descriptor rebinding"),
        "{error:?}"
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
