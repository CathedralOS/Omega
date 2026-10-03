use crate::resolution::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::statement::{StatementNode, TransitionGuardNode};
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolved(source: &str) -> SymbolResolvedTrees {
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
    resolve(ResolutionRequest::new(syntax)).unwrap()
}

fn value_statements(program: &SymbolResolvedTrees) -> &[StatementNode] {
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_state(program.machine_state_handles(machine.states)[0]);
    program
        .tables
        .bodies
        .statements
        .statements(entry.statement_nodes)
}

fn single_boolean_guard_subject(program: &SymbolResolvedTrees) -> ExpressionHandle {
    let statements = value_statements(program);
    assert_eq!(
        statements.len(),
        2,
        "guard evaluation must not create enclosing bindings"
    );
    let StatementNode::Transition(first) = &statements[0] else {
        panic!("first authored arm");
    };
    let TransitionGuardNode::When(guard) = first.guard else {
        panic!("first guard");
    };
    let ExpressionNode::Binary(comparison) = program.tables.bodies.expressions.expression(guard)
    else {
        panic!("Boolean subject pattern comparison");
    };
    let StatementNode::Transition(last) = &statements[1] else {
        panic!("second authored arm");
    };
    assert!(matches!(last.guard, TransitionGuardNode::Always));
    comparison.left
}

fn guard_call_targets(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    calls: &mut Vec<String>,
) {
    let expressions = &program.tables.bodies.expressions;
    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            guard_call_targets(program, binary.left, calls);
            guard_call_targets(program, binary.right, calls);
        }
        ExpressionNode::Cast(cast) => guard_call_targets(program, cast.value, calls),
        ExpressionNode::Call(call) => {
            assert!(call.target_symbol.is_valid());
            for argument in expressions.expression_handles(call.arguments) {
                guard_call_targets(program, *argument, calls);
            }
            calls.push(call.target.as_str().to_owned());
        }
        ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
        _ => panic!("unexpected test guard operand"),
    }
}

#[test]
fn guard_call_casts_retain_authored_left_to_right_operands() {
    for (subject, expected) in [
        ("first() < (second() as u16)", ["first", "second"]),
        ("(second() as u16) < first()", ["second", "first"]),
        ("(first() + 1u16) < second()", ["first", "second"]),
    ] {
        let program = resolved(&format!(
            "machine first() -> u16 {{ 1u16 }}
             machine second() -> u16 {{ 2u16 }}
             machine value() -> bool {{
                 transition {subject} {{ true -> true false -> false }}
             }}"
        ));
        let root = single_boolean_guard_subject(&program);
        let mut calls = Vec::new();
        guard_call_targets(&program, root, &mut calls);
        assert_eq!(calls, expected, "{subject}");
    }
}

#[test]
fn guard_right_call_does_not_hoist_before_a_prior_division() {
    let program = resolved(
        "machine second() -> u16 { 2u16 }
         machine value(input: u16, divisor: u16) -> bool {
             transition (input / divisor) < second() { true -> true false -> false }
         }",
    );
    let root = single_boolean_guard_subject(&program);
    let expressions = &program.tables.bodies.expressions;
    let ExpressionNode::Binary(comparison) = expressions.expression(root) else {
        panic!("authored comparison");
    };
    assert!(
        matches!(expressions.expression(comparison.left), ExpressionNode::Binary(binary)
        if binary.operator == symbol_resolved_trees::expression::BinaryOperator::Divide)
    );
    assert!(matches!(
        expressions.expression(comparison.right),
        ExpressionNode::Call(_)
    ));
}

#[test]
fn numeric_multiway_match_keeps_one_shared_call_binding() {
    let program = resolved(
        "machine roll() -> u8 { 2u8 }
         machine value() -> u8 {
             transition roll() { 1u8 -> 1u8 2u8 -> 2u8 _ -> 3u8 }
         }",
    );
    let statements = value_statements(&program);
    assert_eq!(statements.len(), 4);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("shared numeric subject binding");
    };
    assert!(matches!(
        program
            .tables
            .bodies
            .expressions
            .expression(local.initial_value),
        ExpressionNode::Call(_)
    ));
    assert!(
        statements[1..]
            .iter()
            .all(|statement| matches!(statement, StatementNode::Transition(_)))
    );
}
