use super::*;

mod assignments;

fn checked(argument: &str, combined: bool) -> checked_trees::CheckedTrees {
    let source = format!(
        r#"
        machine inner(input: bool) -> bool {{ input }}
        machine outer(input: bool) -> bool {{ input }}
        machine value(flag: bool, other: bool) -> bool {{
            transition flag {{
                true -> finish({argument})
                false -> finish(flag || outer(inner(flag)))
            }}
            state finish(input: bool) -> bool {{ input }}
        }}
    "#
    );
    checked_source(&source, combined)
}

fn checked_source(source: &str, combined: bool) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let mut typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    if combined {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap()
            .clone();
        let state = typed.machine_states(&machine)[0].clone();
        let statements = typed.statement_table.statements_mut(state.statement_nodes);
        let [
            StatementNode::Transition(first),
            StatementNode::Transition(second),
        ] = statements
        else {
            panic!("two authored transition arms");
        };
        first.continuation = second.target;
        typed.machine_states_mut(&machine)[0].statement_nodes =
            arena::HandleSpan::from_parts(state.statement_nodes.start(), 1);
    }
    crate::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

#[test]
fn scalar_computations_keep_nested_authored_call_ordinals() {
    let checked = checked("other && outer(inner(flag))", false);
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| root.statement_ordinal == 0)
        .expect("selected argument root");
    let CheckedScalarComputationKind::Select { when_true, .. } = plans.nodes.get(root.root).kind
    else {
        panic!("RHS remains conditional");
    };
    let CheckedScalarComputationKind::Call {
        call_ordinal: outer,
        arguments,
        source_call,
        ..
    } = plans.nodes.get(when_true).kind
    else {
        panic!("outer invocation");
    };
    let inner_handle = plans.operands.span_or_empty(arguments)[0];
    let CheckedScalarComputationKind::Call {
        call_ordinal: inner,
        ..
    } = plans.nodes.get(inner_handle).kind
    else {
        panic!("inner invocation");
    };
    assert!(
        outer < inner,
        "preorder occurrence identities are not execution order"
    );
    assert_eq!(
        checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .call_ordinal,
        outer as usize
    );
}

#[test]
fn scalar_computations_keep_combined_arm_roles_distinct() {
    let checked = checked("other && outer(inner(flag))", true);
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].state, roots[1].state);
    assert_eq!(roots[0].statement_ordinal, roots[1].statement_ordinal);
    assert_eq!(
        roots[0].role,
        CheckedScalarExpressionRole::TransitionArgument {
            argument_ordinal: 0
        }
    );
    assert_eq!(
        roots[1].role,
        CheckedScalarExpressionRole::TransitionContinuationArgument {
            argument_ordinal: 0
        }
    );
    assert_ne!(roots[0].root, roots[1].root);
}

#[test]
fn scalar_computations_do_not_require_skipped_call_occurrences() {
    for argument in [
        "false && outer(inner(flag))",
        "true || outer(inner(flag))",
        "(true == false) && outer(inner(flag))",
    ] {
        let checked = checked(argument, false);
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.statement_ordinal == 0)
            .expect("known argument root");
        assert!(
            matches!(
                plans.nodes.get(root.root).kind,
                CheckedScalarComputationKind::Value(_)
            ),
            "{argument}: {:?}",
            plans.nodes.get(root.root).kind
        );
    }
}

#[test]
fn scalar_computations_apply_boolean_templates_to_computed_operands() {
    for argument in [
        "!(other && outer(inner(flag)))",
        "(other && outer(inner(flag))) == flag",
        "(other && outer(inner(flag))) != flag",
    ] {
        let checked = checked(argument, false);
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.statement_ordinal == 0)
            .expect("computed argument root");
        assert!(matches!(
            plans.nodes.get(root.root).kind,
            CheckedScalarComputationKind::Apply { .. }
        ));
    }
}

#[test]
fn scalar_computations_keep_return_arm_roles_and_call_custody() {
    let source = r#"
        machine inner(input: bool) -> bool { input }
        machine outer(input: bool) -> bool { input }
        machine value(flag: bool, other: bool) -> bool {
            transition flag {
                true -> (other && outer(inner(flag)))
                false -> (flag || outer(inner(other)))
            }
        }
    "#;
    for combined in [false, true] {
        let checked = checked_source(source, combined);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        assert_eq!(checked.machine_states(machine).len(), 1);
        let state = &checked.machine_states(machine)[0];
        let plans = &checked.facts.values.scalar_computations;
        let roots = plans
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| root.state == state.symbol)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2, "combined={combined}");
        assert_eq!(roots[0].role, CheckedScalarExpressionRole::Return);
        assert_eq!(roots[0].statement_ordinal, 0);
        assert_eq!(roots[1].statement_ordinal, u32::from(!combined));
        assert_eq!(
            roots[1].role,
            if combined {
                CheckedScalarExpressionRole::ContinuationReturn
            } else {
                CheckedScalarExpressionRole::Return
            }
        );
        for (index, root) in roots.iter().enumerate() {
            let CheckedScalarComputationKind::Select {
                when_true,
                when_false,
                ..
            } = plans.nodes.get(root.root).kind
            else {
                panic!("returned RHS remains conditional");
            };
            let selected = if index == 0 { when_true } else { when_false };
            let CheckedScalarComputationKind::Call {
                source_call,
                call_ordinal,
                arguments,
                ..
            } = plans.nodes.get(selected).kind
            else {
                panic!("selected outer return invocation");
            };
            let fact = checked.facts.flow.control.calls.get(source_call);
            assert_eq!(fact.statement_index, root.statement_ordinal as usize);
            assert_eq!(fact.call_ordinal, call_ordinal as usize);
            let CheckedScalarComputationKind::Call {
                call_ordinal: inner_ordinal,
                ..
            } = plans
                .nodes
                .get(plans.operands.span_or_empty(arguments)[0])
                .kind
            else {
                panic!("inner argument invocation");
            };
            assert!(call_ordinal < inner_ordinal);
        }
    }
}

#[test]
fn scalar_computations_keep_trailing_return_local_namespace() {
    let checked = checked_source(
        r#"
        machine inner(input: bool) -> bool { input }
        machine value(flag: bool, other: bool) -> bool {
            let saved: bool = other;
            flag && inner(saved)
        }
        "#,
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    let root = roots[0];
    assert_eq!(root.statement_ordinal, 1);
    assert_eq!(root.role, CheckedScalarExpressionRole::Return);
    let CheckedScalarComputationKind::Select { when_true, .. } = plans.nodes.get(root.root).kind
    else {
        panic!("conditional return");
    };
    let CheckedScalarComputationKind::Call { arguments, .. } = plans.nodes.get(when_true).kind
    else {
        panic!("selected call");
    };
    assert_eq!(
        plans
            .nodes
            .get(plans.operands.span_or_empty(arguments)[0])
            .kind,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::Local { position: 2 },
        )))
    );
}

#[test]
fn scalar_computations_do_not_duplicate_pure_returns() {
    let checked = checked_source("machine value(flag: bool) -> bool { !flag }", false);
    assert_eq!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .count(),
        0
    );
    let state = checked.machine_states(&checked.machines()[0])[0].symbol;
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(state, 0, CheckedScalarExpressionRole::Return)
            .is_some()
    );
}

#[test]
fn scalar_computations_do_not_require_skipped_return_call_occurrences() {
    for expression in ["false && inner(flag)", "true || inner(flag)"] {
        let checked = checked_source(
            &format!(
                "machine inner(input: bool) -> bool {{ input }}
                 machine value(flag: bool) -> bool {{ {expression} }}"
            ),
            false,
        );
        let plans = &checked.facts.values.scalar_computations;
        let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].role, CheckedScalarExpressionRole::Return);
        assert!(matches!(
            plans.nodes.get(roots[0].root).kind,
            CheckedScalarComputationKind::Value(_)
        ));
    }
}

#[test]
fn scalar_computations_refuse_missing_or_duplicate_return_call_custody() {
    let checked = checked_source(
        "machine inner(input: bool) -> bool { input }
         machine value(flag: bool) -> bool { flag && inner(flag) }",
        false,
    );
    let root = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .next()
        .expect("computed return")
        .1;
    let flow_state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(handle, state)| (state.state_symbol == root.state).then_some(handle))
        .unwrap();
    for duplicate in [false, true] {
        let mut flow = checked.facts.flow.clone();
        let state = flow.control.states.get(flow_state);
        let [call] = flow.control.calls.span_or_empty(state.calls) else {
            panic!("one authored return call");
        };
        let calls = if duplicate {
            let call = call.clone();
            flow.control.calls.insert_many([call.clone(), call])
        } else {
            arena::HandleSpan::default()
        };
        flow.control.states.get_mut(flow_state).calls = calls;
        let plans = build_checked_scalar_computation_plans(
            &checked.typed,
            &checked.facts.operators,
            &flow,
            &checked.facts.proof,
            &checked.facts.values.scalar_expressions,
            &[],
        );
        assert!(
            plans
                .roots
                .iter()
                .all(|(_, candidate)| candidate.state != root.state),
            "duplicate={duplicate}"
        );
    }
}

#[test]
fn scalar_computations_keep_initializer_roles_and_prior_binding_positions() {
    let checked = checked_source(
        r#"
        machine inner(input: bool) -> bool { input }
        machine value(flag: bool) -> bool {
            let previous: bool = flag;
            let mut current: bool = inner(previous);
            let selected: bool = previous && inner(current);
            selected
        }
        "#,
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| root.state == state.symbol)
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].statement_ordinal, 1);
    assert_eq!(
        roots[0].role,
        CheckedScalarExpressionRole::StorageInitializer
    );
    assert_eq!(roots[1].statement_ordinal, 2);
    assert_eq!(
        roots[1].role,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 }
    );
    let CheckedScalarComputationKind::Call { arguments, .. } = plans.nodes.get(roots[0].root).kind
    else {
        panic!("storage initializer call");
    };
    let argument = plans.operands.span_or_empty(arguments)[0];
    assert_eq!(
        plans.nodes.get(argument).kind,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::Local { position: 1 },
        )))
    );
    for root in &roots {
        let StatementNode::LocalData(local) = &checked
            .statement_table
            .statements(state.statement_nodes)[root.statement_ordinal as usize]
        else {
            panic!("initializer source");
        };
        assert_eq!(
            plans.nodes.get(root.root).authored_root,
            local.initial_value
        );
    }
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .find(|graph| graph.machine == machine.symbol)
        .unwrap();
    assert_eq!(
        graph.states[0]
            .bindings
            .iter()
            .map(|binding| binding.value.clone())
            .collect::<Vec<_>>(),
        vec![
            checked_trees::CheckedScalarBindingValue::Expression,
            checked_trees::CheckedScalarBindingValue::Computation,
            checked_trees::CheckedScalarBindingValue::Computation
        ]
    );
}

#[test]
fn scalar_computations_do_not_duplicate_pure_or_direct_call_initializers() {
    let checked = checked_source(
        r#"
        machine inner(input: bool) -> bool { input }
        machine constant() -> bool { true }
        machine value(flag: bool) -> bool {
            let first: bool = !flag;
            let second: bool = inner(first);
            let third: bool = constant();
            second && third
        }
        "#,
        false,
    );
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .next()
            .is_none()
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .find(|graph| graph.machine == machine.symbol)
        .unwrap();
    assert_eq!(
        graph.states[0].bindings[0].value,
        checked_trees::CheckedScalarBindingValue::Expression
    );
    for binding in &graph.states[0].bindings[1..] {
        assert!(matches!(
            binding.value,
            checked_trees::CheckedScalarBindingValue::DirectCall { .. }
        ));
    }
}

#[test]
fn scalar_computations_retain_nested_initializer_invocations() {
    let checked = checked_source(
        r#"
        machine inner(input: bool) -> bool { input }
        machine outer(input: bool) -> bool { input }
        machine value(flag: bool) -> bool {
            let result: bool = outer(inner(flag));
            result
        }
        "#,
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    assert_eq!(
        roots[0].role,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }
    );
    let CheckedScalarComputationKind::Call { arguments, .. } = plans.nodes.get(roots[0].root).kind
    else {
        panic!("outer initializer invocation");
    };
    assert!(matches!(
        plans
            .nodes
            .get(plans.operands.span_or_empty(arguments)[0])
            .kind,
        CheckedScalarComputationKind::Call { .. }
    ));
}

#[test]
fn scalar_computations_retain_integer_initializer_applications() {
    let checked = checked_source(
        r#"
        machine identity(input: u8 in Wrapping) -> u8 in Wrapping { input }
        machine value(input: u8 in Wrapping) -> u8 in Wrapping {
            let result: u8 in Wrapping = identity(input) + 1u8;
            result
        }
        "#,
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots.iter().map(|(_, root)| root).collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    assert_eq!(
        roots[0].role,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }
    );
    let CheckedScalarComputationKind::Apply { operands, .. } = plans.nodes.get(roots[0].root).kind
    else {
        panic!("initializer arithmetic application");
    };
    let operands = plans.operands.span_or_empty(operands);
    assert_eq!(operands.len(), 2);
    assert!(matches!(
        plans.nodes.get(operands[0]).kind,
        CheckedScalarComputationKind::Call { .. }
    ));
    assert!(matches!(
        plans.nodes.get(operands[1]).kind,
        CheckedScalarComputationKind::Value(_)
    ));
}
