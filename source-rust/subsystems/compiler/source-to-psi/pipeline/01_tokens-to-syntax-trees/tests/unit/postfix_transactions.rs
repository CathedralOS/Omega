use crate::expressions::postfix::*;

#[test]
fn rejected_nested_static_attempts_leave_the_published_prefix_intact() {
    for spelling in [
        "::<Outer<[u8; Count]>, value + >()",
        "::<Outer<Inner<3>>, value + >()",
    ] {
        let tokens = crate::frontend_fixture::Lexer::new(spelling)
            .tokenize()
            .unwrap();

        let mut syntax = SyntaxTrees::new(source_text::SourceId(7));
        let prefix = syntax.expressions.insert(ExpressionNode::Boolean(true));
        let input = Input::new(source_text::SourceId(7), &tokens, &mut syntax);
        let types = syntax.type_references.type_reference_count();
        assert!(
            try_parse_static_machine_arguments(&mut syntax, input)
                .unwrap()
                .is_none()
        );
        assert_eq!(syntax.expressions.iter_expressions().count(), 1);
        assert_eq!(syntax.type_references.type_reference_count(), types);
        assert!(matches!(
            syntax.expressions.expression(prefix),
            ExpressionNode::Boolean(true)
        ));
    }
}
