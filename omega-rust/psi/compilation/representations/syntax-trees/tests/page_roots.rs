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
    assert_eq!(
        forest.snapshot().constant_extent_probes,
        [syntax_trees::snapshot::ConstantExtentProbeSnapshot {
            expression: syntax_trees::snapshot::ExpressionSnapshot::Integer {
                text: "1".to_owned()
            },
            is_public: true,
        }]
    );
    let mut copied = SyntaxTrees::new(SourceId(1));
    copied.extend_from(&forest);
    assert_eq!(copied.snapshot(), forest.snapshot());
    assert!(matches!(
        forest.expressions.expression(expression),
        ExpressionNode::Integer(_)
    ));
}
