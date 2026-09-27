use super::{
    CheckedScalarComputationKind, CheckedScalarExpressionRole, ExpressionNode, PrimitiveType,
    StatementNode,
};
use crate::tests::front_end::typed_program;
use crate::values::build_checked_scalar_computation_plans;

mod array_arguments;
mod assignments;
mod borrowed_arguments;
mod cases;
mod dispatch;
mod guards;
mod qualifications;
mod selected_comparisons;
mod structural_fields;
mod structural_values;

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
    let mut typed = typed_program(source);
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
    crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
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
            &checked.facts.borrow,
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
fn scalar_computations_retain_each_call_initializer_without_duplicating_pure_values() {
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
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .collect::<Vec<_>>();
    assert_eq!(
        roots.len(),
        2,
        "only the two call initializers need computations"
    );
    let state = &checked.machine_states(machine)[0];
    for (root, statement_ordinal) in roots.iter().zip([1, 2]) {
        assert_eq!(root.machine, machine.symbol);
        assert_eq!(root.state, state.symbol);
        assert_eq!(root.statement_ordinal, statement_ordinal);
        assert_eq!(
            root.role,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal
            }
        );
        assert!(matches!(
            computations.nodes.get(root.root).kind,
            CheckedScalarComputationKind::Call {
                call_ordinal: 0,
                ..
            }
        ));
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
        graph.states[0].bindings[0].value,
        checked_trees::CheckedScalarBindingValue::Expression
    );
    for binding in &graph.states[0].bindings[1..] {
        assert!(matches!(
            binding.value,
            checked_trees::CheckedScalarBindingValue::Computation
        ));
    }
}
