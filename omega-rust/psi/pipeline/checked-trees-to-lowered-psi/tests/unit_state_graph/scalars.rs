use super::*;
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::TerminalScalarValue;

const PREFIX: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
    machine relay(bytes: &[u8], selected: bool, byte: u8) reaches Output {
        let mut output: u8 = byte;
        output = 42u8;
        let marker: i32 = output as i32;
        Output::write(bytes, marker);
        transition selected {
            true -> finish(bytes, output as i32)
            false -> finish(bytes, 10i32)
        }
        state finish(bytes: &[u8], marker: i32) {
            let mut output: i32 = marker;
            Output::write(bytes, output);
        }
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80A", true, 128u8);
        relay("", false, 255u8);
        Output::write("last", 3i32);
    }
"#;

fn effects(checked: &checked_trees::CheckedTrees) -> Vec<(Vec<u8>, i128)> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Root::enter")
        .expect("state-local values and selected successor operands lower");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("independent scalar graph execution");
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
                panic!("expected output effect");
            };
            let [
                TerminalScalarValue::Integer {
                    value: IntegerValue::Signed(marker),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one signed marker");
            };
            (byte_sequence_arguments[0].clone().unwrap(), *marker)
        })
        .collect()
}

#[test]
fn local_storage_and_immutable_values_survive_calls_and_selected_edges() {
    assert_eq!(
        effects(&checked(PREFIX)),
        vec![
            (b"\x80A".to_vec(), 42),
            (b"\x80A".to_vec(), 42),
            (Vec::new(), 42),
            (Vec::new(), 10),
            (b"last".to_vec(), 3),
        ]
    );
}

#[test]
fn only_the_selected_successor_evaluates_its_scalar_operands() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8], divisor: u8) reaches Output {
            transition divisor != 0u8 {
                true -> finish(bytes, (12u8 / divisor) as i32)
                false -> finish(bytes, 9i32)
            }
            state finish(bytes: &[u8], marker: i32) { Output::write(bytes, marker); }
        }
        data Root {}
        machine Root::enter() reaches Output {
            relay("empty-divisor", 0u8);
            relay("quotient", 3u8);
        }
    "#;
    assert_eq!(
        effects(&checked(source)),
        vec![(b"empty-divisor".to_vec(), 9), (b"quotient".to_vec(), 4),]
    );
}

#[test]
fn scalar_storage_survives_nested_operand_calls_and_drives_the_guard() {
    let source = PREFIX
        .replace(
            "let marker: i32 = output as i32;",
            "let mut ready: bool = selected; ready = !selected; let marker: i32 = output as i32;",
        )
        .replace("transition selected", "transition ready")
        .replace(
            "Output::write(bytes, marker);",
            "Output::write(bytes, identity(output as i32));",
        );
    let source = format!("machine identity(value: i32) -> i32 {{ value }}\n{source}");
    assert_eq!(
        effects(&checked(&source)),
        vec![
            (b"\x80A".to_vec(), 42),
            (b"\x80A".to_vec(), 10),
            (Vec::new(), 42),
            (Vec::new(), 42),
            (b"last".to_vec(), 3),
        ]
    );
}

#[test]
fn selected_head_read_uses_the_actual_view_length_and_skips_empty_bytes() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8]) reaches Output {
            transition bytes.len > 0 {
                true -> finish(bytes, bytes[0] as i32)
                false -> finish(bytes, 9i32)
            }
            state finish(bytes: &[u8], marker: i32) { Output::write(bytes, marker); }
        }
        data Root {}
        machine Root::enter() reaches Output {
            relay("\x80A");
            relay("");
        }
    "#;
    assert_eq!(
        effects(&checked(source)),
        vec![(b"\x80A".to_vec(), 128), (Vec::new(), 9),]
    );
    let unguarded = source.replace("bytes.len > 0", "bytes.len >= 0");
    let tokens = Lexer::new(&unguarded).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let error = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect_err("a selected edge alone does not prove the head is in bounds");
    assert!(
        format!("{error:?}").contains("cannot prove index"),
        "{error:?}"
    );
}

#[test]
fn scalar_prefix_cannot_drop_reorder_or_retarget_authored_writes() {
    let baseline = checked(PREFIX);
    for mutation in 0..3 {
        let mut changed = baseline.clone();
        let relay = changed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "relay")
            .unwrap()
            .symbol;
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == relay)
            .unwrap();
        let entry = &mut plan.states[0];
        assert_eq!(entry.bindings.len(), 3);
        match mutation {
            0 => {
                entry.bindings.clear();
                entry.binding_initializers.clear();
            }
            1 => {
                entry.bindings.swap(0, 1);
                entry.binding_initializers.swap(0, 1);
            }
            2 => {
                entry.bindings[1].destination =
                    checked_trees::CheckedScalarBindingDestination::Immutable;
            }
            _ => unreachable!(),
        }
        let error = checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter")
            .expect_err("authored scalar writes must survive lowering");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
            ),
            "{error:?}"
        );
    }
}

#[test]
fn expression_successors_rejoin_the_exact_selected_source_operand() {
    let mut changed = checked(PREFIX);
    let relay = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .unwrap()
        .symbol;
    let entry = &changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == relay)
        .unwrap()
        .states[0];
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { when_false, .. } =
        &entry.terminator
    else {
        panic!("conditional");
    };
    let state = entry.state;
    let ordinal = when_false.statement_ordinal;
    let role = checked_trees::CheckedScalarExpressionRole::TransitionArgument {
        argument_ordinal: 1,
    };
    let source_handle = changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find(|(_, binding)| {
            binding.state == state && binding.statement_ordinal == ordinal && binding.role == role
        })
        .unwrap()
        .0;
    let unrelated = changed
        .typed
        .expression_table
        .insert(checked_trees::expression::ExpressionNode::Boolean(false));
    changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .get_mut(source_handle)
        .expression = unrelated;
    let error = checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter")
        .expect_err("successor source identity must be checked");
    assert!(
        format!("{error:?}").contains("authored expression or destination"),
        "{error:?}"
    );
}
