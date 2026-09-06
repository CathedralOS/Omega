//! A trailing Unit expression call executes normally before Unit cleanup.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecutionResult, TerminalInterpretError, TerminalScalarValue,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

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
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Root::enter")
        .expect("Unit expression call lowers");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
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

#[test]
fn trailing_boundary_unit_call_executes_computed_operand_without_semicolon() {
    let checked = checked(
        r#"
        machine identity(value: bool) -> bool { value }
        pub data Sink {}
        boundary machine Sink::record(value: bool);
        data Root {}
        machine Root::enter() { Sink::record(identity(true)) }
    "#,
    );
    let artifact = artifact(&checked, false, &[]);
    let mut observer = Observe::default();
    let result = interpret_terminal_artifact_with_effect_handler_measured(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        &[],
        &mut observer,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    assert_eq!(observer.0, vec![vec![TerminalScalarValue::Boolean(true)]]);
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
    interpret_terminal_artifact_with_effect_handler_measured(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        &[],
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
        machine {target}(first: bool, second: bool) {callee_unit} {{
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
        pub data Sink {{}}
        boundary machine Sink::record(first: bool, second: bool) {callee_unit};
        data Relay {{}}
        data Empty {{}}
        {body}
        data Root {{}}
        machine Root::enter() {caller_unit} {{
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
                        let checked = checked(&pair_source(
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
fn trailing_self_unit_call_uses_existing_receiver_custody() {
    for semicolon in [false, true] {
        let punctuation = if semicolon { ";" } else { "" };
        let source = format!(
            r#"
            {IDENTITY}
            pub data Sink {{}}
            boundary machine Sink::record(value: bool);
            data Root {{}}
            machine Root::record(&mut self, value: bool) {{ Sink::record(value); }}
            machine Root::enter(&mut self) {{ self.record(identity(true)){punctuation} }}
        "#
        );
        let checked = checked(&source);
        let artifact = artifact(&checked, semicolon, &[]);
        let mut observer = Observe::default();
        assert_eq!(
            execute(&artifact, &mut observer).unwrap(),
            TerminalExecutionResult::Unit
        );
        assert_eq!(observer.0, vec![vec![TerminalScalarValue::Boolean(true)]]);
    }
}

#[test]
fn trailing_unit_call_preserves_prior_effects_and_normal_reverse_local_cleanup() {
    for semicolon in [false, true] {
        let checked = checked(&pair_source(false, true, semicolon, "", "", true));
        let artifact = artifact(&checked, semicolon, &["one", "two"]);
        let module = decode_module(&artifact.0).unwrap();
        let root = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let established = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                if let terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination } =
                    operation.kind
                {
                    Some(destination)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(established.len(), 2);
        let returns = root
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                terminal_psi::Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => Some(trivial_affine_discards),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(returns.len(), 1);
        assert_eq!(returns[0], &[established[1], established[0]]);
        let mut observer = Observe::default();
        assert_eq!(
            execute(&artifact, &mut observer).unwrap(),
            TerminalExecutionResult::Unit
        );
        assert_eq!(
            observer.0,
            vec![(false, false), (false, true), (true, false)]
                .into_iter()
                .map(|(first, second)| vec![
                    TerminalScalarValue::Boolean(first),
                    TerminalScalarValue::Boolean(second)
                ])
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn trailing_unit_call_short_circuit_and_first_crash_do_not_enter_the_outer_callee() {
    for boundary in [false, true] {
        let target = if boundary {
            "Sink::record"
        } else {
            "Relay::record"
        };
        for semicolon in [false, true] {
            let punctuation = if semicolon { ";" } else { "" };
            for (arguments, cause) in [
                ("identity(false) && abort(), identity(true) || trap()", None),
                ("abort(), trap()", Some(terminal_psi::CrashCause::Abort)),
                ("trap(), abort()", Some(terminal_psi::CrashCause::Trap)),
                (
                    "identity(false) && abort(), identity(false) || trap()",
                    Some(terminal_psi::CrashCause::Trap),
                ),
            ] {
                let source = format!(
                    r#"
                    {IDENTITY}
                    machine abort() -> bool crashes Abort {{ crash Abort; }}
                    machine trap() -> bool crashes Trap {{ crash Trap; }}
                    pub data Sink {{}}
                    boundary machine Sink::record(first: bool, second: bool);
                    data Relay {{}}
                    machine Relay::record(first: bool, second: bool) {{ Sink::record(first, second); }}
                    data Root {{}}
                    machine Root::enter() crashes Abort crashes Trap {{
                        Sink::record(true, false);
                        {target}({arguments}){punctuation}
                    }}
                "#
                );
                let artifact = artifact(&checked(&source), semicolon, &[]);
                let mut observer = Observe::default();
                let result = execute(&artifact, &mut observer);
                let mut expected = vec![vec![
                    TerminalScalarValue::Boolean(true),
                    TerminalScalarValue::Boolean(false),
                ]];
                if let Some(cause) = cause {
                    assert!(
                        matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == cause)
                    );
                } else {
                    assert_eq!(result.unwrap(), TerminalExecutionResult::Unit);
                    expected.push(vec![
                        TerminalScalarValue::Boolean(false),
                        TerminalScalarValue::Boolean(true),
                    ]);
                }
                assert_eq!(observer.0, expected);
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
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let syntax = parse_syntax_trees(&tokens).unwrap();
            let resolved = lower_syntax_trees(&syntax).unwrap();
            let typed = lower_symbol_resolved_trees(&resolved).unwrap();
            assert!(
                typed_trees_to_checked_trees::lower_typed_trees(typed).is_err(),
                "Unit caller cannot silently discard the trailing bool result"
            );
        }
    }
}

#[test]
fn trailing_unit_call_source_and_occurrence_corruption_rejects() {
    for boundary in [false, true] {
        let checked = checked(&pair_source(boundary, true, false, "", "", false));
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
                checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
                "trailing call mutation={mutation}"
            );
        }
    }
}

#[test]
fn trailing_unit_call_semantic_modifiers_cannot_be_erased_into_an_ordinary_call() {
    for boundary in [false, true] {
        let checked = checked(&pair_source(boundary, true, false, "", "", false));
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
            unreachable!()
        };
        for modifier in [
            "quotient",
            "private layout",
            "static requirement",
            "machine argument",
            "evidence argument",
        ] {
            let mut changed = checked.clone();
            let ExpressionNode::Call(call) =
                changed.typed.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            let selected = typed_trees::expression::StaticMachineArgument {
                path: Box::new([]),
                application: None,
                const_literal: None,
                evidence_projection: None,
                symbol: call.target_symbol,
            };
            match modifier {
                "quotient" => {
                    call.quotient_operation =
                        Some(typed_trees::expression::QuotientOperationRequest {
                            kind: typed_trees::expression::QuotientOperationKind::Lift,
                            representative_operation: selected,
                            theorem_evidence: Box::new([]),
                        })
                }
                "private layout" => {
                    call.private_layout_operation =
                        Some(typed_trees::expression::PrivateLayoutOperationRequest {
                            selected_slot: selected,
                        })
                }
                "static requirement" => {
                    call.static_requirement_dispatch =
                        Some(typed_trees::typed_trees::StaticRequirementDispatch {
                            realization_state: call.target_symbol,
                            ..Default::default()
                        })
                }
                "machine argument" => call.machine_arguments = Box::new([selected]),
                "evidence argument" => call.evidence_arguments = Box::new([call.target.clone()]),
                _ => unreachable!(),
            }
            assert!(
                !validation::unit_return_call_is_supported(&changed.typed, root, state, expression,),
                "{modifier} must retain its own semantic route"
            );
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
                "{modifier} cannot publish an ordinary Unit call, boundary={boundary}"
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
            pub data Sink {{}}
            boundary machine Sink::unit();
            boundary machine Sink::record(value: bool);
            data Root {{}}
            machine Root::enter() {{ {body} }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = lower_syntax_trees(&syntax).unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
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
            typed_trees_to_checked_trees::lower_typed_trees(typed).is_err(),
            "Unit cannot supply a scalar operand or local: {body}"
        );
    }
}

#[test]
fn integer_unit_tail_preserves_nested_exact_casts_and_arithmetic_obligations() {
    let checked = checked(
        r#"
        machine identity(value: u8) -> u8 { value }
        pub data Sink {}
        boundary machine Sink::record(first: u16, second: u16);
        data Root {}
        machine Root::enter() {
            Sink::record((identity(identity(250u8)) as u16) + 1u16,
                         identity(19u8) as u16)
        }
    "#,
    );
    let artifact = artifact(&checked, false, &[]);
    let mut observer = Observe::default();
    assert_eq!(
        execute(&artifact, &mut observer).unwrap(),
        TerminalExecutionResult::Unit
    );
    assert_eq!(
        observer.0,
        vec![
            vec![251, 19]
                .into_iter()
                .map(|value| {
                    TerminalScalarValue::Integer {
                        scalar_type: semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            16,
                        )
                        .unwrap(),
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    }
                })
                .collect::<Vec<_>>()
        ]
    );
}

#[test]
fn multistate_pure_and_zero_operand_tails_retain_exact_source_occurrences() {
    for has_argument in [false, true] {
        let signature = if has_argument { "value: bool" } else { "" };
        let yes_argument = if has_argument { "true" } else { "" };
        let no_argument = if has_argument { "false" } else { "" };
        let checked = checked(&format!(
            r#"
            boundary trait Sink {{ machine record({signature}); }}
            data Root {{}}
            machine Root::enter(selected: bool) {{
                transition selected {{ true -> yes() _ -> no() }}
                state yes() {{ Sink::record({yes_argument}) }}
                state no() {{ Sink::record({no_argument}) }}
            }}
        "#
        ));
        let root = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .unwrap();
        let states = checked.typed.machine_states(root);
        let [_, yes, no] = states else {
            panic!("three authored states")
        };
        let expressions = [yes, no].map(|state| {
            let [StatementNode::Expression(expression)] = checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
            else {
                panic!("one authored leaf expression")
            };
            assert!(
                !checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .any(|(_, root)| root.state == state.symbol),
                "pure and zero-operand leaves do not need a computation root"
            );
            *expression
        });
        let artifact = verified_artifact(&checked);
        for selected in [false, true] {
            let mut observer = Observe::default();
            let result = interpret_terminal_artifact_with_effect_handler_measured(
                &artifact.0,
                &artifact.1,
                &AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(selected)],
                &[],
                &mut observer,
            )
            .unwrap();
            assert_eq!(result.value(), TerminalExecutionResult::Unit);
            assert_eq!(
                observer.0,
                vec![if has_argument {
                    vec![TerminalScalarValue::Boolean(selected)]
                } else {
                    Vec::new()
                }]
            );
        }
        let control = &checked.facts.flow.control;
        let yes_flow = control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.machine_symbol == root.symbol && state.state_symbol == yes.symbol)
            .unwrap();
        let [call] = control.calls.span(yes_flow.calls).unwrap() else {
            panic!("one outer call")
        };
        let call_handle = control
            .calls
            .iter()
            .find_map(|(handle, candidate)| std::ptr::eq(candidate, call).then_some(handle))
            .unwrap();
        for mutate_capture in [false, true] {
            let mut changed = checked.clone();
            if mutate_capture {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(call_handle)
                    .authored_expression = expressions[1];
            } else {
                let StatementNode::Expression(expression) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(yes.statement_nodes)[0]
                else {
                    unreachable!()
                };
                *expression = expressions[1];
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
                "another state's same-target call cannot replace this occurrence: argument={has_argument}, capture={mutate_capture}"
            );
        }
    }
}
