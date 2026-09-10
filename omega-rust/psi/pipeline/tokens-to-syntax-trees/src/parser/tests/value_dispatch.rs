use source_files_to_tokens::Lexer;
use syntax_trees::expression::{ExpressionNode, MatchPattern};

#[test]
fn value_match_retains_subject_once_ordered_duplicate_arms_and_spans() {
    for separator in [" ", ", "] {
        let source = format!(
            "match read() {{ 0 -> first(){separator}0 -> second(){separator}_ -> last() }}"
        );
        let source_id = source::SourceId::default();
        let tokens = Lexer::new(&source).tokenize().expect("tokens");
        let mut trees = syntax_trees::SyntaxTrees::new(source_id);
        let (expression, rest) = crate::parser::expression::parse_expression_handle(
            &mut trees,
            crate::parser::input::Input::new(source_id, &tokens),
        )
        .expect("ordered value match");
        assert!(rest.tokens.is_empty());
        let ExpressionNode::Match(dispatch) = trees.expressions.expression(expression) else {
            panic!("match must remain dispatch, not arithmetic");
        };
        assert!(matches!(
            trees.expressions.expression(dispatch.subject),
            ExpressionNode::Call(_)
        ));
        let arms = trees.expressions.match_arms(dispatch.arms);
        assert_eq!(arms.len(), 3);
        assert!(matches!(arms[0].pattern, MatchPattern::Value(_)));
        assert!(matches!(arms[1].pattern, MatchPattern::Value(_)));
        assert!(matches!(arms[2].pattern, MatchPattern::Wildcard));
        for (arm, target) in arms.iter().zip(["first", "second", "last"]) {
            let ExpressionNode::Call(call) = trees.expressions.expression(arm.value) else {
                panic!("arm call");
            };
            assert_eq!(call.target.as_str(), target);
            assert!(arm.source_span.span.end > arm.source_span.span.start);
        }
        assert_eq!(
            trees
                .expressions
                .iter_expressions()
                .filter(|(_, node)| matches!(node, ExpressionNode::Call(_)))
                .count(),
            4
        );
        assert!(
            !trees
                .expressions
                .iter_expressions()
                .any(|(_, node)| matches!(node, ExpressionNode::Binary(_)))
        );
    }
}

#[test]
fn machine_tail_match_is_a_value_dispatch_not_source_transitions() {
    let tokens =
        Lexer::new("machine choose(value: i64) -> i64 { match value { 0 -> 7, _ -> value } }")
            .tokenize()
            .expect("tokens");
    let trees = super::parse_syntax_trees(&tokens).expect("tail value dispatch");
    assert_eq!(
        trees
            .expressions
            .iter_expressions()
            .filter(|(_, node)| matches!(node, ExpressionNode::Match(_)))
            .count(),
        1
    );
}

#[test]
fn nested_initializer_match_retains_expression_local_dispatch() {
    let tokens = Lexer::new("machine choose(value: i64) -> i64 { let result: i64 = match value { 0 -> match value { _ -> 7 }, _ -> value }; result }")
        .tokenize().expect("tokens");
    let trees = super::parse_syntax_trees(&tokens).expect("nested initializer dispatch");
    let machine = trees
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let state = trees
        .items
        .state(trees.items.state_handles(machine.states)[0]);
    let local = trees
        .items
        .statements(state.statements)
        .iter()
        .find_map(|statement| match trees.statements.statement(*statement) {
            syntax_trees::statement::StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .expect("initializer owner");
    let ExpressionNode::Match(outer) = trees.expressions.expression(local.initial_value) else {
        panic!("initializer retains outer dispatch");
    };
    let arms = trees.expressions.match_arms(outer.arms);
    assert_eq!(arms.len(), 2);
    let ExpressionNode::Match(inner) = trees.expressions.expression(arms[0].value) else {
        panic!("selected arm retains nested dispatch");
    };
    assert_eq!(trees.expressions.match_arms(inner.arms).len(), 1);
    assert!(matches!(
        trees.expressions.expression(arms[1].value),
        ExpressionNode::Name(_)
    ));
}

#[test]
fn value_match_rejects_empty_and_unimplemented_binding_patterns() {
    for source in [
        "match value {}",
        "match value { Record { field } -> field, _ -> 0 }",
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokens");
        let mut trees = syntax_trees::SyntaxTrees::new(source::SourceId::default());
        assert!(
            crate::parser::expression::parse_expression_handle(
                &mut trees,
                crate::parser::input::Input::new(source::SourceId::default(), &tokens),
            )
            .is_err()
        );
    }
}
