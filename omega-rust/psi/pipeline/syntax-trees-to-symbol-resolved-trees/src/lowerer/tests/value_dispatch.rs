use source_files_to_tokens::Lexer;
use symbol_resolved_trees::expression::{ExpressionNode, MatchPattern};

#[test]
fn match_resolves_subject_patterns_and_every_arm_in_machine_scope() {
    let source = "machine helper(value: i64) -> i64 { value } machine choose(value: i64, alternative: i64) -> i64 { match value { alternative -> helper(value), _ -> helper(alternative) } }";
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let program = super::lower_syntax_trees(&syntax).expect("resolved dispatch");
    let expressions = &program.tables.bodies.expressions;
    let dispatch = expressions
        .iter_expressions()
        .find_map(|(_, node)| match node {
            ExpressionNode::Match(dispatch) => Some(dispatch),
            _ => None,
        })
        .expect("retained dispatch");
    let ExpressionNode::Name(subject) = expressions.expression(dispatch.subject) else {
        panic!("subject");
    };
    assert!(subject.symbol.is_valid());
    let arms = expressions.match_arms(dispatch.arms);
    assert_eq!(arms.len(), 2);
    let MatchPattern::Value(pattern) = arms[0].pattern else {
        panic!("value pattern");
    };
    let ExpressionNode::Name(pattern) = expressions.expression(pattern) else {
        panic!("pattern name");
    };
    assert!(pattern.symbol.is_valid());
    assert_ne!(subject.symbol, pattern.symbol);
    let mut selected_target = None;
    for arm in arms {
        let ExpressionNode::Call(call) = expressions.expression(arm.value) else {
            panic!("arm call");
        };
        assert!(call.target_symbol.is_valid());
        if let Some(selected_target) = selected_target {
            assert_eq!(call.target_symbol, selected_target);
        }
        selected_target = Some(call.target_symbol);
        for argument in expressions.expression_handles(call.arguments) {
            let ExpressionNode::Name(argument) = expressions.expression(*argument) else {
                panic!("argument");
            };
            assert!(argument.symbol.is_valid());
        }
    }
}
