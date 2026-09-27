use super::{
    AdmissionProfile, SOURCE, TerminalEffect, TerminalExecutionResult, encode_module,
    encode_proof_section, interpret_terminal_artifact_measured,
};
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlTerminatorPlan,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::TerminalScalarValue;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};

fn output(checked: &checked_trees::CheckedTrees) -> Vec<(Vec<u8>, u128)> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("valid source graph lowers before checking exact output");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
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
    let error = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
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
fn checked_state_roster_must_match_authored_states() {
    let base = crate::front_end::checked_program(SOURCE);
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
    let base = crate::front_end::checked_program(&source_with_alternative_inputs());
    assert_eq!(output(&base), expected_output());
    for change_view in [false, true] {
        let mut changed = base.clone();
        let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
            &mut graph(&mut changed).states[0].terminator
        else {
            panic!("entry conditional");
        };
        if change_view {
            assert_eq!(
                when_true.transfers[0].source,
                checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
            );
            when_true.transfers[0].source =
                checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
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
fn guard_source_binding_cannot_retarget_an_unchanged_checked_guard() {
    let mut checked = crate::front_end::checked_program(SOURCE);
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
