use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarComputationKind, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecutionResult, TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::statement::StatementNode;

const IDENTITY: &str = r#"
    machine identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

fn encoded(checked: &checked_trees::CheckedTrees, locals: &[&str]) -> (Vec<u8>, Vec<u8>) {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), 1, "no synthetic source states");
    let statements = checked
        .typed
        .statement_table
        .statements(states[0].statement_nodes);
    let authored_locals = statements
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) => Some(local.name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(authored_locals, locals, "no hoisted argument temporaries");
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.machine == machine.symbol
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                        | CheckedScalarExpressionRole::UnitCallArgument { .. }
                )
        })
        .collect::<Vec<_>>();
    assert!(
        !roots.is_empty(),
        "nested arguments retain checked computation roots"
    );
    for (_, root) in roots {
        let StatementNode::Call(call) = &statements[root.statement_ordinal as usize] else {
            panic!("computed argument stays at its authored statement call");
        };
        let arguments = checked
            .typed
            .statement_table
            .expression_handles(call.arguments);
        assert!(arguments.contains(&computations.nodes.get(root.root).authored_root));
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("nested call arguments lower");
    let artifact = (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    );
    let module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independent decoded verification");
    artifact
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[derive(Default)]
struct ObserveCalls {
    scalar: Vec<Vec<TerminalScalarValue>>,
    structural: Vec<Vec<TerminalStructuralValue>>,
}

impl TerminalEffectHandler for ObserveCalls {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("outer boundary effect");
        };
        self.scalar.push(arguments.clone());
        self.structural.push(structural_arguments.clone());
        Ok(())
    }
}

fn execute(
    artifact: &(Vec<u8>, Vec<u8>),
    arguments: &[TerminalScalarValue],
    observer: &mut ObserveCalls,
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    let module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let structural = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 100 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    interpret_terminal_artifact_with_effect_handler_measured(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        arguments,
        &structural,
        observer,
    )
    .map(|execution| execution.value())
}

fn outer(signature: &str, arguments: &str, boundary: bool) -> String {
    if boundary {
        format!("boundary trait Sink {{ machine finish({signature}) reaches Sink; }}")
    } else {
        format!(
            "boundary trait Host {{ machine finish({signature}) reaches Host; }} \
             data Sink {{}} machine Sink::finish({signature}) reaches Host {{ Host::finish({arguments}); }}"
        )
    }
}

fn arithmetic_source(boundary: bool) -> String {
    let outer = outer(
        "first: u16, second: u16, third: u16",
        "first, second, third",
        boundary,
    );
    let reach = if boundary { "Sink" } else { "Host" };
    format!(
        r#"
        {IDENTITY}
        machine alternative(input: u8) -> u8
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        {{ 0u8 }}
        {outer}
        data Main {{}}
        machine Main::main(left: u8, right: u8) reaches {reach} {{
            Sink::finish((identity(identity(left)) as u16) + 1u16,
                         identity(right) as u16, 19u16);
        }}
    "#
    )
}

#[test]
fn nested_scalar_statement_arguments_complete_before_boundary_or_unit_outer_calls() {
    for boundary in [false, true] {
        let checked = checked(&arithmetic_source(boundary));
        let artifact = encoded(&checked, &[]);
        for (left, right) in [(0, 7), (255, 3)] {
            let mut observer = ObserveCalls::default();
            assert_eq!(
                execute(
                    &artifact,
                    &[unsigned(8, left), unsigned(8, right)],
                    &mut observer
                )
                .unwrap(),
                TerminalExecutionResult::Unit
            );
            assert_eq!(
                observer.scalar,
                vec![vec![
                    unsigned(16, left + 1),
                    unsigned(16, right),
                    unsigned(16, 19)
                ]]
            );
        }
    }
}

#[test]
fn nested_arguments_keep_saved_immutable_values_in_the_caller_namespace() {
    for boundary in [false, true] {
        let outer = outer("first: u8, second: u8", "first, second", boundary);
        let reach = if boundary { "Sink" } else { "Host" };
        let source = format!(
            r#"
            {IDENTITY}
            data Scalar {{}}
            machine Scalar::first(left: u8, right: u8) -> u8
            requires 0u8 == 0u8
            ensures result == left
            {{ left }}
            {outer}
            data Main {{}}
            machine Main::main(left: u8, right: u8) reaches {reach} {{
                let saved: u8 = Scalar::first(left, right);
                Sink::finish(identity(identity(saved)), identity(right));
            }}
        "#
        );
        let checked = checked(&source);
        let artifact = encoded(&checked, &["saved"]);
        let mut observer = ObserveCalls::default();
        assert_eq!(
            execute(
                &artifact,
                &[unsigned(8, 23), unsigned(8, 70)],
                &mut observer
            )
            .unwrap(),
            TerminalExecutionResult::Unit
        );
        assert_eq!(
            observer.scalar,
            vec![vec![unsigned(8, 23), unsigned(8, 70)]]
        );
    }
}

#[test]
fn successive_computed_outer_calls_reset_arguments_and_admit_no_self_attached_helpers() {
    for boundary in [false, true] {
        let outer = outer(
            "first: u16, second: u16, third: u16",
            "first, second, third",
            boundary,
        );
        let reach = if boundary { "Sink" } else { "Host" };
        let source = format!(
            r#"
            {IDENTITY}
            data Scalar {{}}
            machine Scalar::identity(input: u8) -> u8
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            {outer}
            data Main {{}}
            machine Main::main(left: u8, right: u8) reaches {reach} {{
                Sink::finish((Scalar::identity(identity(left)) as u16) + 1u16,
                             Scalar::identity(right) as u16, 19u16);
                Sink::finish((Scalar::identity(Scalar::identity(right)) as u16) + 1u16,
                             identity(Scalar::identity(left)) as u16, 31u16);
            }}
        "#
        );
        let checked = checked(&source);
        let artifact = encoded(&checked, &[]);
        let mut observer = ObserveCalls::default();
        assert_eq!(
            execute(
                &artifact,
                &[unsigned(8, 255), unsigned(8, 3)],
                &mut observer
            )
            .unwrap(),
            TerminalExecutionResult::Unit
        );
        assert_eq!(
            observer.scalar,
            vec![
                vec![unsigned(16, 256), unsigned(16, 3), unsigned(16, 19)],
                vec![unsigned(16, 4), unsigned(16, 255), unsigned(16, 31)],
            ],
            "two outer effects occur once each in authored statement order"
        );
        assert_static_qualifier_custody(&checked);
    }
}

#[test]
fn embedded_static_scalar_helpers_retain_transitive_computation_targets() {
    use typed_trees::expression::ExpressionNode;

    for boundary in [false, true] {
        let outer = outer("first: u8, second: u8", "first, second", boundary);
        let reach = if boundary { "Sink" } else { "Host" };
        let source = format!(
            r#"
            data Leaf {{}}
            machine Leaf::identity(input: u8) -> u8
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine Leaf::other(input: u8) -> u8
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ 0u8 }}
            data Scalar {{}}
            machine Scalar::wrapper(input: u8) -> u8
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ Leaf::identity(input) }}
            {outer}
            data Main {{}}
            machine Main::main(left: u8, right: u8) reaches {reach} {{
                Sink::finish(Scalar::wrapper(left), Scalar::wrapper(right));
            }}
            "#
        );
        let checked = checked(&source);
        let artifact = encoded(&checked, &[]);
        let mut observer = ObserveCalls::default();
        assert_eq!(
            execute(
                &artifact,
                &[unsigned(8, 17), unsigned(8, 93)],
                &mut observer
            )
            .unwrap(),
            TerminalExecutionResult::Unit
        );
        assert_eq!(
            observer.scalar,
            vec![vec![unsigned(8, 17), unsigned(8, 93)]]
        );

        let wrapper = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Scalar::wrapper")
            .unwrap();
        let root = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.machine == wrapper.symbol)
            .expect("wrapper retains its transitive call as a computation");
        let CheckedScalarComputationKind::Call { source_call, .. } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(root.root)
            .kind
        else {
            panic!("wrapper root invokes the statically qualified leaf");
        };
        let authored = checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .authored_expression;
        let ExpressionNode::Call(call) = checked.typed.expression_table.expression(authored) else {
            panic!("captured leaf call");
        };
        let value = checked
            .typed
            .expression_table
            .expression_handles(call.arguments)[0];
        let replacement = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Leaf::other")
            .unwrap();
        let replacement_state = checked.typed.machine_states(replacement)[0].symbol;
        for mutation in 0..3 {
            let mut changed = checked.clone();
            if mutation == 2 {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(source_call)
                    .authored_expression =
                    arena::Handle::from_parts(authored.arena_index(), authored.generation() + 1);
            } else {
                let ExpressionNode::Call(call) =
                    changed.typed.expression_table.expression_mut(authored)
                else {
                    unreachable!();
                };
                if mutation == 0 {
                    call.receiver = value;
                } else {
                    call.target_symbol = replacement_state;
                }
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "transitive static computation source mutation {mutation} must reject"
            );
        }
    }
}

fn assert_static_qualifier_custody(checked: &checked_trees::CheckedTrees) {
    use typed_trees::expression::ExpressionNode;

    let owner = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::identity")
        .unwrap();
    let (source_call, authored, receiver, value, value_symbol) = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(_, node)| {
            let CheckedScalarComputationKind::Call {
                source_call,
                target_machine,
                ..
            } = node.kind
            else {
                return None;
            };
            if target_machine != owner.symbol {
                return None;
            }
            let flow = checked.facts.flow.control.calls.get(source_call);
            let ExpressionNode::Call(call) = checked
                .typed
                .expression_table
                .expression(flow.authored_expression)
            else {
                return None;
            };
            let [value] = checked
                .typed
                .expression_table
                .expression_handles(call.arguments)
            else {
                return None;
            };
            let ExpressionNode::Name(value_path) =
                checked.typed.expression_table.expression(*value)
            else {
                return None;
            };
            Some((
                source_call,
                flow.authored_expression,
                call.receiver,
                *value,
                value_path.symbol,
            ))
        })
        .expect("qualified helper call has an actual scalar value argument");
    let flow = checked.facts.flow.control.calls.get(source_call);
    assert!(flow.has_receiver);
    assert_eq!(flow.receiver_symbol, owner.attached_data_symbol);
    let ExpressionNode::Name(qualifier) = checked.typed.expression_table.expression(receiver)
    else {
        panic!("accepted static qualifier is a live type name");
    };
    assert_eq!(qualifier.symbol, owner.attached_data_symbol);
    assert_ne!(value_symbol, owner.attached_data_symbol);

    let main = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let statements = checked.typed.machine_states(main)[0].statement_nodes;
    let StatementNode::Call(outer) = &checked.typed.statement_table.statements(statements)[0]
    else {
        panic!("authored outer call");
    };
    let non_name = checked
        .typed
        .statement_table
        .expression_handles(outer.arguments)[2];
    assert!(!matches!(
        checked.typed.expression_table.expression(non_name),
        ExpressionNode::Name(_)
    ));
    for mutation in 0..7 {
        let mut changed = checked.clone();
        match mutation {
            0 | 1 | 2 | 6 => {
                let ExpressionNode::Call(call) =
                    changed.typed.expression_table.expression_mut(authored)
                else {
                    unreachable!();
                };
                call.receiver = match mutation {
                    0 | 6 => value,
                    1 => non_name,
                    2 => {
                        arena::Handle::from_parts(receiver.arena_index(), receiver.generation() + 1)
                    }
                    _ => unreachable!(),
                };
                if mutation == 6 {
                    changed
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(source_call)
                        .receiver_symbol = value_symbol;
                }
            }
            3 => {
                let ExpressionNode::Name(path) =
                    changed.typed.expression_table.expression_mut(receiver)
                else {
                    unreachable!();
                };
                path.symbol = main.attached_data_symbol;
            }
            4 => {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(source_call)
                    .receiver_symbol = main.attached_data_symbol
            }
            5 => {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(source_call)
                    .has_receiver = false
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "static qualifier custody mutation={mutation}: no runtime receiver may be erased"
        );
    }
}

#[test]
fn boolean_arguments_skip_unselected_crashes_and_stop_before_the_outer_effect() {
    for boundary in [false, true] {
        let outer = outer("first: bool, second: bool", "first, second", boundary);
        let reach = if boundary { "Sink" } else { "Host" };
        let source = format!(
            r#"
            machine abort() -> bool crashes Abort {{ crash Abort; }}
            machine trap() -> bool crashes Trap {{ crash Trap; }}
            {outer}
            data Main {{}}
            machine Main::main(first: bool, second: bool)
            reaches {reach}
            crashes Abort
            crashes Trap
            {{ Sink::finish(first && abort(), second || trap()); }}
        "#
        );
        let checked = checked(&source);
        let artifact = encoded(&checked, &[]);
        for (first, second, cause) in [
            (false, true, None),
            (true, true, Some(terminal_psi::CrashCause::Abort)),
            (false, false, Some(terminal_psi::CrashCause::Trap)),
            (true, false, Some(terminal_psi::CrashCause::Abort)),
        ] {
            let mut observer = ObserveCalls::default();
            let result = execute(
                &artifact,
                &[
                    TerminalScalarValue::Boolean(first),
                    TerminalScalarValue::Boolean(second),
                ],
                &mut observer,
            );
            if let Some(cause) = cause {
                assert!(matches!(result,
                    Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == cause));
                assert!(
                    observer.scalar.is_empty(),
                    "outer call is not entered after an argument crash"
                );
            } else {
                assert_eq!(result.unwrap(), TerminalExecutionResult::Unit);
                assert_eq!(
                    observer.scalar,
                    vec![vec![
                        TerminalScalarValue::Boolean(false),
                        TerminalScalarValue::Boolean(true)
                    ]]
                );
            }
        }
    }
}

#[test]
fn earlier_argument_crashes_precede_cast_wrapped_later_calls() {
    for boundary in [false, true] {
        for (first, second, expected) in [
            ("Abort", "Trap", terminal_psi::CrashCause::Abort),
            ("Trap", "Abort", terminal_psi::CrashCause::Trap),
        ] {
            let outer = outer("first: u16, second: u16", "first, second", boundary);
            let reach = if boundary { "Sink" } else { "Host" };
            let source = format!(
                r#"
                machine first() -> u16 crashes {first} {{ crash {first}; }}
                machine second() -> u8 crashes {second} {{ crash {second}; }}
                {outer}
                data Main {{}}
                machine Main::main()
                reaches {reach}
                crashes Abort
                crashes Trap
                {{ Sink::finish(first(), second() as u16); }}
            "#
            );
            let checked = checked(&source);
            let artifact = encoded(&checked, &[]);
            let mut observer = ObserveCalls::default();
            assert!(matches!(execute(&artifact, &[], &mut observer),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected));
            assert!(
                observer.scalar.is_empty(),
                "no outer effect after earlier argument crash"
            );
        }
    }
}

#[test]
fn nested_scalar_arguments_preserve_interleaved_structural_formals() {
    for boundary in [false, true] {
        let outer = outer(
            "first: u8, first_token: Token, second: u8, second_token: Token",
            "first, first_token, second, second_token",
            boundary,
        );
        let reach = if boundary { "Sink" } else { "Host" };
        let source = format!(
            r#"
            pub data Token {{ flag: bool; }}
            {IDENTITY}
            {outer}
            data Main {{}}
            machine Main::main(first_token: Token, left: u8, second_token: Token, right: u8)
            reaches {reach}
            {{ Sink::finish(identity(left), first_token, identity(identity(right)), second_token); }}
        "#
        );
        let checked = checked(&source);
        let artifact = encoded(&checked, &[]);
        assert_affine_entry_parameters(&artifact, 2);
        let mut observer = ObserveCalls::default();
        assert_eq!(
            execute(
                &artifact,
                &[unsigned(8, 23), unsigned(8, 70)],
                &mut observer
            )
            .unwrap(),
            TerminalExecutionResult::Unit
        );
        assert_eq!(
            observer.scalar,
            vec![vec![unsigned(8, 23), unsigned(8, 70)]]
        );
        assert_eq!(observer.structural.len(), 1);
        assert_eq!(
            observer.structural[0]
                .iter()
                .map(|value| value.opaque_identity)
                .collect::<Vec<_>>(),
            vec![100, 101]
        );
    }
}

fn assert_affine_entry_parameters(artifact: &(Vec<u8>, Vec<u8>), expected: usize) {
    let module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(entry.structural_parameters.len(), expected);
    assert!(
        entry.structural_parameters.iter().all(|parameter| {
            parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && parameter.access == terminal_psi::StructuralAccess::Owned
        }),
        "source records are owned affine resources, not unrestricted stand-ins"
    );
}

#[test]
fn crashing_argument_keeps_affine_resources_live_and_normal_return_discards_only_the_spare() {
    for boundary in [false, true] {
        for (cause, expected) in [
            ("Trap", terminal_psi::CrashCause::Trap),
            ("Abort", terminal_psi::CrashCause::Abort),
        ] {
            let outer = outer(
                "first: bool, token: Token, second: bool",
                "first, token, second",
                boundary,
            );
            let reach = if boundary { "Sink" } else { "Host" };
            let source = format!(
                r#"
                pub data Token {{ flag: bool; }}
                machine checked_flag(flag: bool) -> bool
                requires true == true
                ensures true == true
                crashes {cause}
                {{ transition {{ flag -> true }} crash {cause}; }}
                {outer}
                data Main {{}}
                machine Main::main(token: Token, spare: Token, flag: bool)
                reaches {reach}
                crashes {cause}
                {{ Sink::finish(checked_flag(flag), token, true); }}
            "#
            );
            let checked = checked(&source);
            let artifact = encoded(&checked, &[]);
            assert_affine_entry_parameters(&artifact, 2);
            let module = decode_module(&artifact.0).unwrap();
            let entry = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            let spare = entry.structural_parameters[1].place;
            let discards = entry
                .blocks
                .iter()
                .filter_map(|block| {
                    if let terminal_psi::Terminator::ReturnUnit {
                        trivial_affine_discards,
                        ..
                    } = &block.terminator
                    {
                        Some(trivial_affine_discards)
                    } else {
                        None
                    }
                })
                .flatten()
                .collect::<Vec<_>>();
            assert_eq!(
                discards.len(),
                1,
                "one normal-return cleanup for the untransferred spare"
            );
            assert_eq!(*discards[0], spare);
            let mut observer = ObserveCalls::default();
            assert_eq!(
                execute(
                    &artifact,
                    &[TerminalScalarValue::Boolean(true)],
                    &mut observer
                )
                .unwrap(),
                TerminalExecutionResult::Unit
            );
            assert_eq!(
                observer.scalar,
                vec![vec![
                    TerminalScalarValue::Boolean(true),
                    TerminalScalarValue::Boolean(true)
                ]]
            );
            assert_eq!(observer.structural.len(), 1);
            assert_eq!(
                observer.structural[0].len(),
                1,
                "exactly one whole token reaches the outer boundary"
            );
            assert_eq!(observer.structural[0][0].opaque_identity, 100);
            let mut observer = ObserveCalls::default();
            assert!(
                matches!(execute(&artifact, &[TerminalScalarValue::Boolean(false)], &mut observer),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected)
            );
            assert!(observer.scalar.is_empty());
            assert!(
                observer.structural.is_empty(),
                "argument crash has no outer transfer or cleanup successor"
            );
        }
    }
}

#[test]
fn pure_boolean_short_circuit_arguments_remain_selective_beside_a_computation() {
    for boundary in [false, true] {
        let outer = outer(
            "first: bool, second: bool, third: bool",
            "first, second, third",
            boundary,
        );
        let reach = if boundary { "Sink" } else { "Host" };
        let source = format!(
            r#"
            machine identity_bool(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            {outer}
            data Main {{}}
            machine Main::main(left: bool, right: bool) reaches {reach}
            {{ Sink::finish(identity_bool(left), left && right, left || right); }}
        "#
        );
        let checked = checked(&source);
        let artifact = encoded(&checked, &[]);
        for left in [false, true] {
            for right in [false, true] {
                let mut observer = ObserveCalls::default();
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(left),
                            TerminalScalarValue::Boolean(right)
                        ],
                        &mut observer
                    )
                    .unwrap(),
                    TerminalExecutionResult::Unit
                );
                assert_eq!(
                    observer.scalar,
                    vec![vec![
                        TerminalScalarValue::Boolean(left),
                        TerminalScalarValue::Boolean(left && right),
                        TerminalScalarValue::Boolean(left || right)
                    ]]
                );
            }
        }
    }
}

#[test]
fn nested_argument_roots_and_call_occurrences_rejoin_authored_source() {
    for boundary in [false, true] {
        let checked = checked(&arithmetic_source(boundary));
        encoded(&checked, &[]);
        let main = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .unwrap();
        let computations = &checked.facts.values.scalar_computations;
        let roots = computations
            .roots
            .iter()
            .filter(|(_, root)| root.machine == main.symbol)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2, "two computed arguments, one pure argument");
        for mutation in 0..4 {
            let mut changed = checked.clone();
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == main.symbol)
                .unwrap();
            let scalar_arguments = match &mut plan.operations[0] {
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                }
                | CheckedUnitEffectOperationPlan::CallUnit {
                    scalar_arguments, ..
                } => scalar_arguments,
                _ => panic!("retained outer statement call"),
            };
            assert!(matches!(
                scalar_arguments.as_slice(),
                [
                    CheckedCallScalarArgument::Computation(_),
                    CheckedCallScalarArgument::Computation(_),
                    CheckedCallScalarArgument::Pure(_),
                ]
            ));
            match mutation {
                0 => {
                    scalar_arguments[0] =
                        CheckedCallScalarArgument::Computation(arena::Handle::invalid())
                }
                1 => scalar_arguments.swap(0, 1),
                2 => scalar_arguments[0] = scalar_arguments[2].clone(),
                3 => {
                    scalar_arguments.remove(0);
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "outer argument plan mutation={mutation}"
            );
        }
        for (handle, root) in &roots {
            for mutation in 0..4 {
                let mut changed = checked.clone();
                let plans = &mut changed.facts.values.scalar_computations;
                match mutation {
                    0 => plans.roots.get_mut(*handle).root = arena::Handle::invalid(),
                    1 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
                    2 => plans.roots.get_mut(*handle).statement_ordinal += 1,
                    3 => plans.roots.get_mut(*handle).role = CheckedScalarExpressionRole::Return,
                    _ => unreachable!(),
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "root mutation={mutation}"
                );
            }
        }
        let calls = computations
            .nodes
            .iter()
            .filter_map(|(handle, node)| {
                matches!(node.kind, CheckedScalarComputationKind::Call { .. }).then_some(handle)
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);
        let alternate_machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "alternative")
            .unwrap();
        let alternate_target = checked.typed.machine_states(alternate_machine)[0].symbol;
        for handle in &calls {
            let CheckedScalarComputationKind::Call { source_call, .. } =
                computations.nodes.get(*handle).kind
            else {
                unreachable!();
            };
            let authored = checked
                .facts
                .flow
                .control
                .calls
                .get(source_call)
                .authored_expression;
            for mutation in 0..4 {
                let mut changed = checked.clone();
                let replacement = match mutation {
                    0 => arena::Handle::invalid(),
                    1 => {
                        arena::Handle::from_parts(authored.arena_index(), authored.generation() + 1)
                    }
                    2 => {
                        let other = calls.iter().find(|other| *other != handle).unwrap();
                        let CheckedScalarComputationKind::Call { source_call, .. } =
                            computations.nodes.get(*other).kind
                        else {
                            unreachable!();
                        };
                        checked
                            .facts
                            .flow
                            .control
                            .calls
                            .get(source_call)
                            .authored_expression
                    }
                    3 => computations.nodes.get(roots[0].1.root).authored_root,
                    _ => unreachable!(),
                };
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(source_call)
                    .authored_expression = replacement;
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "authored nested-call occurrence mutation={mutation}"
                );
            }
            let mut changed = checked.clone();
            let typed_trees::expression::ExpressionNode::Call(call) =
                changed.typed.expression_table.expression_mut(authored)
            else {
                panic!("live nested authored call");
            };
            call.target_symbol = alternate_target;
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "same-carrier authored nested target substitution"
            );
            for mutation in 0..3 {
                let mut changed = checked.clone();
                let CheckedScalarComputationKind::Call {
                    source_call,
                    target_state,
                    call_ordinal,
                    ..
                } = &mut changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(*handle)
                    .kind
                else {
                    unreachable!();
                };
                match mutation {
                    0 => *source_call = arena::Handle::invalid(),
                    1 => *target_state = symbols::SymbolHandle::invalid(),
                    2 => *call_ordinal += 1,
                    _ => unreachable!(),
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "call mutation={mutation}"
                );
            }
        }
        let mut changed = checked.clone();
        let statements = checked.typed.machine_states(main)[0].statement_nodes;
        let StatementNode::Call(call) = &checked.typed.statement_table.statements(statements)[0]
        else {
            panic!("authored outer call");
        };
        let arguments = checked
            .typed
            .statement_table
            .expression_handles(call.arguments);
        changed
            .typed
            .statement_table
            .set_expression_handle_at_offset(call.arguments, 0, arguments[1]);
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "same-carrier authored operand swap"
        );
        let mut changed = checked.clone();
        let StatementNode::Call(call) =
            &mut changed.typed.statement_table.statements_mut(statements)[0]
        else {
            unreachable!();
        };
        call.target_symbol = symbols::SymbolHandle::invalid();
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "authored outer target changed"
        );
    }
}
