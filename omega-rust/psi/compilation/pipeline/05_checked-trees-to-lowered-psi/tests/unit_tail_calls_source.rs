//! A trailing Unit expression call executes normally before Unit cleanup.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact_measured,
};
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

#[path = "unit_tail_calls_source/statements.rs"]
mod statements;

fn artifact(
    checked: &checked_trees::CheckedTrees,
    semicolon: bool,
    locals: &[&str],
) -> (Vec<u8>, Vec<u8>) {
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let [state] = checked.typed.machine_states(root) else {
        panic!("one authored state");
    };
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    assert_eq!(
        statements
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) => Some(local.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        locals,
        "no synthetic source temporaries"
    );
    if semicolon {
        assert!(matches!(statements.last(), Some(StatementNode::Call(_))));
    } else {
        let Some(StatementNode::Expression(expression)) = statements.last() else {
            panic!("authored trailing expression remains an expression");
        };
        assert!(matches!(
            checked.typed.expression_table.expression(*expression),
            ExpressionNode::Call(_)
        ));
    }
    verified_artifact(checked)
}

fn verified_artifact(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("Unit expression call lowers");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    (semantic, evidence)
}

#[derive(Default)]
struct Observe(Vec<Vec<TerminalScalarValue>>);

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("observable boundary effect");
        };
        self.0.push(arguments.clone());
        Ok(())
    }
}

const IDENTITY: &str = r#"
    machine identity(value: bool) -> bool
    requires true == true
    ensures true == true
    { value }
"#;

fn execute(
    artifact: &(Vec<u8>, Vec<u8>),
    observer: &mut Observe,
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_measured(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        observer,
    )
    .map(|result| result.value())
}

fn pair_source(
    boundary: bool,
    qualified: bool,
    semicolon: bool,
    caller_unit: &str,
    callee_unit: &str,
    cleanup: bool,
) -> String {
    let punctuation = if semicolon { ";" } else { "" };
    let target = if boundary {
        "Sink::record"
    } else if qualified {
        "Relay::record"
    } else {
        "record"
    };
    let body = if boundary {
        String::new()
    } else {
        format!(
            r#"
        machine {target}(first: bool, second: bool) {callee_unit} reaches Sink {{
            Sink::record(first, second);
            Sink::record(second, first);
        }}
    "#
        )
    };
    let prefix = if cleanup {
        "let one: Empty = Empty {}; let two: Empty = Empty {}; Sink::record(false, false);"
    } else {
        ""
    };
    format!(
        r#"
        {IDENTITY}
        boundary trait Sink {{ machine record(first: bool, second: bool) {callee_unit} reaches Sink; }}
        data Relay {{}}
        data Empty {{}}
        {body}
        data Root {{}}
        machine Root::enter() {caller_unit} reaches Sink {{
            {prefix}
            {target}(identity(false), identity(identity(true))){punctuation}
        }}
    "#
    )
}

#[test]
fn trailing_and_semicolon_unit_calls_agree_for_explicit_and_omitted_unit_signatures() {
    for boundary in [false, true] {
        for qualified in [false, true] {
            if boundary && !qualified {
                continue;
            }
            for caller_unit in ["", "-> ()"] {
                for callee_unit in ["", "-> ()"] {
                    for semicolon in [false, true] {
                        let checked = crate::front_end::checked_program(&pair_source(
                            boundary,
                            qualified,
                            semicolon,
                            caller_unit,
                            callee_unit,
                            false,
                        ));
                        let artifact = artifact(&checked, semicolon, &[]);
                        let mut observer = Observe::default();
                        assert_eq!(
                            execute(&artifact, &mut observer).unwrap(),
                            TerminalExecutionResult::Unit
                        );
                        let mut expected = vec![vec![
                            TerminalScalarValue::Boolean(false),
                            TerminalScalarValue::Boolean(true),
                        ]];
                        if !boundary {
                            expected.push(vec![
                                TerminalScalarValue::Boolean(true),
                                TerminalScalarValue::Boolean(false),
                            ]);
                        }
                        assert_eq!(
                            observer.0, expected,
                            "each authored Unit body effect occurs exactly once"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn unit_callers_do_not_implicitly_discard_value_returning_trailing_calls() {
    for caller_unit in ["", "-> ()"] {
        for target in ["value", "Scalar::value"] {
            let source = format!(
                r#"
                data Scalar {{}}
                machine {target}() -> bool {{ true }}
                data Root {{}}
                machine Root::enter() {caller_unit} {{ {target}() }}
            "#
            );
            assert!(
                crate::front_end::checked_program_result(&source).is_err(),
                "Unit caller cannot silently discard the trailing bool result"
            );
        }
    }
}

#[test]
fn trailing_unit_call_source_and_occurrence_corruption_rejects() {
    for boundary in [false, true] {
        let checked =
            crate::front_end::checked_program(&pair_source(boundary, true, false, "", "", false));
        artifact(&checked, false, &[]);
        let root = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .unwrap();
        let state = &checked.typed.machine_states(root)[0];
        let StatementNode::Expression(expression) = checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[0]
        else {
            unreachable!();
        };
        let ExpressionNode::Call(call) = checked.typed.expression_table.expression(expression)
        else {
            unreachable!();
        };
        let arguments = checked
            .typed
            .expression_table
            .expression_handles(call.arguments);
        let (flow_handle, _) = checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .find(|(_, flow)| flow.authored_expression == expression && flow.call_ordinal == 0)
            .unwrap();
        for mutation in 0..6 {
            let mut changed = checked.clone();
            match mutation {
                0 => {
                    let ExpressionNode::Call(call) =
                        changed.typed.expression_table.expression_mut(expression)
                    else {
                        unreachable!();
                    };
                    call.target_symbol = symbols::SymbolHandle::invalid();
                }
                1 => changed
                    .typed
                    .expression_table
                    .set_expression_handle_at_offset(call.arguments, 0, arguments[1]),
                2 => {
                    changed
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(flow_handle)
                        .authored_expression = arena::Handle::invalid()
                }
                3 => {
                    changed
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(flow_handle)
                        .statement_index += 1
                }
                4 => {
                    let ExpressionNode::Call(call) =
                        changed.typed.expression_table.expression_mut(expression)
                    else {
                        unreachable!();
                    };
                    call.receiver = arguments[0];
                }
                5 => {
                    let StatementNode::Expression(expression) = &mut changed
                        .typed
                        .statement_table
                        .statements_mut(state.statement_nodes)[0]
                    else {
                        unreachable!();
                    };
                    *expression = arguments[0];
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("Root::enter")
                )
                .is_err(),
                "trailing call mutation={mutation}"
            );
        }
    }
}

#[test]
fn unit_tail_exemption_does_not_admit_unit_calls_in_scalar_value_positions() {
    for body in [
        "Sink::record(Sink::unit())",
        "let invalid: bool = Sink::unit(); Sink::record(invalid)",
    ] {
        let source = format!(
            r#"
            boundary trait Sink {{ machine unit() reaches Sink; machine record(value: bool) reaches Sink; }}
            data Root {{}}
            machine Root::enter() reaches Sink {{ {body} }}
        "#
        );
        let typed = crate::front_end::typed_program(&source);
        let root = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .unwrap();
        let state = &typed.machine_states(root)[0];
        let unit_call = match &typed.statement_table.statements(state.statement_nodes)[0] {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Expression(expression) => {
                let ExpressionNode::Call(call) = typed.expression_table.expression(*expression)
                else {
                    unreachable!()
                };
                typed.expression_table.expression_handles(call.arguments)[0]
            }
            _ => unreachable!(),
        };
        assert!(
            !validation::unit_return_call_is_supported(&typed, root, state, unit_call),
            "only the entire terminal expression receives the Unit exemption"
        );
        assert!(
            typed_trees_to_checked_trees::lower_typed_trees(
                typed,
                &typed_trees_to_checked_trees::CheckingRequest::settled()
            )
            .is_err(),
            "Unit cannot supply a scalar operand or local: {body}"
        );
    }
}
