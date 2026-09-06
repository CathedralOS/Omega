//! Computed boundary operands belong to the selected authored control leaf.

use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralValue,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

const HELPERS: &str = r#"
    machine identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
    data Scalar {}
    machine Scalar::identity(input: u8) -> u8
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

fn encoded(checked: &checked_trees::CheckedTrees, state_count: usize) -> (Vec<u8>, Vec<u8>) {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), state_count, "no synthetic authored states");
    for state in states {
        assert!(
            checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .all(|statement| !matches!(statement, StatementNode::LocalData(_))),
            "computed operands do not manufacture source temporaries"
        );
    }
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.machine == machine.symbol
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                )
        })
        .collect::<Vec<_>>();
    assert!(
        !roots.is_empty(),
        "leaves retain their operand computation roots"
    );
    for (_, root) in roots {
        let state = states
            .iter()
            .find(|state| state.symbol == root.state)
            .unwrap();
        let statement = &checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[root.statement_ordinal as usize];
        let arguments = match statement {
            StatementNode::Call(call) => checked
                .typed
                .statement_table
                .expression_handles(call.arguments),
            StatementNode::Expression(expression) => {
                let ExpressionNode::Call(call) =
                    checked.typed.expression_table.expression(*expression)
                else {
                    panic!("authored Unit expression is a call");
                };
                checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)
            }
            _ => panic!("operand root belongs to an authored leaf call"),
        };
        assert!(arguments.contains(&computations.nodes.get(root.root).authored_root));
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("composed operand evaluation lowers");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independent verification after both codec roundtrips");
    (semantic, evidence)
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[derive(Default)]
struct ObserveCalls {
    calls: Vec<Vec<TerminalScalarValue>>,
    structural: Vec<Vec<TerminalStructuralValue>>,
    reject: bool,
}

impl TerminalEffectHandler for ObserveCalls {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("selected boundary call");
        };
        self.calls.push(arguments.clone());
        self.structural.push(structural_arguments.clone());
        if self.reject {
            return Err(TerminalEffectRejection {
                reason: "provider refused".into(),
            });
        }
        Ok(())
    }
}

fn start(artifact: &(Vec<u8>, Vec<u8>), arguments: &[TerminalScalarValue]) -> TerminalExecution {
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
    TerminalExecution::start_artifact_with_structural_arguments(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        arguments,
        &structural,
    )
    .unwrap()
}

fn arithmetic_source(topology: usize) -> (String, usize) {
    arithmetic_source_spelling(topology, false)
}

fn arithmetic_source_spelling(topology: usize, trailing: bool) -> (String, usize) {
    let terminator = if trailing { "" } else { ";" };
    let entry = match topology {
        0 => "transition first { true -> yes() _ -> no() }",
        1 => {
            "transition { _ -> dispatch(first) } state dispatch(flag: bool) { transition flag { true -> yes() _ -> no() } }"
        }
        2 => {
            "transition first { true -> dispatch(second) _ -> no() } state dispatch(flag: bool) { transition flag { true -> yes() _ -> middle() } } state middle() { Sink::finish(Scalar::identity(identity(7u8)) as u16, identity(255u8) as u16, 2u16); }"
        }
        _ => unreachable!(),
    };
    let entry = entry.replace("2u16);", &format!("2u16){terminator}"));
    let parameters = if topology == 2 {
        "first: bool, second: bool"
    } else {
        "first: bool"
    };
    (
        format!(
            r#"
        {HELPERS}
        boundary trait Sink {{ machine finish(first: u16, second: u16, marker: u16); }}
        data Main {{}}
        machine Main::main({parameters}) {{
            {entry}
            state yes() {{
                Sink::finish((Scalar::identity(identity(255u8)) as u16) + 1u16,
                             identity(7u8) as u16, 1u16){terminator}
            }}
            state no() {{
                Sink::finish(Scalar::identity(identity(255u8)) as u16,
                             identity(7u8) as u16, 3u16){terminator}
            }}
        }}
    "#
        ),
        topology + 3,
    )
}

#[test]
fn selected_leaves_evaluate_nested_operands_across_three_control_shapes() {
    for topology in 0..3 {
        for trailing in [false, true] {
            let (source, state_count) = arithmetic_source_spelling(topology, trailing);
            let checked = checked(&source);
            let artifact = encoded(&checked, state_count);
            for (first, second) in [(false, false), (true, false), (true, true)] {
                let mut arguments = vec![TerminalScalarValue::Boolean(first)];
                if topology == 2 {
                    arguments.push(TerminalScalarValue::Boolean(second));
                }
                let mut execution = start(&artifact, &arguments);
                let mut observer = ObserveCalls::default();
                assert_eq!(
                    execution
                        .resume_with_effect_handler(
                            &mut TerminalFuelMeter::unbounded(),
                            &mut observer
                        )
                        .unwrap(),
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
                );
                let expected = if !first {
                    [255, 7, 3]
                } else if topology == 2 && !second {
                    [7, 255, 2]
                } else {
                    [256, 7, 1]
                };
                assert_eq!(
                    observer.calls,
                    vec![expected.map(|value| unsigned(16, value)).to_vec()]
                );
            }
        }
    }
}

#[test]
fn unselected_leaves_and_short_circuit_operands_do_not_crash() {
    for trailing in [false, true] {
        let terminator = if trailing { "" } else { ";" };
        for (first, second) in [(false, true), (true, false), (false, false)] {
            let source = format!(
                r#"
        machine abort() -> bool crashes Abort {{ crash Abort; }}
        machine trap() -> bool crashes Trap {{ crash Trap; }}
        boundary trait Sink {{ machine finish(first: bool, second: bool); }}
        data Main {{}}
        machine Main::main(selected: bool)
        crashes Abort crashes Trap {{
            transition selected {{ true -> yes() _ -> no() }}
            state yes() {{
                Sink::finish({first} && abort(), {second} || trap()){terminator}
            }}
            state no() {{ Sink::finish(false, true){terminator} }}
        }}
    "#
            );
            let checked = checked(&source);
            let artifact = encoded(&checked, 3);
            for selected in [false, true] {
                let cause = if !selected {
                    None
                } else if first {
                    Some(terminal_psi::CrashCause::Abort)
                } else if !second {
                    Some(terminal_psi::CrashCause::Trap)
                } else {
                    None
                };
                let mut execution = start(&artifact, &[TerminalScalarValue::Boolean(selected)]);
                let mut observer = ObserveCalls::default();
                let result = execution
                    .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer);
                if let Some(cause) = cause {
                    assert!(
                        matches!(&result, Ok(TerminalExecutionStatus::Crashed(crash)) if crash.cause == cause),
                        "selected={selected}, expected={cause:?}, actual={result:?}"
                    );
                    assert!(observer.calls.is_empty());
                } else {
                    assert_eq!(
                        result.unwrap(),
                        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
                    );
                    assert_eq!(
                        observer.calls,
                        vec![vec![
                            TerminalScalarValue::Boolean(false),
                            TerminalScalarValue::Boolean(true)
                        ]]
                    );
                }
            }
        }
    }
}

#[test]
fn first_leaf_argument_crash_precedes_later_call_even_under_exact_casts() {
    for trailing in [false, true] {
        let terminator = if trailing { "" } else { ";" };
        for (first, second, cause) in [
            ("Abort", "Trap", terminal_psi::CrashCause::Abort),
            ("Trap", "Abort", terminal_psi::CrashCause::Trap),
        ] {
            let source = format!(
                r#"
            machine first() -> u8 crashes {first} {{ crash {first}; }}
            machine second() -> u8 crashes {second} {{ crash {second}; }}
            boundary trait Sink {{ machine finish(first: u16, second: u16); }}
            data Main {{}}
            machine Main::main(selected: bool) crashes Abort crashes Trap {{
                transition selected {{ true -> yes() _ -> no() }}
                state yes() {{ Sink::finish(first() as u16, second() as u16){terminator} }}
                state no() {{ Sink::finish(second() as u16, first() as u16){terminator} }}
            }}
        "#
            );
            let artifact = encoded(&checked(&source), 3);
            for selected in [true, false] {
                let mut execution = start(&artifact, &[TerminalScalarValue::Boolean(selected)]);
                let mut observer = ObserveCalls::default();
                let expected = if selected {
                    cause
                } else if cause == terminal_psi::CrashCause::Abort {
                    terminal_psi::CrashCause::Trap
                } else {
                    terminal_psi::CrashCause::Abort
                };
                let result = execution
                    .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer);
                assert!(
                    matches!(&result, Ok(TerminalExecutionStatus::Crashed(crash)) if crash.cause == expected),
                    "selected={selected}, expected={expected:?}, actual={result:?}"
                );
                assert!(observer.calls.is_empty());
            }
        }
    }
}

#[test]
fn linear_claim_stays_live_until_selected_computed_boundary_call_succeeds() {
    let source = format!(
        r#"
        {HELPERS}
        pub data Receipt [linear] {{ value: u64; }}
        boundary machine Receipt::settle(self, value: u16) ensures true;
        data Main {{}}
        machine Main::main(selected: bool, receipt: Receipt) {{
            transition selected {{ true -> yes(receipt) _ -> no(receipt) }}
            state yes(receipt: Receipt) {{ receipt.settle(identity(255u8) as u16); }}
            state no(receipt: Receipt) {{ receipt.settle(Scalar::identity(255u8) as u16); }}
        }}
    "#
    );
    let artifact = encoded(&checked(&source), 3);
    for selected in [false, true] {
        let mut execution = start(&artifact, &[TerminalScalarValue::Boolean(selected)]);
        let initial_claims = execution.live_claim_frontier().collect::<Vec<_>>();
        assert_eq!(initial_claims.len(), 1);
        let mut rejected = ObserveCalls {
            reject: true,
            ..ObserveCalls::default()
        };
        assert!(matches!(
            execution
                .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut rejected),
            Err(TerminalInterpretError::EffectRejected { .. })
        ));
        assert_eq!(
            execution.live_claim_frontier().collect::<Vec<_>>(),
            initial_claims
        );
        assert_eq!(rejected.calls, vec![vec![unsigned(16, 255)]]);
        let mut accepted = ObserveCalls::default();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut accepted)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(execution.live_claim_frontier().count(), 0);
        assert_eq!(accepted.calls, rejected.calls);
        assert_eq!(accepted.structural, rejected.structural);
        assert_eq!(accepted.structural[0][0].opaque_identity, 100);
    }
}

#[test]
fn nominal_boundary_leaf_calls_keep_authored_callable_identity() {
    let (source, state_count) = arithmetic_source(0);
    let source = source
        .replace("Sink::finish(", "SinkParam(")
        .replace("machine Main::main(first: bool)",
            "machine Main::main<machine SinkParam>(first: bool)\nwhere machine SinkParam satisfies Sink::finish;");
    let checked = checked(&source);
    let artifact = encoded(&checked, state_count);
    for selected in [false, true] {
        let mut execution = start(&artifact, &[TerminalScalarValue::Boolean(selected)]);
        let mut observer = ObserveCalls::default();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        let expected = if selected { [256, 7, 1] } else { [255, 7, 3] };
        assert_eq!(
            observer.calls,
            vec![expected.map(|value| unsigned(16, value)).to_vec()]
        );
    }
    for (handle, call) in checked.facts.flow.control.calls.iter() {
        let Some((_, signature)) = checked
            .typed
            .machine_parameter_signature(call.target_symbol)
        else {
            continue;
        };
        assert_ne!(call.target_symbol, signature.symbol);
        let mut changed = checked.clone();
        changed
            .facts
            .flow
            .control
            .calls
            .get_mut(handle)
            .target_symbol = signature.symbol;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "flow target must retain the callable parameter, not just its requirement"
        );
    }
    let (state_handle, state) = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| !state.calls.is_empty())
        .max_by_key(|(_, state)| state.calls.start().arena_index() + state.calls.count())
        .unwrap();
    let call = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| call.call_ordinal == 0)
        .unwrap();
    let (_, signature) = checked
        .typed
        .machine_parameter_signature(call.target_symbol)
        .unwrap();
    let mut duplicate = call.clone();
    duplicate.target_symbol = signature.symbol;
    let mut changed = checked.clone();
    let mut calls = state.calls;
    changed
        .facts
        .flow
        .control
        .calls
        .append_to_span(&mut calls, duplicate);
    changed
        .facts
        .flow
        .control
        .states
        .get_mut(state_handle)
        .calls = calls;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
        "same-coordinate call duplicates reject even when targets differ"
    );
}

#[test]
fn composed_operand_roots_and_nested_occurrences_rejoin_their_source_leaf() {
    for trailing in [false, true] {
        let (source, state_count) = arithmetic_source_spelling(2, trailing);
        let checked = checked(&source);
        encoded(&checked, state_count);
        let plans = &checked.facts.values.scalar_computations;
        let roots = plans
            .roots
            .iter()
            .filter(|(_, root)| {
                matches!(
                    root.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                )
            })
            .collect::<Vec<_>>();
        for (handle, root) in &roots {
            let other = roots
                .iter()
                .find(|(_, other)| other.state != root.state)
                .unwrap()
                .1;
            for mutation in 0..3 {
                let mut changed = checked.clone();
                let plans = &mut changed.facts.values.scalar_computations;
                match mutation {
                    0 => plans.roots.get_mut(*handle).root = other.root,
                    1 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
                    2 => plans.roots.get_mut(*handle).statement_ordinal += 1,
                    _ => unreachable!(),
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err()
                );
            }
        }
        for (_, node) in plans.nodes.iter() {
            let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
                continue;
            };
            let authored = checked
                .facts
                .flow
                .control
                .calls
                .get(source_call)
                .authored_expression;
            let ExpressionNode::Call(call) = checked.typed.expression_table.expression(authored)
            else {
                unreachable!();
            };
            if call.receiver.is_valid() {
                let receiver = call.receiver;
                let argument = checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)[0];
                let mut changed = checked.clone();
                let ExpressionNode::Call(call) =
                    changed.typed.expression_table.expression_mut(authored)
                else {
                    unreachable!();
                };
                call.receiver = argument;
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "computed leaf helper cannot discard a runtime receiver in place of its static qualifier"
                );
                let mut changed = checked.clone();
                let ExpressionNode::Name(path) =
                    changed.typed.expression_table.expression_mut(receiver)
                else {
                    unreachable!();
                };
                path.symbol = symbols::SymbolHandle::invalid();
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err()
                );
            }
            let mut changed = checked.clone();
            changed
                .facts
                .flow
                .control
                .calls
                .get_mut(source_call)
                .authored_expression = arena::Handle::invalid();
            assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err());
        }
    }
}
