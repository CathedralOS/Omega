use super::*;
use checked_trees::CheckedScalarExpressionRole;
use typed_trees::{expression::ExpressionNode, statement::TransitionGuardNode};

#[test]
fn pure_guard_and_direct_call_arguments_keep_exact_source_custody() {
    let source = r#"
        machine identity(first: bool, second: bool) -> bool { first || second }
        machine value(flag: bool, other: bool) -> bool {
            let mut current: bool = flag;
            let saved: bool = current;
            current = other;
            let called: bool = identity(saved, !current);
            transition called && saved { true -> true false -> false }
        }
    "#;
    let checked = lower_typed_trees(typed_trees(source)).unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let parameters = checked.state_parameters(state);
    let statements = checked.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(saved) = &statements[1] else {
        panic!("saved immutable local");
    };
    let StatementNode::LocalData(called) = &statements[3] else {
        panic!("direct call binding");
    };
    let ExpressionNode::Call(call) = checked.expression_table.expression(called.initial_value)
    else {
        panic!("authored direct call");
    };
    let plans = &checked.facts.values.scalar_expressions;
    for (ordinal, argument) in checked
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        let (binding, _) = plans
            .bound_expression_at(
                state.symbol,
                3,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal: 1,
                    argument_ordinal: u32::try_from(ordinal).unwrap(),
                },
            )
            .expect("one exact direct argument binding");
        assert_eq!(binding.expression, *argument);
        assert!(!binding.destination.is_valid());
        assert_eq!(
            plans.binding_symbols.span_or_empty(binding.symbols),
            &[parameters[0].symbol, parameters[1].symbol, saved.symbol]
        );
    }
    let StatementNode::Transition(transition) = &statements[4] else {
        panic!("authored guarded return");
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        panic!("authored guard");
    };
    let (binding, _) = plans
        .bound_expression_at(state.symbol, 4, CheckedScalarExpressionRole::Guard)
        .expect("one exact guard binding");
    assert_eq!(binding.expression, guard);
    assert!(!binding.destination.is_valid());
    assert_eq!(
        plans.binding_symbols.span_or_empty(binding.symbols),
        &[
            parameters[0].symbol,
            parameters[1].symbol,
            saved.symbol,
            called.symbol
        ]
    );
    assert!(
        plans
            .bound_expression_at(
                state.symbol,
                3,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 }
            )
            .is_none(),
        "direct call arguments do not manufacture a pure initializer plan"
    );
}
