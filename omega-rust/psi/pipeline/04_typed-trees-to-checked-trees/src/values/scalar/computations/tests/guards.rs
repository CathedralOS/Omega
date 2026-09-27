use super::super::CheckedScalarComputationRoot;
use super::{CheckedScalarExpressionRole, PrimitiveType, StatementNode};
use crate::values::build_checked_scalar_computation_plans;
use crate::values::scalar::computations::tests::checked_source;

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
            &checked.facts.borrow,
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
