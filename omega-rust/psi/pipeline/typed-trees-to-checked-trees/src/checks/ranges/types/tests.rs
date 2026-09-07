use super::*;
use typed_trees::typed_trees::StaticRequirementDispatch;

fn fixture(source: &str) -> (typed_trees::TypedTrees, ExpressionHandle) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("symbols");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("types");
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

#[test]
fn dependent_callee_result_does_not_use_the_callers_same_named_field() {
    let (program, expression) = fixture(
        "data Selector { limit: u64 [0..=10]; }
        data Caller { limit: u64 [0..=1]; selector: Selector; }
        machine Selector::cut(&self) -> u64 [0..=self.limit] { 0 }
        machine Caller::inspect(&self) -> u64 { self.selector.cut() }",
    );
    assert_eq!(range(&program, expression), None);
}

#[test]
fn permissive_return_range_shells_are_not_enforced_intervals() {
    for policy in ["Wrapping", "Saturating"] {
        let (program, expression) = fixture(&format!(
            "machine cut() -> u64 [1..=1] in {policy} {{ 1 }}
             machine inspect() -> u64 {{ cut() }}"
        ));
        assert_eq!(range(&program, expression), None, "{policy}");
    }
}
