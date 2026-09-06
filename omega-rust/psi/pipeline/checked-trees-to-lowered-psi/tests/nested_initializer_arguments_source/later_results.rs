use super::*;

fn encoded_locals(checked: &checked_trees::CheckedTrees, names: &[&str]) -> (Vec<u8>, Vec<u8>) {
    let machine = main_machine(checked);
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), 1, "initializers remain in the authored state");
    let locals = checked
        .typed
        .statement_table
        .statements(states[0].statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) => {
                assert!(!local.is_mutable);
                Some(local.name.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locals, names, "no synthetic source bindings");
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("later immutable result initializer lowers");
    let artifact = (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    );
    let module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    artifact
}

#[test]
fn pure_local_before_computed_scalar_result_initializer_reaches_consumer() {
    let source = scalar_source(false)
        .replace("let result: u16", "let prefix: u8 = left;\nlet result: u16")
        .replace(
            "Scalar::identity(identity(left))",
            "Scalar::identity(identity(prefix))",
        );
    let checked = checked(&source);
    let artifact = encoded_locals(&checked, &["prefix", "result"]);
    let mut observer = ObserveResults::default();
    assert_eq!(
        execute(
            &artifact,
            &[unsigned(8, 255), unsigned(8, 7)],
            &mut observer
        )
        .unwrap(),
        TerminalExecutionResult::Unit
    );
    assert_eq!(observer.calls, vec![vec![unsigned(16, 7)]]);
}

const SCALAR_HELPERS: &str = r#"
    machine identity16(value: u16) -> u16
    requires 0u16 == 0u16
    ensures result == value
    { value }
    data Ordinary {}
    machine Ordinary::choose(first: u16, second: u16, third: u16) -> u16
    requires 0u16 == 0u16
    ensures result == second
    { second }
    boundary trait Producer {
        machine choose(first: u16, second: u16, third: u16) -> u16 reaches Producer;
    }
    boundary trait Host { machine finish(value: u16) reaches Host; }
"#;

fn multiple_source(first_boundary: bool, second_boundary: bool) -> String {
    let reach = if first_boundary || second_boundary {
        "Producer + Host"
    } else {
        "Host"
    };
    let first = if first_boundary {
        "Producer"
    } else {
        "Ordinary"
    };
    let second = if second_boundary {
        "Producer"
    } else {
        "Ordinary"
    };
    format!(
        r#"
        {SCALAR_HELPERS}
        data Consumer {{}}
        machine Consumer::finish(value: u16) reaches Host {{ Host::finish(value); }}
        data Main {{}}
        machine Main::main(left: u8, right: u8) reaches {reach} {{
            let prefix: u16 = left as u16;
            let first: u16 = {first}::choose(identity16(identity16(prefix)), identity16(right as u16), 11u16);
            Consumer::finish(identity16(first));
            let between: u16 = first;
            let second: u16 = {second}::choose(identity16(between), identity16(identity16(prefix)), 19u16);
            Host::finish(second);
            let third: u16 = Ordinary::choose(identity16(second), identity16(between), 23u16);
            Host::finish(prefix);
            Host::finish(first);
            Host::finish(second);
            Host::finish(third);
        }}
    "#
    )
}

#[test]
fn mixed_result_initializers_preserve_prior_locals_and_intervening_effects() {
    for first_boundary in [false, true] {
        for second_boundary in [false, true] {
            let checked = checked(&multiple_source(first_boundary, second_boundary));
            let artifact =
                encoded_locals(&checked, &["prefix", "first", "between", "second", "third"]);
            for (left, right) in [(255, 7), (19, 243)] {
                let mut observer = ObserveResults::default();
                assert_eq!(
                    execute(
                        &artifact,
                        &[unsigned(8, left), unsigned(8, right)],
                        &mut observer
                    )
                    .unwrap(),
                    TerminalExecutionResult::Unit
                );
                let mut expected = Vec::new();
                if first_boundary {
                    expected.push(vec![
                        unsigned(16, left),
                        unsigned(16, right),
                        unsigned(16, 11),
                    ]);
                }
                expected.push(vec![unsigned(16, right)]);
                if second_boundary {
                    expected.push(vec![
                        unsigned(16, right),
                        unsigned(16, left),
                        unsigned(16, 19),
                    ]);
                }
                expected.extend(
                    [left, left, right, left, right].map(|value| vec![unsigned(16, value)]),
                );
                assert_eq!(
                    observer.calls, expected,
                    "earlier results survive every later computation fragment and effect"
                );
            }
        }
    }
}

#[test]
fn later_boolean_result_operands_short_circuit_using_prior_result_values() {
    for boundary in [false, true] {
        let reach = if boundary { "Producer + Host" } else { "Host" };
        let second = if boundary { "Producer" } else { "Ordinary" };
        let source = format!(
            r#"
            machine identity(value: bool) -> bool
            requires true == true
            ensures true == true
            {{ value }}
            machine abort() -> bool crashes Abort {{ crash Abort; }}
            machine trap() -> bool crashes Trap {{ crash Trap; }}
            data Ordinary {{}}
            machine Ordinary::choose(first: bool, second: bool) -> bool
            requires true == true
            ensures true == true
            {{ second }}
            boundary trait Producer {{ machine choose(first: bool, second: bool) -> bool reaches Producer; }}
            boundary trait Host {{ machine finish(value: bool) reaches Host; }}
            data Main {{}}
            machine Main::main(input: bool, other: bool) reaches {reach}
            crashes Abort crashes Trap {{
                let prefix: bool = input;
                let first: bool = Ordinary::choose(identity(prefix), identity(prefix));
                Host::finish(first);
                let second: bool = {second}::choose(first && abort(), other || trap());
                Host::finish(second);
                Host::finish(first);
            }}
        "#
        );
        let artifact = encoded_locals(&checked(&source), &["prefix", "first", "second"]);
        for (input, other, cause) in [
            (false, true, None),
            (true, false, Some(terminal_psi::CrashCause::Abort)),
            (false, false, Some(terminal_psi::CrashCause::Trap)),
        ] {
            let mut observer = ObserveResults::default();
            let result = execute(
                &artifact,
                &[
                    TerminalScalarValue::Boolean(input),
                    TerminalScalarValue::Boolean(other),
                ],
                &mut observer,
            );
            let mut expected = vec![vec![TerminalScalarValue::Boolean(input)]];
            if let Some(cause) = cause {
                assert!(
                    matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == cause)
                );
            } else {
                assert_eq!(result.unwrap(), TerminalExecutionResult::Unit);
                if boundary {
                    expected.push(vec![
                        TerminalScalarValue::Boolean(false),
                        TerminalScalarValue::Boolean(true),
                    ]);
                }
                expected
                    .extend([true, false].map(|value| vec![TerminalScalarValue::Boolean(value)]));
            }
            assert_eq!(
                observer.calls, expected,
                "no result or later effects are established after argument failure"
            );
        }
    }
}

#[test]
fn later_initializer_crash_keeps_prior_effects_and_skips_later_arguments_and_consumers() {
    for boundary in [false, true] {
        for (first_crash, second_crash, cause) in [
            ("Abort", "Trap", terminal_psi::CrashCause::Abort),
            ("Trap", "Abort", terminal_psi::CrashCause::Trap),
        ] {
            let target = if boundary { "Producer" } else { "Ordinary" };
            let source = format!(
                r#"
                {SCALAR_HELPERS}
                machine first_crash() -> u8 crashes {first_crash} {{ crash {first_crash}; }}
                machine second_crash() -> u8 crashes {second_crash} {{ crash {second_crash}; }}
                data Main {{}}
                machine Main::main() reaches Producer + Host crashes Abort crashes Trap {{
                    let prefix: u16 = 5u16;
                    let first: u16 = Producer::choose(identity16(prefix), identity16(7u16), 11u16);
                    Host::finish(first);
                    let second: u16 = {target}::choose(first_crash() as u16, second_crash() as u16, 19u16);
                    Host::finish(second);
                    Host::finish(first);
                }}
            "#
            );
            let artifact = encoded_locals(&checked(&source), &["prefix", "first", "second"]);
            let mut observer = ObserveResults::default();
            assert!(
                matches!(execute(&artifact, &[], &mut observer), Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == cause)
            );
            assert_eq!(
                observer.calls,
                vec![
                    vec![unsigned(16, 5), unsigned(16, 7), unsigned(16, 11)],
                    vec![unsigned(16, 7)]
                ]
            );
        }
    }
}

#[test]
fn later_initializer_custody_rejects_target_coordinate_namespace_and_result_drift() {
    let checked = checked(&multiple_source(true, false));
    encoded_locals(&checked, &["prefix", "first", "between", "second", "third"]);
    let machine = main_machine(&checked);
    let state = &checked.typed.machine_states(machine)[0];
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    let locals = statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match statement {
            StatementNode::LocalData(local)
                if matches!(
                    checked
                        .typed
                        .expression_table
                        .expression(local.initial_value),
                    ExpressionNode::Call(_)
                ) =>
            {
                Some((index, local))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 3);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .unwrap();
    for (operation_index, operation) in plan.operations.iter().enumerate() {
        if !matches!(
            operation,
            CheckedUnitEffectOperationPlan::ScalarCall { .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
        ) {
            continue;
        }
        for mutation in 0..2 {
            let mut changed = checked.clone();
            let changed_plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == machine.symbol)
                .unwrap();
            let result = match &mut changed_plan.operations[operation_index] {
                CheckedUnitEffectOperationPlan::ScalarCall { result, .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } => result,
                _ => unreachable!(),
            };
            if mutation == 0 {
                result.binding_ordinal += 1;
            } else {
                result.primitive_type = typed_trees::types::PrimitiveType::Bool;
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "result binding ordinal and carrier must match the source local: mutation={mutation}"
            );
        }
    }
    let ExpressionNode::Call(second) = checked
        .typed
        .expression_table
        .expression(locals[1].1.initial_value)
    else {
        unreachable!();
    };
    let second_argument = checked
        .typed
        .expression_table
        .expression_handles(second.arguments)[0];
    let ExpressionNode::Call(nested) = checked.typed.expression_table.expression(second_argument)
    else {
        unreachable!();
    };
    let earlier_name = checked
        .typed
        .expression_table
        .expression_handles(nested.arguments)[0];
    for destination in [locals[1].1.symbol, locals[2].1.symbol] {
        for change_head in [false, true] {
            let mut changed = checked.clone();
            let ExpressionNode::Name(path) =
                changed.typed.expression_table.expression_mut(earlier_name)
            else {
                unreachable!();
            };
            path.symbol = destination;
            if change_head {
                path.head_symbol = destination;
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "second initializer cannot read its own or a later result before establishment"
            );
        }
    }
    let (between_index, between) = statements
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "between" => {
                Some((index, local))
            }
            _ => None,
        })
        .unwrap();
    let StatementNode::LocalData(prefix) = &statements[0] else {
        unreachable!();
    };
    assert_ne!(between.initial_value, prefix.initial_value);
    let mut changed = checked.clone();
    let StatementNode::LocalData(changed_between) = &mut changed
        .typed
        .statement_table
        .statements_mut(state.statement_nodes)[between_index]
    else {
        unreachable!();
    };
    changed_between.initial_value = prefix.initial_value;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
        "pure local between result calls retains its exact authored initializer"
    );
    let mut changed = checked.clone();
    let StatementNode::LocalData(changed_between) = &mut changed
        .typed
        .statement_table
        .statements_mut(state.statement_nodes)[between_index]
    else {
        unreachable!();
    };
    changed_between.symbol = prefix.symbol;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
        "pure local declaration identity cannot reuse an earlier local"
    );
    for (index, local) in &locals {
        let ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(local.initial_value)
        else {
            unreachable!();
        };
        for mutation in 0..4 {
            let mut changed = checked.clone();
            if mutation == 0 {
                let ExpressionNode::Call(changed_call) = changed
                    .typed
                    .expression_table
                    .expression_mut(local.initial_value)
                else {
                    unreachable!();
                };
                changed_call.target_symbol = symbols::SymbolHandle::invalid();
            } else if mutation == 1 {
                let other = locals
                    .iter()
                    .find(|(_, other)| other.symbol != local.symbol)
                    .unwrap()
                    .1;
                let StatementNode::LocalData(changed_local) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[*index]
                else {
                    unreachable!();
                };
                changed_local.initial_value = other.initial_value;
            } else if mutation == 2 {
                let (handle, _) = checked
                    .facts
                    .flow
                    .control
                    .calls
                    .iter()
                    .find(|(_, flow)| {
                        flow.authored_expression == local.initial_value && flow.call_ordinal == 0
                    })
                    .unwrap();
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(handle)
                    .statement_index += 1;
            } else {
                let arguments = checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments);
                changed
                    .typed
                    .expression_table
                    .set_expression_handle_at_offset(call.arguments, 0, arguments[1]);
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "initializer at statement {index}: mutation={mutation}"
            );
        }
    }
    let roots = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.machine == machine.symbol
                && root.statement_ordinal > 1
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::UnitCallArgument { .. }
                        | CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                )
        })
        .collect::<Vec<_>>();
    assert!(!roots.is_empty());
    for (handle, root) in roots {
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .roots
            .get_mut(handle)
            .statement_ordinal = 1;
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err());
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(root.root)
            .authored_root = locals[0].1.initial_value;
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err());
    }
    let bindings = &checked.facts.values.scalar_expressions;
    let later_namespaces = bindings
        .source_bindings
        .iter()
        .filter(|(_, binding)| {
            binding.state == state.symbol
                && binding.statement_ordinal > 1
                && binding.symbols.count() >= 4
        })
        .collect::<Vec<_>>();
    assert!(
        !later_namespaces.is_empty(),
        "later pure operands and result consumers retain declaration namespaces"
    );
    for (_, binding) in later_namespaces {
        let mut changed = checked.clone();
        let symbols = changed
            .facts
            .values
            .scalar_expressions
            .binding_symbols
            .span_mut(binding.symbols)
            .unwrap();
        let last = symbols.len() - 1;
        symbols.swap(0, last);
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "later initializer/consumer cannot reorder its parameter and prior-result namespace"
        );
    }
}
