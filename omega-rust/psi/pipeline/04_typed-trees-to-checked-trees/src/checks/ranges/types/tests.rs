use super::{ExpressionHandle, ExpressionNode, SymbolHandle};
use crate::checks::ranges::expression_enforced_declared_range;
use crate::tests::front_end::typed_program;
use typed_trees::typed_trees::StaticRequirementDispatch;

fn fixture(source: &str) -> (typed_trees::TypedTrees, ExpressionHandle) {
    let program = typed_program(source);
    let call = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Call(call) if call.target.as_str() == "cut" => Some(handle),
            _ => None,
        })
        .expect("cut call");
    (program, call)
}

fn range(program: &typed_trees::TypedTrees, expression: ExpressionHandle) -> Option<(i64, i64)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| matches!(machine.name.as_str(), "inspect" | "Caller::inspect"))
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    expression_enforced_declared_range(program, machine, state, expression)
}

#[test]
fn call_return_range_follows_only_the_selected_state_identity() {
    let (mut program, expression) = fixture(
        "machine cut() -> u64 [1..=1] { 1 }
        machine other() -> u64 [5..=5] { 5 }
        machine inspect() -> u64 { cut() }",
    );
    assert_eq!(range(&program, expression), Some((1, 1)));
    let other = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "other")
        .expect("other machine");
    let other_state = program.machine_states(other)[0].symbol;
    let other_machine = other.symbol;
    for (target, expected) in [
        (other_state, Some((5, 5))),
        (SymbolHandle::invalid(), None),
        (other_machine, None),
    ] {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("call kind")
        };
        call.target_symbol = target;
        assert_eq!(range(&program, expression), expected);
    }
}

#[test]
fn private_realization_range_does_not_become_a_public_requirement_fact() {
    let (mut program, expression) = fixture(
        "machine cut() -> u64 [1..=1] { 1 }
        machine inspect() -> u64 { cut() }",
    );
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
        panic!("call kind")
    };
    call.static_requirement_dispatch = Some(StaticRequirementDispatch::default());
    assert_eq!(range(&program, expression), None);
}
