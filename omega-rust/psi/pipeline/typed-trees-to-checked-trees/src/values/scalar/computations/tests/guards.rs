use super::*;

fn guard_program(expression: &str) -> checked_trees::CheckedTrees {
    checked_source(
        &format!(
            "machine inner(input: bool) -> bool {{ input }}
         machine outer(input: bool) -> bool {{ input }}
         machine value(flag: bool, other: bool) -> bool {{
             transition {expression} {{ true -> true false -> false }}
         }}"
        ),
        false,
    )
}

fn guard_root(checked: &checked_trees::CheckedTrees) -> &CheckedScalarComputationRoot {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let root = checked
        .facts
        .values
        .scalar_computations
        .root_at(state.symbol, 0, CheckedScalarExpressionRole::Guard)
        .expect("guard root");
    let StatementNode::Transition(transition) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("authored transition");
    };
    let typed_trees::statement::TransitionGuardNode::When(expression) = transition.guard else {
        panic!("authored guard");
    };
    let node = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(root.root);
    assert_eq!(node.authored_root, expression);
    assert_eq!(node.primitive_type, PrimitiveType::Bool);
    root
}

fn guard_nodes(checked: &checked_trees::CheckedTrees) -> Vec<&CheckedScalarComputation> {
    let plans = &checked.facts.values.scalar_computations;
    let mut pending = vec![guard_root(checked).root];
    let mut nodes = Vec::new();
    while let Some(handle) = pending.pop() {
        let node = plans.nodes.get(handle);
        match &node.kind {
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
            } => {
                pending.extend([*condition, *when_true, *when_false]);
            }
            CheckedScalarComputationKind::Call { arguments, .. } => {
                pending.extend(plans.operands.span_or_empty(*arguments));
            }
            CheckedScalarComputationKind::Apply { operands, .. } => {
                pending.extend(plans.operands.span_or_empty(*operands));
            }
            CheckedScalarComputationKind::Value(_) => {}
        }
        nodes.push(node);
    }
    nodes
}

#[test]
fn scalar_computations_keep_short_circuit_guard_nodes_and_nested_call_custody() {
    for expression in ["other && outer(inner(flag))", "other || outer(inner(flag))"] {
        let checked = guard_program(expression);
        let root = guard_root(&checked);
        let nodes = guard_nodes(&checked);
        assert!(
            nodes
                .iter()
                .any(|node| matches!(node.kind, CheckedScalarComputationKind::Select { .. }))
        );
        let mut ordinals = Vec::new();
        for node in &nodes {
            if let CheckedScalarComputationKind::Call {
                source_call,
                call_ordinal,
                ..
            } = node.kind
            {
                let call = checked.facts.flow.control.calls.get(source_call);
                assert_eq!(call.statement_index, root.statement_ordinal as usize);
                assert_eq!(call.call_ordinal, call_ordinal as usize);
                ordinals.push(call_ordinal);
            }
        }
        ordinals.sort_unstable();
        ordinals.dedup();
        assert_eq!(ordinals.len(), 2);
    }
}

#[test]
fn scalar_computations_keep_numeric_guard_applications_and_casts() {
    for (expression, call_count) in [
        ("identity(input) < identity(input)", 2),
        ("(identity(input) as u16) == (input as u16)", 1),
        ("identity(input) + 1u8 > input", 1),
    ] {
        let checked = checked_source(
            &format!(
                "machine identity(input: u8 in Wrapping) -> u8 in Wrapping {{ input }}
             machine value(input: u8 in Wrapping) -> bool {{
                 transition {expression} {{ true -> true false -> false }}
             }}"
            ),
            false,
        );
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        let statements = checked
            .statement_table
            .statements(checked.machine_states(machine)[0].statement_nodes);
        assert!(
            matches!(statements.first(), Some(StatementNode::Transition(_))),
            "{expression}: guard calls must not become preceding local initializers: {statements:?}"
        );
        let nodes = guard_nodes(&checked);
        assert!(
            nodes
                .iter()
                .any(|node| matches!(node.kind, CheckedScalarComputationKind::Apply { .. }))
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node.kind, CheckedScalarComputationKind::Call { .. }))
                .count(),
            call_count,
            "{expression}"
        );
    }
}

#[test]
fn scalar_computations_preserve_existing_lone_call_guard_binding() {
    let checked = checked_source(
        "machine identity(input: u8) -> u8 { input }
         machine value(input: u8) -> bool {
             transition identity(input) < input { true -> true false -> false }
         }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let statements = checked.statement_table.statements(state.statement_nodes);
    assert!(matches!(
        statements.first(),
        Some(StatementNode::LocalData(_))
    ));
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
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, 1, CheckedScalarExpressionRole::Guard,)
            .is_some()
    );
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .unwrap();
    assert!(matches!(
        graph.states[0].bindings[0].value,
        checked_trees::CheckedScalarBindingValue::DirectCall { .. }
    ));
}

#[test]
fn scalar_computations_do_not_duplicate_pure_guard_plans() {
    let checked = guard_program("flag && other");
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
    let state = &checked.machine_states(machine)[0];
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, 0, CheckedScalarExpressionRole::Guard,)
            .is_some()
    );
}

#[test]
fn scalar_computations_refuse_missing_or_duplicate_guard_call_custody() {
    let checked = guard_program("outer(flag)");
    let root = guard_root(&checked);
    let state_handle = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(handle, state)| (state.state_symbol == root.state).then_some(handle))
        .unwrap();
    for duplicate in [false, true] {
        let mut flow = checked.facts.flow.clone();
        let source_calls = flow
            .control
            .calls
            .span_or_empty(flow.control.states.get(state_handle).calls)
            .to_vec();
        assert!(!source_calls.is_empty());
        let calls = if duplicate {
            flow.control
                .calls
                .insert_many(source_calls.iter().chain(&source_calls).cloned())
        } else {
            arena::HandleSpan::default()
        };
        flow.control.states.get_mut(state_handle).calls = calls;
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
                .root_at(root.state, 0, CheckedScalarExpressionRole::Guard)
                .is_none(),
            "duplicate={duplicate}"
        );
    }
}
