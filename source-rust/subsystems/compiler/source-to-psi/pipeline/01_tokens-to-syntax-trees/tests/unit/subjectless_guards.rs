//! What the parser hands name resolution for an arm of a `transition` without
//! a subject.
//!
//! Without a subject each arm is one guard read as an ordinary expression
//! (`wiki/spec/language/patterns.md#bare-names-in-dispatch-arms`: "In a
//! dispatch without a subject, an arm condition such as `ready -> 1` reads the
//! existing value of `ready`"). A guard may start with a parenthesis, as in
//! `(a + b) > a -> done()`, and must stay that one comparison rather than be
//! read as a tuple pattern followed by stray tokens.

use syntax_trees::expression::{BinaryOperator, ExpressionNode};
use syntax_trees::item::Item;
use syntax_trees::statement::{StatementNode, TransitionGuardNode};

fn guards(source: &str) -> Vec<String> {
    let syntax = crate::tests::parse_source(source);
    let Item::Machine(machine) = syntax.root_items().next().expect("one machine") else {
        panic!("a machine root");
    };
    let state = syntax.items.state_handles(machine.states)[0];
    let mut found = Vec::new();
    for statement in syntax
        .items
        .statements(syntax.items.state(state).statements)
    {
        let StatementNode::Dispatch(handle) = syntax.statements.statement(*statement) else {
            continue;
        };
        for arm in syntax.statements.dispatch(*handle).arms.iter() {
            assert!(arm.patterns.is_empty(), "a subjectless arm has no patterns");
            found.push(match arm.guard {
                TransitionGuardNode::Always => "_".to_owned(),
                TransitionGuardNode::When(guard) => {
                    let span = syntax.expressions.source_span(guard).span;
                    let operator = match syntax.expressions.expression(guard) {
                        ExpressionNode::Binary(binary)
                            if binary.operator == BinaryOperator::Greater =>
                        {
                            ">"
                        }
                        _ => "other",
                    };
                    format!("{operator} at {}", &source[span.start..span.end])
                }
            });
        }
    }
    found
}

#[test]
fn a_subjectless_guard_may_start_with_a_parenthesis() {
    let source = "machine main() {\n    transition { (a + b) > a -> done(), _ -> done(), }\n    machine done() {}\n}";
    assert_eq!(guards(source), ["> at >", "_"]);
}

#[test]
fn a_parenthesized_list_in_a_subjectless_arm_is_still_a_refused_tuple() {
    let source = "machine main() {\n    transition { (a, b) -> done(), }\n    machine done() {}\n}";
    let error = crate::tests::parse_error(source);
    assert!(
        error
            .message
            .contains("anonymous transition blocks do not support tuple patterns"),
        "{}",
        error.message
    );
}
