use super::qualification_expression_contains;
use crate::test_support::*;

mod qualifications;

#[test]
fn qualification_expression_search_includes_every_value_dispatch_child() {
    let mut program = CheckedTrees::default();
    let expressions = &mut program.typed.expression_table;
    let subject = expressions.insert(ExpressionNode::Boolean(true));
    let pattern = expressions.insert(ExpressionNode::Boolean(false));
    let first = expressions.insert(ExpressionNode::Boolean(false));
    let last = expressions.insert(ExpressionNode::Boolean(true));
    let arms = expressions.insert_match_arms([
        typed_trees::expression::TableMatchArm {
            pattern: typed_trees::expression::MatchPattern::Value(pattern),
            value: first,
            source_span: Default::default(),
        },
        typed_trees::expression::TableMatchArm {
            pattern: typed_trees::expression::MatchPattern::Wildcard,
            value: last,
            source_span: Default::default(),
        },
    ]);
    let dispatch = expressions.insert(ExpressionNode::Match(
        typed_trees::expression::TableMatchExpression { subject, arms },
    ));
    for child in [subject, pattern, first, last] {
        assert!(qualification_expression_contains(
            &program,
            dispatch,
            child,
            &mut Vec::new()
        ));
    }
}
