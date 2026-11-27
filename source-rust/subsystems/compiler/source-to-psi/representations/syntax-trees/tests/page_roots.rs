use numeric_literals::IntegerLiteral;
use source_text::SourceId;
use syntax_trees::{SyntaxTrees, expression::ExpressionNode};

#[test]
fn adopting_a_page_preserves_its_constant_extent_roots() {
    let mut forest = SyntaxTrees::new(SourceId(1));
    let mut page = SyntaxTrees::new_with_generation(SourceId(2), 2);
    let expression = page
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    page.roots.constant_extent_probes.push((expression, true));
    forest.adopt_page(page);
    assert_eq!(forest.roots.constant_extent_probes, [(expression, true)]);
    assert!(matches!(
        forest.expressions.expression(expression),
        ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
    ));
    let mut copied = SyntaxTrees::new(SourceId(1));
    copied.extend_from(&forest);
    let &[(copied_expression, is_public)] = copied.roots.constant_extent_probes.as_slice() else {
        panic!("copied forest should retain one constant extent probe");
    };
    assert!(is_public);
    assert!(matches!(
        copied.expressions.expression(copied_expression),
        ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
    ));
    assert!(matches!(
        forest.expressions.expression(expression),
        ExpressionNode::Integer(_)
    ));
}
