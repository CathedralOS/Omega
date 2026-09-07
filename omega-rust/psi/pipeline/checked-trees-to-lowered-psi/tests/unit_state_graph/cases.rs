use super::*;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlTerminatorPlan,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::TerminalScalarValue;

fn output(checked: &checked_trees::CheckedTrees) -> Vec<(Vec<u8>, u128)> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Root::enter")
        .expect("valid source graph lowers before checking exact output");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("verified graph executes");
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
            let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
                panic!("one exact byte view");
            };
            let [
                TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(marker),
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned marker");
            };
            assert_eq!(
                *scalar_type,
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
            );
            (bytes.clone(), *marker)
        })
        .collect()
}

fn expected_output() -> Vec<(Vec<u8>, u128)> {
    [
        (b"\x80A".as_slice(), 7),
        (b"\x80A".as_slice(), 1),
        (b"\x80A".as_slice(), 7),
        (b"".as_slice(), 9),
        (b"".as_slice(), 2),
        (b"".as_slice(), 9),
        (b"last".as_slice(), 3),
    ]
    .into_iter()
    .map(|(bytes, marker)| (bytes.to_vec(), marker))
    .collect()
}

fn graph(checked: &mut checked_trees::CheckedTrees) -> &mut CheckedComposedUnitControlMachinePlan {
    let relay = checked
        .machines()
        .iter()
        .find(|machine| checked.typed.symbols.display_path(machine.symbol, "::") == "relay")
        .expect("authored relay machine")
        .symbol;
    let plans = &mut checked.facts.flow.terminal_unit_effects.composed_machines;
    let plan = plans
        .iter_mut()
        .find(|plan| plan.machine == relay)
        .expect("retained relay graph");
    assert!(plan.attachment_type_identity.is_none());
    plan
}

fn rejected(checked: &checked_trees::CheckedTrees, expected: &str) {
    let error = checked_trees_to_lowered_psi::lower_machine(checked, "Root::enter")
        .expect_err("tampered or unsupported graph must reject during lowering");
    assert!(
        matches!(
            &error,
            checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
        ),
        "{error:?}"
    );
    assert!(format!("{error:?}").contains(expected), "{error:?}");
}

#[test]
fn authored_state_declarations_need_not_follow_execution_order() {
    let first = SOURCE.find("        state first(").unwrap();
    let second = SOURCE.find("        state second(").unwrap();
    let finish = SOURCE.find("        state finish(").unwrap();
    let end = SOURCE.find("\n    }\n    data Root").unwrap();
    let declarations = [
        &SOURCE[first..second],
        &SOURCE[second..finish],
        &SOURCE[finish..end],
    ];
    for order in [[2, 1, 0], [1, 2, 0], [2, 0, 1]] {
        let source = format!(
            "{}{}\n{}\n{}{}",
            &SOURCE[..first],
            declarations[order[0]],
            declarations[order[1]],
            declarations[order[2]],
            &SOURCE[end..],
        );
        assert_eq!(output(&checked(&source)), expected_output(), "{order:?}");
    }
}

#[test]
fn empty_forwarding_state_preserves_joined_view_scalar_and_continuation() {
    let source = SOURCE
        .replace("_ -> finish(bytes, marker)", "_ -> forward(bytes, marker)")
        .replace(
            "        state finish(",
            "        state forward(bytes: &[u8], marker: u8) {
            transition { _ -> finish(bytes, marker) }
        }
        state finish(",
        );
    let mut checked = checked(&source);
    assert_eq!(graph(&mut checked).states.len(), 5);
    assert_eq!(output(&checked), expected_output());
}

#[test]
fn free_graph_retains_a_final_unit_call_expression() {
    let source = SOURCE.replace(
        "state finish(bytes: &[u8], marker: u8) {\n            Output::write(bytes, marker);",
        "state finish(bytes: &[u8], marker: u8) {\n            Output::write(bytes, marker)",
    );
    assert_ne!(source, SOURCE);
    assert_eq!(output(&checked(&source)), expected_output());
}

#[test]
fn view_and_scalar_successor_permutations_preserve_simultaneous_bindings() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
        machine relay(first: &[u8], second: &[u8], left: u8, right: u8) reaches Output {
            transition { _ -> swapped(second, first, right, left) }
            state swapped(first: &[u8], second: &[u8], left: u8, right: u8) {
                Output::write(first, left);
                Output::write(second, right);
                transition { _ -> restored(second, first, right, left) }
            }
            state restored(first: &[u8], second: &[u8], left: u8, right: u8) {
                Output::write(first, left);
                Output::write(second, right);
            }
        }
        data Root {}
        machine Root::enter() reaches Output { relay("first", "second", 1u8, 2u8); }
    "#;
    assert_eq!(
        output(&checked(source)),
        vec![
            (b"second".to_vec(), 2),
            (b"first".to_vec(), 1),
            (b"first".to_vec(), 1),
            (b"second".to_vec(), 2),
        ]
    );
}

#[test]
fn checked_state_roster_must_match_authored_states() {
    let base = checked(SOURCE);
    assert_eq!(output(&base), expected_output());
    for mutation in 0..3 {
        let mut changed = base.clone();
        let plan = graph(&mut changed);
        let expected = match mutation {
            0 => {
                plan.states.pop();
                "state roster drifted"
            }
            1 => {
                plan.states.swap(1, 2);
                "state identity, contract, or custody drifted"
            }
            2 => {
                plan.states[2] = plan.states[1].clone();
                "state identity, contract, or custody drifted"
            }
            _ => unreachable!(),
        };
        rejected(&changed, expected);
    }
}

#[test]
fn checked_body_calls_cannot_be_dropped_or_reordered() {
    let source = SOURCE.replace(
        "        Output::write(bytes, marker);\n        transition selected",
        "        Output::write(bytes, marker);\n        Output::write(bytes, 4u8);\n        transition selected",
    );
    assert_ne!(source, SOURCE);
    let base = checked(&source);
    let mut expected = expected_output();
    expected.insert(1, (b"\x80A".to_vec(), 4));
    expected.insert(5, (Vec::new(), 4));
    assert_eq!(output(&base), expected);
    for reorder in [false, true] {
        let mut changed = base.clone();
        let entry = &mut graph(&mut changed).states[0];
        assert_eq!(entry.operations.len(), 2);
        let expected = if reorder {
            entry.operations.swap(0, 1);
            "reordered a source effect"
        } else {
            entry.operations.remove(0);
            "dropped or added a body effect"
        };
        rejected(&changed, expected);
    }
}

fn source_with_alternative_inputs() -> String {
    SOURCE
        .replace(
            "relay(bytes: &[u8], selected: bool, marker: u8)",
            "relay(bytes: &[u8], other: &[u8], selected: bool, marker: u8, alternate: u8)",
        )
        .replace(
            r#"relay("\x80A", true, 7u8)"#,
            r#"relay("\x80A", "other", true, 7u8, 8u8)"#,
        )
        .replace(
            r#"relay("", false, 9u8)"#,
            r#"relay("", "different", false, 9u8, 10u8)"#,
        )
}

#[test]
fn checked_scalar_and_view_edges_reject_other_same_typed_source_parameters() {
    let base = checked(&source_with_alternative_inputs());
    assert_eq!(output(&base), expected_output());
    for change_view in [false, true] {
        let mut changed = base.clone();
        let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
            &mut graph(&mut changed).states[0].terminator
        else {
            panic!("entry conditional");
        };
        if change_view {
            assert_eq!(when_true.transfers[0].source_parameter_index, 0);
            when_true.transfers[0].source_parameter_index = 1;
        } else {
            assert_eq!(
                when_true.scalar_arguments[0].source,
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 1 }
            );
            when_true.scalar_arguments[0].source =
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 2 };
        }
        rejected(&changed, "not the retained parameter binding");
    }
}

#[test]
fn distinct_incoming_view_roots_require_descriptor_rebinding() {
    let source = source_with_alternative_inputs().replace(
        "false -> second(bytes, marker)",
        "false -> second(other, marker)",
    );
    let mut checked = checked(&source);
    assert_eq!(
        graph(&mut checked).states.len(),
        4,
        "source has a checked graph"
    );
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &graph(&mut checked).states[0].terminator
    else {
        panic!("entry conditional");
    };
    assert_eq!(when_true.transfers[0].source_parameter_index, 0);
    assert_eq!(when_false.transfers[0].source_parameter_index, 1);
    rejected(&checked, "descriptor rebinding");
}

#[test]
fn guard_source_binding_cannot_retarget_an_unchanged_checked_guard() {
    let mut checked = checked(SOURCE);
    assert_eq!(output(&checked), expected_output());
    let state = graph(&mut checked).states[0].state;
    let role = checked_trees::CheckedScalarExpressionRole::Guard;
    let handle = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find(|(_, binding)| binding.state == state && binding.role == role)
        .map(|(handle, _)| handle)
        .expect("relay guard retains authored source custody");
    let replacement = checked
        .typed
        .expression_table
        .insert(checked_trees::expression::ExpressionNode::Boolean(false));
    // The selected expression and composed guard still agree. Only their
    // source-binding handle now names an unrelated, live Boolean expression.
    let binding = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .get_mut(handle);
    assert_ne!(binding.expression, replacement);
    binding.expression = replacement;
    rejected(
        &checked,
        "pure scalar plan disagrees with its authored expression or destination",
    );
}

#[test]
fn dropped_graph_call_cannot_fall_back_to_legacy_three_state_admission() {
    let mut checked = checked(
        r#"
        boundary trait Output { machine write(marker: u8) reaches Output; }
        data Relay {}
        machine Relay::relay(selected: bool) reaches Output {
            transition selected { true -> first() false -> second() }
            state first() { Output::write(1u8); Output::write(2u8); }
            state second() { Output::write(3u8); }
        }
        data Root {}
        machine Root::enter() reaches Output { Relay::relay(true); }
    "#,
    );
    checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Relay::relay")
        .unwrap()
        .symbol;
    let graph = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| plan.machine == relay)
        .unwrap();
    assert_eq!(graph.states[1].operations.len(), 2);
    graph.states[1].operations.pop();
    rejected(&checked, "dropped or added a body effect");
}
